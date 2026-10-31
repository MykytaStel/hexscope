//! Conservative recognition of the XMP fields that describe an Ultra HDR
//! gain map. Unknown XML is never retained in a clean copy.

const MAX_PACKET: usize = 64 * 1024;
const MAX_ATTRIBUTES: usize = 8;
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XMP: &str = "adobe:ns:meta/";
const CONTAINER: &str = "http://ns.google.com/photos/1.0/container/";
const ITEM: &str = "http://ns.google.com/photos/1.0/container/item/";
const HDRGM: &str = "http://ns.adobe.com/hdr-gain-map/1.0/";
const APPLE_HDR: &str = "http://ns.apple.com/HDRGainMap/1.0/";

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct HdrXmpInfo {
    pub safe: bool,
    pub has_container_directory: bool,
    pub has_gain_map_metadata: bool,
}

#[derive(Debug)]
struct Tag<'a> {
    name: &'a str,
    attributes: [Option<(&'a str, &'a str)>; MAX_ATTRIBUTES],
    attribute_count: usize,
    self_closing: bool,
}

impl<'a> Tag<'a> {
    fn attributes(&self) -> impl Iterator<Item = (&'a str, &'a str)> + '_ {
        self.attributes[..self.attribute_count]
            .iter()
            .flatten()
            .copied()
    }

    fn get(&self, name: &str) -> Option<&'a str> {
        self.attributes()
            .find_map(|(key, value)| (key == name).then_some(value))
    }
}

