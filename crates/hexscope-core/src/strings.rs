//! What a file hexscope does not take apart still says in its own words. A
//! folder in someone's home — `/Users/olena/…`, `/home/olena/…`,
//! `C:\Users\Olena\…` — names the account that made or built it, and an
//! email address names a person. Programs keep both in formats of their
//! own: a Photoshop or Blender file the folders its pictures came from, a
//! compiled program the paths of the machine it was built on. Read as ASCII
//! and as UTF-16, as Windows programs write text.

/// One thing found: its kind, what to say, and where its first copy lies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub kind: &'static str,
    pub text: String,
    pub at: u64,
    pub len: u64,
}

/// Bytes read, at most: past this, a file is not looked through.
const MAX_SCAN: usize = 64 << 20;
/// Distinct names and addresses kept for a line; the rest are counted.
const MAX_KEPT: usize = 4;
/// Longest path shown, in characters.
const MAX_PATH: usize = 120;
/// Accounts that belong to no one: shared folders, build machines, defaults.
const NOT_PEOPLE: [&str; 9] = [
    "shared",
    "public",
    "default",
    "default user",
    "all users",
    "runner",
    "root",
    "user",
    "username",
];
/// What follows an `@` in names that only look like addresses: pictures at
/// twice the size (`icon@2x.png`), files, packages.
const NOT_DOMAINS: [&str; 14] = [
    "png", "jpg", "jpeg", "gif", "svg", "webp", "pdf", "js", "css", "json", "xml", "html", "txt",
    "md",
];

/// The character `step` bytes wide at `i`, if it is printable ASCII: a byte
/// for ASCII, or a byte and a zero for UTF-16 little-endian.
fn at(data: &[u8], i: usize, step: usize) -> Option<u8> {
    let c = *data.get(i)?;
    if step == 2 && data.get(i + 1) != Some(&0) {
        return None;
    }
    (0x20..0x7F).contains(&c).then_some(c)
}

/// What a name in a home folder, a path, an address's name and its domain
/// are made of.
fn word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b' ')
}
fn path_char(c: u8) -> bool {
    !matches!(c, b'"' | b'\'' | b'<' | b'>' | b'|' | b'*' | b'?')
}
fn local(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'%' | b'+' | b'-')
}
fn host(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-')
}

/// The printable run that starts at `i`, while `keep` holds, up to `max`.
fn run(
    data: &[u8],
    mut i: usize,
    step: usize,
    max: usize,
    keep: fn(u8) -> bool,
) -> (String, usize) {
    let start = i;
    let mut out = String::new();
    while out.len() < max {
        match at(data, i, step) {
            Some(c) if keep(c) => out.push(c as char),
            _ => break,
        }
        i += step;
    }
    (out, i - start)
}

/// Where each copy of `needle` starts, written `step` bytes a character.
fn each(data: &[u8], needle: &[u8], step: usize) -> Vec<usize> {
    let mut wide = Vec::with_capacity(needle.len() * step);
    for &b in needle {
        wide.push(b);
        if step == 2 {
            wide.push(0);
        }
    }
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = data
        .get(from..)
        .and_then(|d| d.windows(wide.len()).position(|w| w == wide))
    {
        out.push(from + p);
        from += p + wide.len();
    }
    out
}

/// A name, an extra text, and where its first copy lies.
type Hit = (String, String, usize, usize);

