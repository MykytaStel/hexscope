//! The tree as parallel arrays, and what a PNG or a photo adds to them.

use super::*;

pub(crate) fn with_png(bytes: &[u8], doc: PngDocument) -> Parsed {
    let mut parsed = flatten(&doc.tree);
    parsed.preview = hexscope_core::png::preview::preview(bytes, &doc, PREVIEW_MAX)
        .map(|p| (p.width, p.height, p.rgba));
    parsed.format = "png";
    parsed.header_len = 2;
    parsed.trailer_len = 4;
    parsed.ihdr = doc.ihdr.map(|h| {
        [
            h.width,
            h.height,
            h.bit_depth as u32,
            h.color_type as u32,
            h.interlace as u32,
        ]
    });
    parsed.dimensions = doc.ihdr.map(|h| [h.width, h.height]);
    add_facts(&mut parsed, &doc.facts);
    parsed.trace = doc.trace.as_ref().map(|t| {
        [
            t.total_events as f64,
            t.literals as f64,
            t.matches as f64,
            t.output_bytes as f64,
        ]
    });
    parsed.idat_bytes = idat_payload_bytes(&doc.tree);

    for r in &doc.idat {
        let (start, end) = (r.start as usize, r.end() as usize);
        if let Some(payload) = bytes.get(start..end) {
            parsed.stream.extend_from_slice(payload);
        }
        parsed
            .segments
            .extend_from_slice(&[r.start as f64, r.len as f64]);
    }
    parsed.checkpoints = doc.trace.map(|t| t.checkpoints).unwrap_or_default();
    parsed.output = match doc.inflated {
        Some(output) => output,
        // Replay to the failure so the player has real bytes to draw.
        None => match parsed.body() {
            Some(body) => {
                let mut d = Decoder::new(body, MAX_PIXEL_BYTES);
                while let Some(Ok(_)) = d.step() {}
                d.into_output()
            }
            None => Vec::new(),
        },
    };
    parsed
}

pub(crate) fn add_facts(parsed: &mut Parsed, facts: &PhotoFacts) {
    // All image containers share EXIF orientation; the browser decodes it before resizing.
    if let Some(orientation @ 1..=8) = facts.orientation {
        parsed.orientation = orientation;
    }
    let listed = [
        ("camera", &facts.camera),
        ("lens", &facts.lens),
        ("serial", &facts.serial),
        ("owner", &facts.owner),
        ("place", &facts.place),
        ("taken", &facts.taken),
        ("caption", &facts.caption),
        ("software", &facts.software),
        ("original", &facts.original),
        ("history", &facts.history),
        ("thumbnail", &facts.thumbnail),
        ("shutter", &facts.shutter),
        ("uptime", &facts.uptime),
        ("linked", &facts.linked),
        ("screenshot", &facts.screenshot),
        ("ai", &facts.ai),
        ("credentials", &facts.credentials),
        ("prompt", &facts.prompt),
    ];
    for (kind, fact) in listed {
        if let Some(f) = fact {
            parsed.facts.push((kind, sanitise(&f.text), f.node));
        }
    }
    parsed.location = facts.location.map(|l| {
        [
            l.latitude,
            l.longitude,
            l.altitude.unwrap_or(f64::NAN),
            l.node as f64,
        ]
    });
}

/// Replaces control characters so a corrupt chunk type or text payload cannot
/// inject separators, newlines or terminal escapes into the UI.
pub(crate) fn sanitise(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .collect()
}

pub(crate) fn display(value: &Option<Value>) -> String {
    match value {
        None => String::new(),
        Some(Value::U64(n)) => n.to_string(),
        Some(Value::Text(t)) => sanitise(t),
        Some(Value::Bytes(n)) if *n == 1 => "1 byte".to_string(),
        Some(Value::Bytes(n)) => format!("{n} bytes"),
        Some(Value::Enum { raw, name }) => format!("{raw} ({name})"),
    }
}

pub(crate) fn kind_code(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::Container => 0,
        NodeKind::Field => 1,
        NodeKind::Warning => 2,
        NodeKind::Error => 3,
    }
}

pub(crate) fn idat_payload_bytes(tree: &ParseTree) -> f64 {
    tree.nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Container && n.label == "IDAT")
        .map(|n| match n.value {
            Some(Value::Bytes(b)) => b as f64,
            _ => 0.0,
        })
        .sum()
}

/// The tree as parallel arrays. Format-specific extras are filled in by the
/// caller; for an unrecognised file this is everything there is.
pub fn flatten(tree: &ParseTree) -> Parsed {
    let nodes = tree.nodes();
    let sep = SEPARATOR.to_string();

    Parsed {
        starts: nodes.iter().map(|n| n.range.start as f64).collect(),
        lens: nodes.iter().map(|n| n.range.len as f64).collect(),
        parents: nodes
            .iter()
            .map(|n| n.parent.map_or(-1, |p| p as i32))
            .collect(),
        kinds: nodes.iter().map(|n| kind_code(n.kind)).collect(),
        labels: nodes
            .iter()
            .map(|n| sanitise(&n.label))
            .collect::<Vec<_>>()
            .join(&sep),
        values: nodes
            .iter()
            .map(|n| display(&n.value))
            .collect::<Vec<_>>()
            .join(&sep),
        ihdr: None,
        trace: None,
        idat_bytes: 0.0,
        stream: Vec::new(),
        segments: Vec::new(),
        output: Vec::new(),
        checkpoints: Vec::new(),
        format: "unknown",
        dimensions: None,
        facts: Vec::new(),
        verification: None,
        location: None,
        header_len: 0,
        trailer_len: 0,
        zip: None,
        extract_error: String::new(),
        docs: DocTables::default(),
        composition: Vec::new(),
        preview: None,
        blackouts: Vec::new(),
        blackout_texts: Vec::new(),
        pdf_source: Vec::new(),
        attachment_names: Vec::new(),
        source: Vec::new(),
        blocks: None,
        blocks_note: String::new(),
        orientation: 1,
    }
}