/// Accepts only XMP made of Ultra HDR/ISO-adjacent technical fields and the
/// GContainer directory. Personal, unknown, malformed, or entity-bearing XML
/// is rejected so a cleaner can remove it or refuse when it is essential.
pub(crate) fn inspect_hdr_xmp(packet: &[u8], expected_map_len: Option<u64>) -> HdrXmpInfo {
    let mut info = HdrXmpInfo {
        has_container_directory: packet
            .windows(b"Container:Directory".len())
            .any(|w| w == b"Container:Directory"),
        ..HdrXmpInfo::default()
    };
    if packet.len() > MAX_PACKET {
        return info;
    }
    let Ok(xml) = std::str::from_utf8(packet) else {
        return info;
    };

    let mut stack = Vec::<&str>::new();
    let mut namespace_stack = Vec::<u8>::new();
    let mut directory_items = Vec::<(&str, &str, Option<u64>)>::new();
    let mut directory_depth = None;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut at = 0usize;

    while at < xml.len() {
        if xml.as_bytes()[at].is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if !xml[at..].starts_with('<') {
            let end = xml[at..].find('<').map_or(xml.len(), |n| at + n);
            if !xml[at..end].trim().is_empty() {
                return info;
            }
            at = end;
            continue;
        }
        if xml[at..].starts_with("<?") {
            let Some(end) = xml[at..].find("?>").map(|n| at + n + 2) else {
                return info;
            };
            let pi = &xml[at + 2..end - 2];
            if !safe_xpacket(pi) {
                return info;
            }
            at = end;
            continue;
        }
        if xml[at..].starts_with("<!") {
            return info;
        }
        if xml[at..].starts_with("</") {
            let Some(end) = xml[at..].find('>').map(|n| at + n) else {
                return info;
            };
            let name = xml[at + 2..end].trim();
            if stack.pop() != Some(name) {
                return info;
            }
            namespace_stack.pop();
            if name == "Container:Directory" {
                if directory_items.len() != 2
                    || !directory_items.iter().any(|(s, _, _)| *s == "Primary")
                    || !directory_items.iter().any(|(s, _, _)| *s == "GainMap")
                    || directory_items.iter().any(|(semantic, mime, length)| {
                        *mime != "image/jpeg"
                            || (*semantic == "Primary" && length.is_some())
                            || (*semantic == "GainMap"
                                && (length.is_none()
                                    || expected_map_len.is_some_and(|expected| {
                                        length.is_some_and(|actual| actual != expected)
                                    })))
                    })
                {
                    return info;
                }
                directory_depth = None;
            }
            if stack.is_empty() {
                root_closed = true;
            }
            at = end + 1;
            continue;
        }

        let Some(end) = tag_end(xml, at + 1) else {
            return info;
        };
        let raw = xml[at + 1..end].trim_end();
        let self_closing = raw.ends_with('/');
        let raw = raw.strip_suffix('/').unwrap_or(raw).trim_end();
        let Some(tag) = parse_tag(raw, self_closing) else {
            return info;
        };
        if !valid_tag_name(tag.name)
            || !valid_parent(stack.last().copied(), tag.name)
            || (root_closed && !tag.name.is_empty())
        {
            return info;
        }
        if !root_seen {
            if tag.name != "x:xmpmeta" {
                return info;
            }
            root_seen = true;
        }

        let mut namespaces = namespace_stack.last().copied().unwrap_or(0);
        for (name, value) in tag.attributes() {
            if name == "xmlns" {
                return info;
            }
            if let Some(prefix) = name.strip_prefix("xmlns:") {
                let Some(namespace) = namespace_bit(prefix, value) else {
                    return info;
                };
                namespaces |= namespace;
            }
        }
        if !prefix_is_bound(tag.name, namespaces)
            || tag
                .attributes()
                .any(|(name, _)| !name.starts_with("xmlns:") && !prefix_is_bound(name, namespaces))
            || !tag
                .attributes()
                .all(|(name, value)| safe_attribute(name, value, tag.name))
        {
            return info;
        }

        if tag.name == "Container:Directory" {
            if directory_depth.is_some() {
                return info;
            }
            directory_depth = Some(stack.len());
            directory_items.clear();
        } else if (tag.name == "rdf:Seq" && stack.last().copied() != Some("Container:Directory"))
            || (tag.name == "rdf:li" && stack.last().copied() != Some("rdf:Seq"))
        {
            return info;
        } else if tag.name == "Container:Item" {
            if stack.last().copied() != Some("rdf:li") {
                return info;
            }
            let Some(semantic) = tag.get("Item:Semantic") else {
                return info;
            };
            let Some(mime) = tag.get("Item:Mime") else {
                return info;
            };
            let length = tag.get("Item:Length").and_then(|s| s.parse::<u64>().ok());
            directory_items.push((semantic, mime, length));
        }

        if tag.get("hdrgm:Version").is_some() || tag.get("HDRGainMap:HDRGainMapVersion").is_some() {
            info.has_gain_map_metadata = true;
        }
        if !tag.self_closing {
            stack.push(tag.name);
            namespace_stack.push(namespaces);
        } else if stack.is_empty() && tag.name == "x:xmpmeta" {
            root_closed = true;
        }
        at = end + 1;
    }

    info.safe = root_seen
        && root_closed
        && stack.is_empty()
        && directory_depth.is_none()
        && (info.has_gain_map_metadata || info.has_container_directory);
    info
}

fn tag_end(xml: &str, from: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, ch) in xml[from..].char_indices() {
        match (quote, ch) {
            (Some(q), c) if q == c => quote = None,
            (None, '"' | '\'') => quote = Some(ch),
            (None, '>') => return Some(from + offset),
            _ => {}
        }
    }
    None
}

fn parse_tag(raw: &str, self_closing: bool) -> Option<Tag<'_>> {
    let mut at = 0usize;
    let bytes = raw.as_bytes();
    while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    let name = raw.get(..at)?;
    parse_tag_attributes(name, raw, at, self_closing)
}

fn parse_tag_attributes<'a>(
    name: &'a str,
    raw: &'a str,
    mut at: usize,
    self_closing: bool,
) -> Option<Tag<'a>> {
    let bytes = raw.as_bytes();
    let mut attributes = [None; MAX_ATTRIBUTES];
    let mut attribute_count = 0;
    while at < bytes.len() {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at == bytes.len() {
            break;
        }
        let start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'=' {
            at += 1;
        }
        if start == at {
            return None;
        }
        let key = raw.get(start..at)?;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let value = parse_attribute_value(raw, &mut at)?;
        if attributes[..attribute_count]
            .iter()
            .flatten()
            .any(|(name, _)| *name == key)
            || attribute_count == MAX_ATTRIBUTES
        {
            return None;
        }
        attributes[attribute_count] = Some((key, value));
        attribute_count += 1;
    }
    Some(Tag {
        name,
        attributes,
        attribute_count,
        self_closing,
    })
}

