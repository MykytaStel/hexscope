//! Content Credentials (C2PA): a signed record, kept in JUMBF boxes, of who
//! or what made a picture and how it was edited — written by cameras,
//! Photoshop and the AI image generators. The whole manifest is CBOR under
//! signatures; two things are worth saying to a person, and both read as
//! plain strings in it: the program that wrote it, and whether it says the
//! picture was made by AI (IPTC's digital source types). The signature is
//! not checked: this says what the file claims, not that the claim is true.

use super::{Fact, PhotoFacts};
use crate::model::NodeId;

/// IPTC's words for a picture AI made, or AI helped make, most specific
/// first (cv.iptc.org/newscodes/digitalsourcetype).
const AI: [(&str, &str); 3] = [
    (
        "compositeWithTrainedAlgorithmicMedia",
        "partly made with generative AI",
    ),
    ("trainedAlgorithmicMedia", "made with generative AI"),
    (
        "algorithmicMedia",
        "made by a computer program, not a camera",
    ),
];

/// What a digital source type URL or name says, in words.
pub(crate) fn source_type(value: &str) -> Option<&'static str> {
    AI.iter()
        .find(|(key, _)| value.contains(key))
        .map(|(_, words)| *words)
}

/// A CBOR text string (RFC 8949 §3.1, major type 3) at `at`.
fn text_at(b: &[u8], at: usize) -> Option<&str> {
    let head = *b.get(at)?;
    if head >> 5 != 3 {
        return None;
    }
    let (len, skip) = match head & 0x1f {
        n @ 0..=23 => (usize::from(n), 1),
        24 => (usize::from(*b.get(at + 1)?), 2),
        25 => (
            usize::from(u16::from_be_bytes([*b.get(at + 1)?, *b.get(at + 2)?])),
            3,
        ),
        _ => return None,
    };
    std::str::from_utf8(b.get(at + skip..at + skip + len)?).ok()
}

/// Where the value after a CBOR text key such as `claim_generator` starts.
fn key(b: &[u8], name: &str, from: usize) -> Option<usize> {
    let mut needle = vec![0x60 | name.len() as u8];
    needle.extend_from_slice(name.as_bytes());
    let at = from
        + b.get(from..)?
            .windows(needle.len())
            .position(|w| w == needle.as_slice())?;
    Some(at + needle.len())
}

/// The text value after a CBOR text key, and where it starts.
fn value_of<'a>(b: &'a [u8], name: &str, from: usize) -> Option<(&'a str, usize)> {
    let after = key(b, name, from)?;
    Some((text_at(b, after)?, after))
}

/// `Adobe_Photoshop/25.0 adobe_c2pa/0.7.6` → `Adobe Photoshop 25.0`.
fn program(generator: &str) -> String {
    let first = generator.split_whitespace().next().unwrap_or("");
    first
        .replace(['_', '/'], " ")
        .trim()
        .chars()
        .take(80)
        .collect()
}

/// Facts from a C2PA manifest store's bytes, named at `node`.
pub(crate) fn from_manifest(bytes: &[u8], node: NodeId) -> PhotoFacts {
    let mut f = PhotoFacts::default();
    if bytes.windows(4).all(|w| w != b"c2pa") {
        return f;
    }
    // C2PA 2 lists the program in claim_generator_info; 1 in claim_generator.
    let made = key(bytes, "claim_generator_info", 0)
        .and_then(|at| value_of(bytes, "name", at).filter(|(_, n)| *n < at + 400))
        .map(|(name, at)| {
            let version = value_of(bytes, "version", at)
                .filter(|(_, v)| *v < at + 200)
                .map(|(v, _)| format!(" {v}"))
                .unwrap_or_default();
            format!("{name}{version}")
        })
        .or_else(|| value_of(bytes, "claim_generator", 0).map(|(g, _)| program(g)))
        .filter(|p| !p.trim().is_empty());
    f.credentials = Some(Fact {
        text: match &made {
            Some(p) => format!("a signed record of how it was made, written by {p}"),
            None => "a signed record of how it was made".into(),
        },
        node,
    });
    let text = String::from_utf8_lossy(bytes);
    if let Some(words) = source_type(&text) {
        f.ai = Some(Fact {
            text: format!("{words}, its Content Credentials say"),
            node,
        });
    }
    f
}

