//! What XMP and IPTC say about a photo that EXIF may not: who it credits,
//! the place it names — a city and a country, with or without GPS — its
//! caption, the file it was made from, and how it was edited. Photoshop,
//! Lightroom and photo agencies write these; a phone rarely does.
//!
//! XMP is read with the same tag reader as a PDF's (XMP part 1 §7); IPTC
//! is the older binary record Photoshop keeps in a JPEG's APP13 (IPTC IIM
//! 4.2, inside Photoshop's image resources).

use super::{Fact, PhotoFacts};
use crate::model::NodeId;
use crate::pdf::facts::{xmp_date, xmp_text};

/// Longest fact kept, in characters.
const MAX: usize = 200;
/// Names listed in a history line, at most.
const MAX_AGENTS: usize = 3;

fn set(slot: &mut Option<Fact>, text: Option<String>, node: NodeId) {
    let Some(text) = text else { return };
    let text = text.trim();
    if slot.is_none() && !text.is_empty() {
        *slot = Some(Fact {
            text: text.chars().take(MAX).collect(),
            node,
        });
    }
}

/// Parts of a place, in order, each once: `Old Town, Lviv, Ukraine`.
fn place(parts: impl Iterator<Item = String>) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        let p = p.trim().to_string();
        if !p.is_empty() && !out.contains(&p) {
            out.push(p);
        }
    }
    (!out.is_empty()).then(|| out.join(", "))
}

/// The facts an XMP packet holds.
pub(crate) fn from_xmp(xml: &str, node: NodeId) -> PhotoFacts {
    let mut f = PhotoFacts::default();
    let get = |name: &str| xmp_text(xml, name);
    set(&mut f.owner, get("dc:creator"), node);
    set(&mut f.software, get("xmp:CreatorTool"), node);
    set(&mut f.camera, get("tiff:Model"), node);
    let taken = [
        "exif:DateTimeOriginal",
        "photoshop:DateCreated",
        "xmp:CreateDate",
    ]
    .iter()
    .find_map(|k| get(k));
    set(&mut f.taken, taken.map(|t| xmp_date(&t)), node);
    let named = [
        "Iptc4xmpCore:Location",
        "photoshop:City",
        "photoshop:State",
        "photoshop:Country",
    ];
    set(
        &mut f.place,
        place(named.iter().filter_map(|k| get(k))),
        node,
    );
    set(&mut f.caption, get("dc:description"), node);
    // The file this one was made from: Lightroom keeps its name, Camera Raw
    // the raw file's, Photoshop the path of what it was derived from.
    let original = [
        "xmpMM:PreservedFileName",
        "crs:RawFileName",
        "stRef:filePath",
    ]
    .iter()
    .find_map(|k| get(k));
    set(&mut f.original, original, node);
    set(&mut f.history, history(xml), node);
    f
}

/// `4 steps in Adobe Photoshop 25.0 and Lightroom, last on 2026-06-15 10:02 +02:00`
/// from `xmpMM:History`, whose events are written as attributes or as
/// elements.
fn history(xml: &str) -> Option<String> {
    let start = xml.find("<xmpMM:History")?;
    let end = xml[start..]
        .find("</xmpMM:History>")
        .map_or(xml.len(), |e| start + e);
    let h = &xml[start..end];
    let steps = h.matches("<rdf:li").count();
    if steps == 0 {
        return None;
    }
    let mut agents: Vec<String> = Vec::new();
    for agent in values(h, "stEvt:softwareAgent") {
        if !agents.contains(&agent) {
            agents.push(agent);
        }
    }
    let mut out = format!("{steps} {}", if steps == 1 { "step" } else { "steps" });
    if !agents.is_empty() {
        let more = agents.len().saturating_sub(MAX_AGENTS);
        agents.truncate(MAX_AGENTS);
        out.push_str(" in ");
        out.push_str(&agents.join(", "));
        if more > 0 {
            out.push_str(&format!(" and {more} more"));
        }
    }
    if let Some(when) = values(h, "stEvt:when").last() {
        out.push_str(&format!(", last on {}", xmp_date(when)));
    }
    Some(out)
}

