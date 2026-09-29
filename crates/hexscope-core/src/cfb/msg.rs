//! Outlook's saved message (MS-OXMSG): each property a stream named for its
//! tag, attachments and recipients storages of their own. A message that
//! came by mail keeps the headers it arrived with; they are read as an
//! email's are, with its text and the names of its files, so a message
//! saved from Outlook is checked as the same message saved as .eml would be.

use super::Compound;
use crate::eml::parse_eml;

/// Longest text of a message read.
const MAX_TEXT: u64 = 2 << 20;

/// A property stream's name, `__substg1.0_` then its tag in hex, as what it holds.
pub(crate) fn label(name: &str) -> Option<String> {
    if let Some(n) = name.strip_prefix("__attach_version1.0_#") {
        return Some(format!(
            "attachment {}",
            u32::from_str_radix(n, 16).ok()? + 1
        ));
    }
    if let Some(n) = name.strip_prefix("__recip_version1.0_#") {
        return Some(format!(
            "recipient {}",
            u32::from_str_radix(n, 16).ok()? + 1
        ));
    }
    match name {
        "__properties_version1.0" => return Some("properties".into()),
        "__nameid_version1.0" => return Some("named properties".into()),
        _ => {}
    }
    let tag = name.strip_prefix("__substg1.0_")?;
    let what = match tag.get(..4)? {
        "0037" => "subject",
        "0E1D" => "subject, normalised",
        "0070" => "conversation",
        "0C1A" => "sender's name",
        "0C1F" => "sender's address",
        "5D01" => "sender's SMTP address",
        "0042" => "sent for, name",
        "0065" => "sent for, address",
        "0E04" => "to",
        "0E03" => "cc",
        "0E02" => "bcc",
        "007D" => "internet headers",
        "1035" => "message id",
        "1000" => "text",
        "1013" => "HTML",
        "1009" => "RTF",
        "3001" => "display name",
        "3003" => "address",
        "39FE" => "SMTP address",
        "3701" => "file",
        "3703" => "extension",
        "3704" => "file name, short",
        "3707" => "file name",
        "370E" => "MIME type",
        _ => return None,
    };
    Some(what.into())
}

pub(crate) struct Message {
    /// Kind, text, and the entry it was read from.
    pub(crate) facts: Vec<(&'static str, String, Option<usize>)>,
    /// Each attached file's name, and the entry holding its bytes.
    pub(crate) attachments: Vec<(String, Option<usize>)>,
}

/// A text property under `parent`: its entry and its text.
fn text(c: &Compound, parent: usize, id: &str) -> Option<(usize, String)> {
    if let Some(i) = c.find(parent, &format!("__substg1.0_{id}001F")) {
        let b = c.read(&c.entries[i], MAX_TEXT);
        return Some((i, super::utf16(&b).trim_end_matches('\0').to_string()));
    }
    let i = c
        .find(parent, &format!("__substg1.0_{id}001E"))
        .or_else(|| c.find(parent, &format!("__substg1.0_{id}0102")))?;
    let b = c.read(&c.entries[i], MAX_TEXT);
    Some((
        i,
        String::from_utf8_lossy(&b)
            .trim_end_matches('\0')
            .to_string(),
    ))
}

/// The headers without those about the body's own form, which the message
/// built around them replaces.
fn without_mime(headers: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in headers.split_inclusive('\n') {
        if !line.starts_with([' ', '\t']) {
            let name = line
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            skipping = matches!(
                name.as_str(),
                "content-type"
                    | "content-transfer-encoding"
                    | "mime-version"
                    | "content-disposition"
            );
        }
        if !skipping {
            out.push_str(line);
        }
    }
    out.trim_end().to_string()
}

pub(crate) fn read(c: &Compound) -> Message {
    let mut attachments = Vec::new();
    for (i, e) in c.entries.iter().enumerate() {
        if e.parent != Some(0) || e.kind != 1 || !e.name.starts_with("__attach_version1.0_#") {
            continue;
        }
        let name = ["3707", "3704", "3001"]
            .iter()
            .find_map(|id| text(c, i, id).map(|(_, t)| t).filter(|t| !t.is_empty()))
            .unwrap_or_else(|| format!("attachment {}", attachments.len() + 1));
        attachments.push((name, c.find(i, "__substg1.0_37010102")));
    }

    let mut facts = Vec::new();
    let Some((headers_at, headers)) = text(c, 0, "007D") else {
        return Message { facts, attachments };
    };
    // The HTML body; else the RTF one, which carries the HTML of a message
    // that came as HTML, or links of its own; else the plain text.
    let rtf = || {
        let i = c.find(0, "__substg1.0_10090102")?;
        let html = super::rtf::to_html(&super::rtf::decompress(&c.read(&c.entries[i], MAX_TEXT))?);
        Some((i, html))
    };
    let html = text(c, 0, "1013").or_else(rtf);
    let plain = text(c, 0, "1000");
    let (body_at, body, form) = match (&html, &plain) {
        (Some((i, t)), _) => (Some(*i), t.as_str(), "text/html"),
        (None, Some((i, t))) => (Some(*i), t.as_str(), "text/plain"),
        _ => (None, "", "text/plain"),
    };
    let mut email = without_mime(&headers);
    email.push_str(
        "\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"hexscope-msg\"\r\n\r\n",
    );
    email.push_str(&format!(
        "--hexscope-msg\r\nContent-Type: {form}; charset=utf-8\r\n\r\n{body}\r\n"
    ));
    for (name, _) in &attachments {
        let name = name.replace(['"', '\r', '\n'], "");
        email.push_str(&format!(
            "--hexscope-msg\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"{name}\"\r\n\r\n\r\n"
        ));
    }
    email.push_str("--hexscope-msg--\r\n");

    let first_file = attachments.iter().find_map(|(_, at)| *at);
    for f in parse_eml(email.as_bytes()).facts {
        let at = match f.kind {
            "weblinks" | "linkmismatch" | "linkidn" => body_at,
            "riskyfile" => first_file,
            _ => Some(headers_at),
        };
        facts.push((f.kind, f.text, at));
    }
    Message { facts, attachments }
}