/// Accounts named by home folders, each with the first path that names it.
fn homes(data: &[u8], step: usize, out: &mut Vec<Hit>) {
    for needle in [&b"/Users/"[..], b"\\Users\\", b"/home/"] {
        let sep = needle[0];
        for p in each(data, needle, step) {
            let name_at = p + needle.len() * step;
            let (name, n) = run(data, name_at, step, 64, word);
            if name.is_empty()
                || at(data, name_at + n, step) != Some(sep)
                || NOT_PEOPLE.iter().any(|n| n.eq_ignore_ascii_case(&name))
                || out
                    .iter()
                    .any(|(user, ..)| user.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            // From the drive letter, when there is one, to where the path ends.
            let start = if sep == b'\\' && p >= 2 * step && at(data, p - step, step) == Some(b':') {
                p - 2 * step
            } else {
                p
            };
            let (path, len) = run(data, start, step, MAX_PATH, path_char);
            out.push((name, path.trim_end().to_string(), start, len));
        }
    }
}

/// Email addresses: a name, an `@`, and a domain with a dot and a word after it.
fn emails(data: &[u8], step: usize, out: &mut Vec<Hit>) {
    for p in each(data, b"@", step) {
        // Back over the name, past any dots or dashes it cannot start with.
        let mut start = p;
        while start >= step
            && at(data, start - step, step).is_some_and(local)
            && p - start < 64 * step
        {
            start -= step;
        }
        while start < p && matches!(at(data, start, step), Some(b'.' | b'-' | b'+')) {
            start += step;
        }
        let (name, _) = run(data, start, step, 64, local);
        let (mut domain, n) = run(data, p + step, step, 128, host);
        while domain.ends_with('.') || domain.ends_with('-') {
            domain.pop();
        }
        let top = &domain[domain
            .bytes()
            .rposition(|b| b == b'.')
            .map_or(domain.len(), |d| d + 1)..];
        if name.is_empty()
            || !name.bytes().any(|c| c.is_ascii_alphabetic())
            || top.len() < 2
            || !top.bytes().all(|c| c.is_ascii_alphabetic())
            || NOT_DOMAINS.iter().any(|d| d.eq_ignore_ascii_case(top))
            || domain.ends_with("example.com")
            || name == "git"
        {
            continue;
        }
        let mut address = name;
        address.push('@');
        address.push_str(&domain);
        if !out.iter().any(|(a, ..)| a.eq_ignore_ascii_case(&address)) {
            out.push((address, String::new(), start, p + step + n - start));
        }
    }
}

/// A line of what was found, the first few by name and the rest counted.
fn listed(found: &[Hit]) -> String {
    let mut out = String::new();
    for (i, (name, ..)) in found.iter().take(MAX_KEPT).enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(name);
    }
    if found.len() > MAX_KEPT {
        out.push_str(" and ");
        out.push_str(&(found.len() - MAX_KEPT).to_string());
        out.push_str(" more");
    }
    out
}

/// The accounts and addresses the file's text names.
pub fn scan(data: &[u8]) -> Vec<Found> {
    let data = &data[..data.len().min(MAX_SCAN)];
    let mut users = Vec::new();
    let mut addresses = Vec::new();
    for step in [1, 2] {
        homes(data, step, &mut users);
        emails(data, step, &mut addresses);
    }
    let mut out = Vec::new();
    if let Some((_, path, at, len)) = users.first() {
        out.push(Found {
            kind: "account",
            text: listed(&users) + ", in paths like " + path,
            at: *at as u64,
            len: *len as u64,
        });
    }
    if let Some((_, _, at, len)) = addresses.first() {
        out.push(Found {
            kind: "emails",
            text: listed(&addresses),
            at: *at as u64,
            len: *len as u64,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn home_folders_name_their_accounts_in_ascii_and_utf16() {
        let mut data = b"\x00\x01PSD /Users/olena/Pictures/sea.jpg\x00".to_vec();
        data.extend(wide("C:\\Users\\Ivan K\\Desktop\\plan.dwg"));
        data.extend(b"/home/runner/work/x /Users/Shared/y /home/olena/z");
        let found = scan(&data);
        assert_eq!(found[0].kind, "account");
        assert_eq!(
            found[0].text,
            "olena, Ivan K, in paths like /Users/olena/Pictures/sea.jpg"
        );
        assert_eq!(found[0].at, 6);
    }

    #[test]
    fn addresses_are_named_and_pictures_at_twice_the_size_are_not() {
        let mut data = b"contact: olena.koval@acme.com.ua; icon@2x.png git@github.com".to_vec();
        data.extend(wide("ivan@mail.example.com; ivan@ukr.net"));
        let found = scan(&data);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, "emails");
        assert_eq!(found[0].text, "olena.koval@acme.com.ua, ivan@ukr.net");
    }

    /// A file in no format hexscope reads still says who made it, through the
    /// whole parse: a node for each, and the facts a summary lists.
    #[test]
    fn a_blender_file_names_its_author() {
        let data = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/scene.blend"
        ))
        .unwrap();
        let s = crate::summary::summarize(&data);
        assert_eq!(s.format, "unknown");
        assert_eq!(
            s.facts,
            vec![
                (
                    "account",
                    "olena, in paths like /Users/olena/Projects/Acme/textures/wood_diffuse.png"
                        .to_string()
                ),
                ("emails", "olena.koval@studio.ua".to_string()),
            ]
        );
        assert!(crate::clean::clean(&data).is_err(), "no format, no copy");
    }

    #[test]
    fn many_are_counted_and_nothing_is_found_in_plain_bytes() {
        let data: String = (0..7).map(|i| format!("u{i}@mail.org ")).collect();
        assert_eq!(
            scan(data.as_bytes())[0].text,
            "u0@mail.org, u1@mail.org, u2@mail.org, u3@mail.org and 3 more"
        );
        assert!(scan(&(0..=255u8).cycle().take(100_000).collect::<Vec<_>>()).is_empty());
        assert!(scan(b"").is_empty());
    }
}