/// Every value of a property written as `name="…"` or `<name>…</name>`,
/// in order.
fn values(xml: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let attr = format!("{name}=\"");
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let mut at = 0;
    while at < xml.len() {
        let a = xml[at..].find(&attr).map(|i| (at + i, true));
        let e = xml[at..].find(&open).map(|i| (at + i, false));
        let Some((i, is_attr)) = [a, e].into_iter().flatten().min_by_key(|&(i, _)| i) else {
            break;
        };
        let (from, until) = if is_attr {
            (i + attr.len(), "\"")
        } else {
            (i + open.len(), close.as_str())
        };
        let Some(len) = xml[from..].find(until) else {
            break;
        };
        let v = xml[from..from + len].trim();
        if !v.is_empty() && !v.contains('<') {
            out.push(v.to_string());
        }
        at = from + len;
    }
    out
}

/// The facts IPTC records in Photoshop's image resources hold: the payload
/// of an APP13 segment after `Photoshop 3.0\0`.
pub(crate) fn from_photoshop(resources: &[u8], node: NodeId) -> PhotoFacts {
    let mut f = PhotoFacts::default();
    let Some(iim) = resource(resources, 0x0404) else {
        return f;
    };
    let mut place_parts: Vec<String> = Vec::new();
    let mut at = 0;
    // Each dataset: 0x1C, record, dataset, a two-byte length, the data.
    while at + 5 <= iim.len() && iim[at] == 0x1C {
        let (record, dataset) = (iim[at + 1], iim[at + 2]);
        let len = u16::from_be_bytes([iim[at + 3], iim[at + 4]]) as usize;
        // A length with its top bit set is an extended one: rare, and not
        // for the short texts read here.
        if len & 0x8000 != 0 {
            break;
        }
        let Some(data) = iim.get(at + 5..at + 5 + len) else {
            break;
        };
        let text = || Some(String::from_utf8_lossy(data).into_owned());
        if record == 2 {
            match dataset {
                80 => set(&mut f.owner, text(), node),
                120 => set(&mut f.caption, text(), node),
                92 | 90 | 95 | 101 => place_parts.extend(text()),
                _ => {}
            }
        }
        at += 5 + len;
    }
    set(&mut f.place, place(place_parts.into_iter()), node);
    f
}

