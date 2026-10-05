//! Bounded cross-file observations. No identity, home or route inference.
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct Signal {
    pub index: usize,
    pub serial: Option<String>,
    pub location: Option<[f64; 2]>,
    pub taken: Option<String>,
}
#[derive(Debug)]
pub struct Report {
    pub total: usize,
    pub serial_groups: Vec<Vec<usize>>,
    pub location_groups: Vec<Vec<usize>>,
    pub located_files: usize,
    pub local_date_order: Vec<usize>,
    pub utc_date_order: Vec<usize>,
}
impl Report {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"hexscope.photo-mosaic\",\"version\":1,\"total\":{},\"serial_groups\":{:?},\"location_groups\":{:?},\"located_files\":{},\"local_date_order\":{:?},\"utc_date_order\":{:?},\"location_radius_metres\":100,\"inferences\":false}}",
            self.total,
            self.serial_groups,
            self.location_groups,
            self.located_files,
            self.local_date_order,
            self.utc_date_order
        )
    }
}
fn sphere([lat, lon]: [f64; 2]) -> Option<[f64; 3]> {
    if !lat.is_finite()
        || !lon.is_finite()
        || !(-90.0..=90.0).contains(&lat)
        || !(-180.0..=180.0).contains(&lon)
    {
        return None;
    }
    let lat = lat.to_radians();
    let lon = lon.to_radians();
    Some([
        6_371_000.0 * lat.cos() * lon.cos(),
        6_371_000.0 * lat.cos() * lon.sin(),
        6_371_000.0 * lat.sin(),
    ])
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn cell(p: [f64; 3]) -> [i32; 3] {
    p.map(|v| (v / 100.0).floor() as i32)
}
/// Strict EXIF/ISO full timestamps. Unknown-zone wall clocks stay separate from UTC.
fn date(text: &str) -> Option<(i64, bool)> {
    let b = text.as_bytes();
    if b.len() < 19
        || b.len() > 25
        || !matches!(b[4], b':' | b'-')
        || b[7] != b[4]
        || !matches!(b[10], b' ' | b'T')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let part = |start, end| {
        let s = b.get(start..end)?;
        if !s.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(s).ok()?.parse::<i64>().ok()
    };
    let (year, month, day, hour, minute, second) = (
        part(0, 4)?,
        part(5, 7)?,
        part(8, 10)?,
        part(11, 13)?,
        part(14, 16)?,
        part(17, 19)?,
    );
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day < 1 || day > days[(month - 1) as usize] {
        return None;
    }
    let years = year - 1;
    let mut timestamp = (years * 365 + years / 4 - years / 100
        + years / 400
        + days[..(month - 1) as usize].iter().sum::<i64>()
        + day
        - 1)
        * 86400
        + hour * 3600
        + minute * 60
        + second;
    let known = match &b[19..] {
        [] => false,
        [b'Z'] => true,
        [sign, a, c, b':', d, e]
            if matches!(sign, b'+' | b'-') && [a, c, d, e].iter().all(|v| v.is_ascii_digit()) =>
        {
            let h = i64::from(*a - b'0') * 10 + i64::from(*c - b'0');
            let m = i64::from(*d - b'0') * 10 + i64::from(*e - b'0');
            if h > 14 || m > 59 || (h == 14 && m != 0) {
                return None;
            }
            let delta = h * 3600 + m * 60;
            timestamp -= if *sign == b'+' { delta } else { -delta };
            true
        }
        _ => return None,
    };
    Some((timestamp, known))
}
pub fn analyze(signals: &[Signal]) -> Result<Report, &'static str> {
    if signals.len() > 1000 {
        return Err("photo mosaic is limited to 1000 files");
    }
    let mut ids = BTreeSet::new();
    if signals.iter().any(|s| s.index == 0 || !ids.insert(s.index)) {
        return Err("file indexes must be positive and unique");
    }
    let mut serials: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut locations = Vec::new();
    let mut local = Vec::new();
    let mut utc = Vec::new();
    for s in signals {
        if let Some(serial) = s.serial.as_deref().map(str::trim).filter(|v| {
            !v.is_empty()
                && v.len() <= 512
                && !matches!(
                    v.to_ascii_lowercase().as_str(),
                    "unknown" | "none" | "n/a" | "0"
                )
        }) {
            serials.entry(serial).or_default().push(s.index);
        }
        if let Some(point) = s.location.and_then(sphere) {
            locations.push((s.index, point));
        }
        if let Some((date, known)) = s.taken.as_deref().and_then(date) {
            if known {
                utc.push((date, s.index));
            } else {
                local.push((date, s.index));
            }
        }
    }
    // Representative clusters: every member is within 100m of its first file.
    // This avoids a chain of 90m links being presented as one nearby location.
    locations.sort_by_key(|(id, _)| *id);
    let mut cells: BTreeMap<[i32; 3], Vec<usize>> = BTreeMap::new();
    let mut groups: Vec<([f64; 3], Vec<usize>)> = Vec::new();
    for (id, point) in &locations {
        let key = cell(*point);
        let mut matched = None;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if let Some(candidates) = cells.get(&[key[0] + x, key[1] + y, key[2] + z]) {
                        for candidate in candidates {
                            if distance(groups[*candidate].0, *point) <= 100.0
                                && matched.is_none_or(|current| *candidate < current)
                            {
                                matched = Some(*candidate);
                            }
                        }
                    }
                }
            }
        }
        if let Some(group) = matched {
            groups[group].1.push(*id);
        } else {
            let index = groups.len();
            groups.push((*point, vec![*id]));
            cells.entry(key).or_default().push(index);
        }
    }
    let mut serial_groups = serials
        .into_values()
        .filter(|v| v.len() > 1)
        .collect::<Vec<_>>();
    for group in &mut serial_groups {
        group.sort_unstable();
    }
    serial_groups.sort();
    let mut location_groups = groups
        .into_iter()
        .map(|(_, g)| g)
        .filter(|g| g.len() > 1)
        .collect::<Vec<_>>();
    location_groups.sort();
    local.sort_unstable();
    utc.sort_unstable();
    Ok(Report {
        total: signals.len(),
        serial_groups,
        location_groups,
        located_files: locations.len(),
        local_date_order: local.into_iter().map(|(_, i)| i).collect(),
        utc_date_order: utc.into_iter().map(|(_, i)| i).collect(),
    })
}
/// Only photos are admitted by the caller; values never enter the output report.
pub fn from_facts(index: usize, facts: &[(&str, String)]) -> Signal {
    let first = |kind| {
        facts
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, v)| v.clone())
    };
    let location = first("location").and_then(|v| {
        let (a, b) = v.split_once(',')?;
        Some([a.trim().parse().ok()?, b.trim().parse().ok()?])
    });
    Signal {
        index,
        serial: first("serial"),
        location,
        taken: first("taken"),
    }
}

/// Preserve original coordinate precision across CLI and browser consumers.
pub fn from_document(index: usize, doc: &crate::Document) -> Option<Signal> {
    let facts = match doc {
        crate::Document::Jpeg(d) => &d.facts,
        crate::Document::Png(d) => &d.facts,
        crate::Document::Webp(d) => &d.facts,
        crate::Document::Heif(d) => &d.facts,
        crate::Document::Gif(d) => &d.facts,
        _ => return None,
    };
    Some(Signal {
        index,
        serial: facts.serial.as_ref().map(|f| f.text.clone()),
        taken: facts.taken.as_ref().map(|f| f.text.clone()),
        location: facts.location.as_ref().map(|l| [l.latitude, l.longitude]),
    })
}