fn parse_attribute_value<'a>(raw: &'a str, at: &mut usize) -> Option<&'a str> {
    let bytes = raw.as_bytes();
    while *at < bytes.len() && bytes[*at].is_ascii_whitespace() {
        *at += 1;
    }
    if bytes.get(*at) != Some(&b'=') {
        return None;
    }
    *at += 1;
    while *at < bytes.len() && bytes[*at].is_ascii_whitespace() {
        *at += 1;
    }
    let quote = *bytes.get(*at)?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    *at += 1;
    let value_start = *at;
    while *at < bytes.len() && bytes[*at] != quote {
        if matches!(bytes[*at], b'<' | b'>' | b'&') {
            return None;
        }
        *at += 1;
    }
    let value = raw.get(value_start..*at)?;
    if bytes.get(*at) != Some(&quote) {
        return None;
    }
    *at += 1;
    Some(value)
}

fn valid_tag_name(name: &str) -> bool {
    matches!(
        name,
        "x:xmpmeta"
            | "rdf:RDF"
            | "rdf:Description"
            | "Container:Directory"
            | "rdf:Seq"
            | "rdf:li"
            | "Container:Item"
    )
}

fn valid_parent(parent: Option<&str>, name: &str) -> bool {
    matches!(
        (parent, name),
        (None, "x:xmpmeta")
            | (Some("x:xmpmeta"), "rdf:RDF")
            | (Some("rdf:RDF"), "rdf:Description")
            | (Some("rdf:Description"), "Container:Directory")
            | (Some("Container:Directory"), "rdf:Seq")
            | (Some("rdf:Seq"), "rdf:li")
            | (Some("rdf:li"), "Container:Item")
    )
}

fn namespace_bit(prefix: &str, uri: &str) -> Option<u8> {
    match (prefix, uri) {
        ("x", XMP) => Some(1 << 0),
        ("rdf", RDF) => Some(1 << 1),
        ("Container", CONTAINER) => Some(1 << 2),
        ("Item", ITEM) => Some(1 << 3),
        ("hdrgm", HDRGM) => Some(1 << 4),
        ("HDRGainMap", APPLE_HDR) => Some(1 << 5),
        _ => None,
    }
}

fn prefix_is_bound(name: &str, namespaces: u8) -> bool {
    name.split_once(':')
        .and_then(|(prefix, _)| match prefix {
            "x" => Some(1 << 0),
            "rdf" => Some(1 << 1),
            "Container" => Some(1 << 2),
            "Item" => Some(1 << 3),
            "hdrgm" => Some(1 << 4),
            "HDRGainMap" => Some(1 << 5),
            _ => None,
        })
        .is_some_and(|bit| namespaces & bit != 0)
}

fn safe_attribute(name: &str, value: &str, tag: &str) -> bool {
    if name.starts_with("xmlns:") {
        return true;
    }
    match name {
        "rdf:about" => tag == "rdf:Description" && value.is_empty(),
        "rdf:parseType" => tag == "rdf:li" && value == "Resource",
        "Item:Semantic" => tag == "Container:Item" && matches!(value, "Primary" | "GainMap"),
        "Item:Mime" => tag == "Container:Item" && value == "image/jpeg",
        "Item:Length" => {
            tag == "Container:Item"
                && !value.is_empty()
                && value.bytes().all(|b| b.is_ascii_digit())
        }
        "Item:Padding" => {
            tag == "Container:Item"
                && !value.is_empty()
                && value.bytes().all(|b| b.is_ascii_digit())
        }
        "hdrgm:Version" => tag == "rdf:Description" && value == "1.0",
        "HDRGainMap:HDRGainMapVersion" => {
            tag == "rdf:Description"
                && !value.is_empty()
                && value.bytes().all(|b| b.is_ascii_digit())
        }
        "hdrgm:BaseRenditionIsHDR" => tag == "rdf:Description" && matches!(value, "True" | "False"),
        "hdrgm:GainMapMin"
        | "hdrgm:GainMapMax"
        | "hdrgm:Gamma"
        | "hdrgm:OffsetSDR"
        | "hdrgm:OffsetHDR"
        | "hdrgm:HDRCapacityMin"
        | "hdrgm:HDRCapacityMax" => tag == "rdf:Description" && numeric_values(value),
        _ => false,
    }
}