/// One image resource's data by its id (Photoshop's `8BIM` blocks: a
/// signature, an id, a padded Pascal name, a length, the padded data).
fn resource(mut b: &[u8], id: u16) -> Option<&[u8]> {
    while b.len() >= 12 && &b[..4] == b"8BIM" {
        let this = u16::from_be_bytes([b[4], b[5]]);
        // The name: a length byte and the name, padded to an even size.
        let name = (1 + b[6] as usize + 1) & !1;
        let at = 6 + name;
        let len = u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize;
        let data = b.get(at + 4..(at + 4).checked_add(len)?)?;
        if this == id {
            return Some(data);
        }
        let next = (at + 4 + len + 1) & !1;
        b = b.get(next..)?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIGHTROOM: &str = r#"<x:xmpmeta><rdf:RDF><rdf:Description
        xmp:CreatorTool="Adobe Photoshop Lightroom Classic 14.0 (Macintosh)"
        xmpMM:PreservedFileName="IMG_4821.CR3"
        photoshop:City="Lviv" photoshop:Country="Ukraine"
        Iptc4xmpCore:Location="Rynok Square">
      <dc:creator><rdf:Seq><rdf:li>Olena Koval</rdf:li></rdf:Seq></dc:creator>
      <dc:description><rdf:Alt><rdf:li xml:lang="x-default">Before the meeting</rdf:li></rdf:Alt></dc:description>
      <xmpMM:History><rdf:Seq>
        <rdf:li stEvt:action="derived" stEvt:softwareAgent="Adobe Photoshop Lightroom Classic 14.0 (Macintosh)" stEvt:when="2026-06-14T19:00:00+02:00"/>
        <rdf:li stEvt:action="saved" stEvt:softwareAgent="Adobe Photoshop 25.0 (Macintosh)" stEvt:when="2026-06-15T10:02:11+02:00"/>
        <rdf:li><stEvt:action>saved</stEvt:action><stEvt:softwareAgent>Adobe Photoshop 25.0 (Macintosh)</stEvt:softwareAgent><stEvt:when>2026-06-15T10:05:00+02:00</stEvt:when></rdf:li>
      </rdf:Seq></xmpMM:History>
    </rdf:Description></rdf:RDF></x:xmpmeta>"#;

    fn text(f: &Option<Fact>) -> Option<&str> {
        f.as_ref().map(|f| f.text.as_str())
    }

    #[test]
    fn lightroom_and_photoshop_leave_a_trail() {
        let f = from_xmp(LIGHTROOM, 7);
        assert_eq!(text(&f.owner), Some("Olena Koval"));
        assert_eq!(text(&f.place), Some("Rynok Square, Lviv, Ukraine"));
        assert_eq!(text(&f.caption), Some("Before the meeting"));
        assert_eq!(text(&f.original), Some("IMG_4821.CR3"));
        assert_eq!(
            text(&f.history),
            Some(
                "3 steps in Adobe Photoshop Lightroom Classic 14.0 (Macintosh), Adobe Photoshop 25.0 (Macintosh), last on 2026-06-15 10:05 +02:00"
            )
        );
        assert_eq!(f.owner.unwrap().node, 7);
    }

    #[test]
    fn a_bare_packet_says_nothing() {
        let f = from_xmp(
            "<x:xmpmeta><rdf:RDF><rdf:Description/></rdf:RDF></x:xmpmeta>",
            0,
        );
        assert_eq!(f, PhotoFacts::default());
        assert_eq!(history("<xmpMM:History><rdf:Seq/></xmpMM:History>"), None);
    }

    fn iptc(sets: &[(u8, &str)]) -> Vec<u8> {
        let mut iim = Vec::new();
        for (ds, t) in sets {
            iim.extend([0x1C, 2, *ds]);
            iim.extend((t.len() as u16).to_be_bytes());
            iim.extend(t.as_bytes());
        }
        // A resource before it, with a name and odd length, to be skipped.
        let mut out = b"8BIM\x03\xED\x03abc".to_vec();
        out.extend(3u32.to_be_bytes());
        out.extend(b"xyz\0");
        out.extend(b"8BIM\x04\x04\0\0");
        out.extend((iim.len() as u32).to_be_bytes());
        out.extend(&iim);
        out
    }

    #[test]
    fn iptc_names_the_photographer_and_the_place() {
        let f = from_photoshop(
            &iptc(&[
                (80, "Petro Ivanenko"),
                (90, "Kyiv"),
                (101, "Ukraine"),
                (120, "Press photo"),
            ]),
            3,
        );
        assert_eq!(text(&f.owner), Some("Petro Ivanenko"));
        assert_eq!(text(&f.place), Some("Kyiv, Ukraine"));
        assert_eq!(text(&f.caption), Some("Press photo"));
    }

    #[test]
    fn broken_resources_give_nothing_and_do_not_panic() {
        let good = iptc(&[(80, "A")]);
        for cut in 0..good.len() {
            let _ = from_photoshop(&good[..cut], 0);
        }
        let mut long = good.clone();
        let at = long.len() - 3;
        long[at] = 0xFF; // a dataset length past the end
        let _ = from_photoshop(&long, 0);
        assert_eq!(from_photoshop(b"not 8BIM at all", 0), PhotoFacts::default());
    }
}