/// What an image generator's own text notes in a PNG say: Stable
/// Diffusion's web UIs write the prompt, the settings and the model;
/// ComfyUI its whole workflow.
pub(crate) fn from_generator_note(keyword: &str, text: &str, node: NodeId) -> PhotoFacts {
    let mut f = PhotoFacts::default();
    let fact = |t: String| {
        Some(Fact {
            text: t.chars().take(200).collect(),
            node,
        })
    };
    match keyword {
        // "a cat on a hill\nNegative prompt: …\nSteps: 20, …, Model: v1-5, …"
        "parameters" if text.contains("Steps:") => {
            let prompt = text
                .split("\nNegative prompt:")
                .next()
                .unwrap_or("")
                .split("\nSteps:")
                .next()
                .unwrap_or("")
                .trim();
            let model = text
                .split("Model: ")
                .nth(1)
                .and_then(|m| m.split([',', '\n']).next())
                .map(|m| format!(", model {}", m.trim()))
                .unwrap_or_default();
            f.ai = fact(format!("made with Stable Diffusion{model}"));
            if !prompt.is_empty() {
                f.prompt = fact(prompt.to_string());
            }
        }
        "prompt" | "workflow" if text.trim_start().starts_with('{') => {
            f.ai = fact("made with ComfyUI, an image generator".into());
            // The first text a node was given, which is the prompt.
            if let Some(rest) = text.split("\"text\"").nth(1)
                && let Some(start) = rest.find('"')
            {
                let body = &rest[start + 1..];
                let mut out = String::new();
                let mut escaped = false;
                for c in body.chars() {
                    match (escaped, c) {
                        (false, '\\') => escaped = true,
                        (false, '"') => break,
                        (true, 'n') => {
                            out.push(' ');
                            escaped = false;
                        }
                        (_, c) => {
                            out.push(c);
                            escaped = false;
                        }
                    }
                }
                if !out.trim().is_empty() {
                    f.prompt = fact(out.trim().to_string());
                }
            }
        }
        "invokeai_metadata" | "sd-metadata" | "Dream" => {
            f.ai = fact("made with InvokeAI, an image generator".into());
        }
        _ => {}
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tstr(s: &str) -> Vec<u8> {
        let mut v = if s.len() < 24 {
            vec![0x60 | s.len() as u8]
        } else {
            vec![0x78, s.len() as u8]
        };
        v.extend_from_slice(s.as_bytes());
        v
    }

    #[test]
    fn a_manifest_says_who_wrote_it_and_whether_ai_made_it() {
        let mut m = b"jumb....c2pa....".to_vec();
        m.extend(tstr("claim_generator_info"));
        m.extend([0x81, 0xa2]);
        m.extend(tstr("name"));
        m.extend(tstr("Adobe Firefly"));
        m.extend(tstr("version"));
        m.extend(tstr("3.1"));
        m.extend(b"...http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia");
        let f = from_manifest(&m, 4);
        assert_eq!(
            f.credentials.unwrap().text,
            "a signed record of how it was made, written by Adobe Firefly 3.1"
        );
        assert_eq!(
            f.ai.unwrap().text,
            "made with generative AI, its Content Credentials say"
        );

        let mut old = b"c2pa".to_vec();
        old.extend(tstr("claim_generator"));
        old.extend(tstr("Adobe_Photoshop/25.0 adobe_c2pa/0.7.6"));
        let f = from_manifest(&old, 4);
        assert_eq!(
            f.credentials.unwrap().text,
            "a signed record of how it was made, written by Adobe Photoshop 25.0"
        );
        assert!(f.ai.is_none());
        assert!(from_manifest(b"no manifest here", 4).credentials.is_none());
        assert!(
            from_manifest(&[0x7f, 0xff, b'c', b'2', b'p', b'a'], 4)
                .credentials
                .is_some()
        );
    }

    #[test]
    fn an_image_generators_notes_give_its_prompt() {
        let sd = "a lighthouse at dusk, oil painting\nNegative prompt: blurry\nSteps: 20, Sampler: Euler a, Model: dreamshaper_8, Seed: 1";
        let f = from_generator_note("parameters", sd, 2);
        assert_eq!(f.prompt.unwrap().text, "a lighthouse at dusk, oil painting");
        assert_eq!(
            f.ai.unwrap().text,
            "made with Stable Diffusion, model dreamshaper_8"
        );
        let comfy = r#"{"6": {"inputs": {"text": "a red \"fox\"\nin snow", "clip": ["4", 1]}}}"#;
        let f = from_generator_note("prompt", comfy, 2);
        assert_eq!(f.prompt.unwrap().text, "a red \"fox\" in snow");
        assert!(from_generator_note("Comment", "hello", 2).ai.is_none());
    }

    /// A PNG from a generator: its notes and its credentials are named,
    /// and the clean copy has neither.
    #[test]
    fn a_generated_png_says_so_and_its_clean_copy_does_not() {
        let chunk = |kind: &[u8; 4], data: &[u8]| {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(kind);
            c.extend_from_slice(data);
            let crc = crate::crc32::crc32(&c[4..]);
            c.extend_from_slice(&crc.to_be_bytes());
            c
        };
        let mut png = std::fs::read(format!(
            "{}/tests/fixtures/pngsuite/basn2c08.png",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let mut manifest = b"jumb c2pa ".to_vec();
        manifest.extend(tstr("claim_generator"));
        manifest.extend(tstr("OpenAI_API/1.0"));
        manifest.extend(b" trainedAlgorithmicMedia");
        let mut extra = chunk(
            b"tEXt",
            b"parameters\0a fox in snow\nSteps: 20, Model: sdxl",
        );
        extra.extend(chunk(b"caBX", &manifest));
        png.splice(33..33, extra);
        let doc = crate::png::parse_png(&png);
        let f = &doc.facts;
        assert_eq!(f.prompt.as_ref().unwrap().text, "a fox in snow");
        assert_eq!(
            f.credentials.as_ref().unwrap().text,
            "a signed record of how it was made, written by OpenAI API 1.0"
        );
        // Signed credentials say it before the generator's own note does.
        assert_eq!(
            f.ai.as_ref().unwrap().text,
            "made with generative AI, its Content Credentials say"
        );
        let c = crate::clean::clean(&png).unwrap();
        let what: Vec<&str> = c.removed.iter().map(|r| r.what.as_str()).collect();
        assert!(
            what.iter().any(|w| w.starts_with("Content Credentials")),
            "{what:?}"
        );
        let after = crate::png::parse_png(&c.bytes).facts;
        assert!(after.ai.is_none() && after.prompt.is_none() && after.credentials.is_none());
    }
}