fn numeric_values(value: &str) -> bool {
    let mut found = false;
    for part in value
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
    {
        found = true;
        if !part.parse::<f64>().is_ok_and(f64::is_finite) {
            return false;
        }
    }
    found
}

fn safe_xpacket(pi: &str) -> bool {
    let Some(rest) = pi.strip_prefix("xpacket") else {
        return false;
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return false;
    }
    let Some(attrs) = parse_tag_attributes("xpacket", rest, 0, true) else {
        return false;
    };
    attrs.attributes().all(|(name, value)| match name {
        "begin" => value == "\u{feff}",
        "id" => value == "W5M0MpCehiHzreSzNTczkc9d",
        "end" => matches!(value, "w" | "r"),
        _ => false,
    }) && (attrs.get("begin").is_some() || attrs.get("end").is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apple_map() -> String {
        format!(
            "<x:xmpmeta xmlns:x=\"{XMP}\"><rdf:RDF xmlns:rdf=\"{RDF}\"><rdf:Description xmlns:HDRGainMap=\"{APPLE_HDR}\" HDRGainMap:HDRGainMapVersion=\"131072\"/></rdf:RDF></x:xmpmeta>"
        )
    }

    #[test]
    fn accepts_apple_gain_map_technical_metadata() {
        let info = inspect_hdr_xmp(apple_map().as_bytes(), None);
        assert!(info.safe);
        assert!(info.has_gain_map_metadata);
        assert!(!info.has_container_directory);
    }

    #[test]
    fn accepts_android_container_directory_with_matching_gain_map_length() {
        let xml = format!(
            "<x:xmpmeta xmlns:x=\"{XMP}\"><rdf:RDF xmlns:rdf=\"{RDF}\"><rdf:Description xmlns:Container=\"{CONTAINER}\" xmlns:Item=\"{ITEM}\" xmlns:hdrgm=\"{HDRGM}\" hdrgm:Version=\"1.0\"><Container:Directory><rdf:Seq><rdf:li rdf:parseType=\"Resource\"><Container:Item Item:Semantic=\"Primary\" Item:Mime=\"image/jpeg\"/></rdf:li><rdf:li rdf:parseType=\"Resource\"><Container:Item Item:Semantic=\"GainMap\" Item:Mime=\"image/jpeg\" Item:Length=\"3539\"/></rdf:li></rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>"
        );
        let info = inspect_hdr_xmp(xml.as_bytes(), Some(3539));
        assert!(info.safe);
        assert!(info.has_container_directory);
        assert!(info.has_gain_map_metadata);
        assert!(!inspect_hdr_xmp(xml.as_bytes(), Some(3540)).safe);
    }

    #[test]
    fn refuses_personal_or_unrecognized_xmp_properties() {
        let xml = apple_map().replace(
            "HDRGainMap:HDRGainMapVersion=\"131072\"",
            "HDRGainMap:HDRGainMapVersion=\"131072\" xmp:CreatorTool=\"Camera App\"",
        );
        assert!(!inspect_hdr_xmp(xml.as_bytes(), None).safe);
        assert!(
            !inspect_hdr_xmp(
                b"<!DOCTYPE x [<!ENTITY e SYSTEM 'file:///etc/passwd'>]>",
                None
            )
            .safe
        );
    }
}
