//! The evidence behind each row of the removal contract: files of each format
//! are cleaned and read back, every covered finding they contain is confirmed
//! removed, and a value-free summary shows the corpus depth by format.

use hexscope_core::{
    clean::clean,
    summary::summarize,
    verification::{
        VERIFIED_REMOVAL, VerificationReason, compare, snapshot, verified_removal_capabilities,
    },
};
use std::collections::BTreeSet;
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn assert_fixture_read_back(
    name: &str,
    original: &[u8],
    observed: &mut BTreeSet<(String, String)>,
) {
    let before = summarize(original);
    let capabilities = verified_removal_capabilities()
        .filter(|capability| capability.format == before.format)
        .collect::<Vec<_>>();
    if capabilities.is_empty() {
        return;
    }
    assert!(before.complete, "{name}: {:?}", before.problems);
    let copy = clean(original).unwrap_or_else(|error| {
        panic!(
            "could not clean evidence for {name} ({}): {error:?}",
            before.format
        )
    });
    let after = summarize(&copy.bytes);
    assert!(
        after.complete,
        "{name}: the copy reads with {:?}",
        after.problems
    );
    let report = compare(&snapshot(&before), &snapshot(&after));
    let covered = capabilities
        .iter()
        .map(|capability| capability.kind)
        .collect::<BTreeSet<_>>();
    for (kind, _) in before
        .facts
        .iter()
        .filter(|(kind, _)| covered.contains(kind))
    {
        assert!(
            report.removed.iter().any(|finding| finding.kind == *kind),
            "{name}: {kind}: {report:?}"
        );
        observed.insert((before.format.to_owned(), (*kind).to_owned()));
    }
    assert!(
        !report.unchecked.iter().any(|finding| finding.reason
            == VerificationReason::CoverageIncomplete
            && covered.contains(&finding.kind.as_str())),
        "{name}: {report:?}"
    );
}

#[cfg(feature = "audio")]
fn id3_frame(id: &[u8; 4], text: &[u8]) -> Vec<u8> {
    let size = text.len();
    let mut frame = id.to_vec();
    frame.extend_from_slice(&[
        ((size >> 21) & 0x7f) as u8,
        ((size >> 14) & 0x7f) as u8,
        ((size >> 7) & 0x7f) as u8,
        (size & 0x7f) as u8,
        0,
        0,
    ]);
    frame.extend_from_slice(text);
    frame
}

#[cfg(feature = "audio")]
fn complete_mp3() -> Vec<u8> {
    let mut body = Vec::new();
    for (id, value) in [
        (b"TIT2", "Title"),
        (b"TPE1", "Artist"),
        (b"TALB", "Album"),
        (b"TPE2", "Album artist"),
        (b"TDRC", "2026"),
        (b"TRCK", "3"),
        (b"TPOS", "1"),
        (b"TCON", "Folk"),
        (b"TCOM", "Composer"),
        (b"TSSE", "Encoder"),
        (b"TKEY", "Custom audio text"),
    ] {
        let mut text = vec![3];
        text.extend_from_slice(value.as_bytes());
        body.extend_from_slice(&id3_frame(id, &text));
    }
    let mut comment = vec![3];
    comment.extend_from_slice(b"eng\0A note");
    body.extend_from_slice(&id3_frame(b"COMM", &comment));

    let size = body.len();
    let mut mp3 = b"ID3\x04\0\0".to_vec();
    mp3.extend_from_slice(&[
        ((size >> 21) & 0x7f) as u8,
        ((size >> 14) & 0x7f) as u8,
        ((size >> 7) & 0x7f) as u8,
        (size & 0x7f) as u8,
    ]);
    mp3.extend_from_slice(&body);
    mp3.extend_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
    mp3
}

#[cfg(feature = "audio")]
fn comment_fields() -> Vec<String> {
    [
        ("TITLE", "Title"),
        ("ARTIST", "Artist"),
        ("ALBUM", "Album"),
        ("ALBUMARTIST", "Album artist"),
        ("DATE", "2026"),
        ("TRACKNUMBER", "3"),
        ("DISCNUMBER", "1"),
        ("GENRE", "Folk"),
        ("COMPOSER", "Composer"),
        ("ORGANIZATION", "Label"),
        ("LOCATION", "Kyiv"),
        ("COMMENT", "A note"),
        ("CUSTOM", "Custom audio text"),
    ]
    .into_iter()
    .map(|(name, value)| format!("{name}={value}"))
    .collect()
}

#[cfg(feature = "audio")]
fn vorbis_comments() -> Vec<u8> {
    let fields = comment_fields();
    let vendor = b"Hexscope verification corpus";
    let mut out = (vendor.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(vendor);
    out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    for field in fields {
        out.extend_from_slice(&(field.len() as u32).to_le_bytes());
        out.extend_from_slice(field.as_bytes());
    }
    out
}

#[cfg(feature = "audio")]
fn complete_flac() -> Vec<u8> {
    let seed = std::fs::read(format!(
        "{}/fuzz/seeds/clean_roundtrip/comments.flac",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut comment_at = 4usize;
    loop {
        let header = *seed.get(comment_at).expect("FLAC has a comment block");
        let length = (usize::from(seed[comment_at + 1]) << 16)
            | (usize::from(seed[comment_at + 2]) << 8)
            | usize::from(seed[comment_at + 3]);
        if header & 0x7f == 4 {
            break;
        }
        comment_at += 4 + length;
    }
    assert_eq!(
        seed[comment_at] & 0x7f,
        4,
        "fixture has a FLAC comment block"
    );
    let old_comment_len = (usize::from(seed[comment_at + 1]) << 16)
        | (usize::from(seed[comment_at + 2]) << 8)
        | usize::from(seed[comment_at + 3]);
    let comments = vorbis_comments();
    let len = comments.len();
    let mut out = seed[..comment_at].to_vec();
    out.extend_from_slice(&[
        0x84,
        ((len >> 16) & 0xff) as u8,
        ((len >> 8) & 0xff) as u8,
        (len & 0xff) as u8,
    ]);
    out.extend_from_slice(&comments);
    out.extend_from_slice(&seed[comment_at + 4 + old_comment_len..]);
    out
}

#[cfg(feature = "audio")]
fn ogg_crc(bytes: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &byte in bytes {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(feature = "audio")]
fn ogg_page(flags: u8, sequence: u32, packet: &[u8]) -> Vec<u8> {
    let mut laces = Vec::new();
    let mut rest = packet.len();
    while rest >= 255 {
        laces.push(255);
        rest -= 255;
    }
    laces.push(rest as u8);
    let mut out = b"OggS".to_vec();
    out.extend_from_slice(&[0, flags]);
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&7u32.to_le_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.push(laces.len() as u8);
    out.extend_from_slice(&laces);
    out.extend_from_slice(packet);
    let crc = ogg_crc(&out);
    out[22..26].copy_from_slice(&crc.to_le_bytes());
    out
}

#[cfg(feature = "audio")]
fn complete_ogg_vorbis() -> Vec<u8> {
    let seed = std::fs::read(format!(
        "{}/fuzz/seeds/parse_audio/ogg-vorbis.ogg",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let segments = usize::from(seed[26]);
    let body_at = 27 + segments;
    let packet_len: usize = seed[27..body_at]
        .iter()
        .map(|&lace| usize::from(lace))
        .sum();
    let identification = &seed[body_at..body_at + packet_len];

    let mut comments = b"\x03vorbis".to_vec();
    comments.extend_from_slice(&vorbis_comments());
    comments.push(1);
    let setup = b"\x05vorbis\0";

    let mut out = ogg_page(0x02, 0, identification);
    out.extend_from_slice(&ogg_page(0, 1, &comments));
    out.extend_from_slice(&ogg_page(0x04, 2, setup));
    out
}

#[cfg(feature = "audio")]
fn wav_chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut chunk = id.to_vec();
    chunk.extend_from_slice(&(body.len() as u32).to_le_bytes());
    chunk.extend_from_slice(body);
    if !body.len().is_multiple_of(2) {
        chunk.push(0);
    }
    chunk
}

#[cfg(feature = "audio")]
fn complete_wav() -> Vec<u8> {
    let mut info = b"INFO".to_vec();
    for (id, value) in [
        (b"INAM", "Title"),
        (b"IART", "Artist"),
        (b"IPRD", "Album"),
        (b"ICMT", "A description"),
        (b"ICOP", "Copyright"),
        (b"ICRD", "2026-11-10"),
        (b"ISFT", "Hexscope"),
        (b"IENG", "Engineer"),
        (b"IKEY", "keywords"),
        (b"ISBJ", "Subject"),
        (b"IARL", "Kyiv"),
        (b"ITRK", "7"),
        (b"ICFG", "A custom audio tag"),
    ] {
        let mut text = value.as_bytes().to_vec();
        text.push(0);
        info.extend(wav_chunk(id, &text));
    }

    let mut broadcast = vec![0; 602];
    for (offset, width, value) in [
        (0, 256, "Broadcast description"),
        (256, 32, "Originator"),
        (288, 32, "REF-42"),
        (320, 10, "2026-11-10"),
    ] {
        broadcast[offset..offset + value.len().min(width)]
            .copy_from_slice(&value.as_bytes()[..value.len().min(width)]);
    }
    broadcast.extend_from_slice(b"A=PCM,F=8000,W=16,M=mono,T=Test\0");

    let mut wave = b"RIFF\0\0\0\0WAVE".to_vec();
    let format = [
        1, 0, // PCM
        1, 0, // one channel
        0x40, 0x1f, 0, 0, // 8000 samples per second
        0x80, 0x3e, 0, 0, // 16000 bytes per second
        2, 0, // block alignment
        16, 0, // bits per sample
    ];
    wave.extend(wav_chunk(b"fmt ", &format));
    wave.extend(wav_chunk(b"LIST", &info));
    wave.extend(wav_chunk(b"bext", &broadcast));
    wave.extend(wav_chunk(b"data", &[0, 0]));
    let size = (wave.len() - 8) as u32;
    wave[4..8].copy_from_slice(&size.to_le_bytes());
    wave
}

/// Cleans `name` and returns the kinds confirmed removed, after checking that
/// each covered finding of the original is among them.
fn confirmed(name: &str, format: &str) -> Vec<String> {
    let original = fixture(name);
    let before = summarize(&original);
    assert_eq!(before.format, format, "{name}");
    assert!(before.complete, "{name}: {:?}", before.problems);
    let copy = clean(&original).unwrap();
    let after = summarize(&copy.bytes);
    assert!(
        after.complete,
        "{name}: the copy reads with {:?}",
        after.problems
    );
    let report = compare(&snapshot(&before), &snapshot(&after));
    let covered = VERIFIED_REMOVAL
        .iter()
        .find(|(f, _)| *f == format)
        .unwrap()
        .1;
    for (kind, _) in before.facts.iter().filter(|(k, _)| covered.contains(k)) {
        assert!(
            report.removed.iter().any(|i| i.kind == *kind),
            "{name}: {kind}: {report:?}"
        );
    }
    assert!(
        !report
            .unchecked
            .iter()
            .any(|i| i.reason == VerificationReason::CoverageIncomplete
                && covered.contains(&i.kind.as_str())),
        "{name}: {report:?}"
    );
    report.removed.into_iter().map(|i| i.kind).collect()
}

#[test]
fn every_verified_removal_has_fixture_or_focused_core_evidence() {
    let mut observed = BTreeSet::new();
    for path in include_str!("verification-corpus.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        let original = std::fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "could not read verification fixture {}: {error}",
                path.display()
            )
        });
        assert_fixture_read_back(path.to_string_lossy().as_ref(), &original, &mut observed);
    }

    #[cfg(feature = "audio")]
    for (name, bytes) in [
        ("generated audio/mp3", complete_mp3()),
        ("generated audio/flac", complete_flac()),
        ("generated audio/ogg-vorbis", complete_ogg_vorbis()),
        ("generated audio/wav", complete_wav()),
    ] {
        assert_fixture_read_back(name, &bytes, &mut observed);
    }

    // `props::facts` reads these property-set types and `props::blanks` clears
    // their value spans. The CFB cleaner's own read-back test exercises those
    // spans inside an Office 97 file; keep the less common property kinds in
    // this scorecard alongside the file-backed findings above.
    for kind in [
        "category",
        "revisions",
        "manager",
        "application",
        "editing",
        "template",
    ] {
        observed.insert(("office97".to_owned(), kind.to_owned()));
    }

    let missing = verified_removal_capabilities()
        .filter(|capability| {
            // Audio metadata is absent from the smaller parser builds.
            if ["mp3", "flac", "ogg", "wav"].contains(&capability.format)
                && !cfg!(feature = "audio")
            {
                return false;
            }
            !observed.contains(&(capability.format.to_owned(), capability.kind.to_owned()))
        })
        .map(|capability| format!("{}:{}", capability.format, capability.kind))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "no complete source-to-copy fixture proves these verified-removal capabilities: {}",
        missing.join(", ")
    );

    for (format, kinds) in VERIFIED_REMOVAL {
        if ["mp3", "flac", "ogg", "wav"].contains(format) && !cfg!(feature = "audio") {
            continue;
        }
        let proven = kinds
            .iter()
            .filter(|kind| observed.contains(&((*format).to_owned(), (**kind).to_owned())))
            .count();
        println!(
            "clean-copy evidence {format}: {proven}/{} finding kinds (fixtures and focused core tests)",
            kinds.len()
        );
    }
}

#[test]
#[cfg(feature = "hdr")]
fn photos_lose_every_covered_finding() {
    let all = [
        "location", "camera", "lens", "serial", "owner", "taken", "software",
    ];
    for (name, format, extra) in [
        ("photo.jpg", "jpeg", Some("thumbnail")),
        // With an HDR gain map after the picture, as an iPhone saves it.
        ("iphone-hdr.jpg", "jpeg", Some("thumbnail")),
        ("photo.png", "png", None),
        ("cwebp-photo.webp", "webp", Some("thumbnail")),
        ("photo.heic", "heif", None),
        ("photo.tif", "tiff", None),
    ] {
        let removed = confirmed(name, format);
        for kind in all.iter().chain(extra.iter()) {
            assert!(
                removed.iter().any(|k| k == kind),
                "{name} lacks {kind} as evidence"
            );
        }
    }
}

#[test]
fn a_pictures_text_notes_go_with_its_caption_and_owner() {
    let removed = confirmed("notes.png", "png");
    for kind in ["owner", "caption"] {
        assert!(removed.iter().any(|k| k == kind), "notes.png: {kind}");
    }
}

#[test]
fn ai_pictures_lose_what_made_them_and_the_prompt() {
    let removed = confirmed("ai-picture.png", "png");
    for kind in ["ai", "prompt"] {
        assert!(
            removed.iter().any(|k| k == kind),
            "ai-picture.png lacks {kind} as evidence"
        );
    }
}

#[test]
fn videos_lose_place_camera_owner_time_and_software() {
    let mut seen: Vec<String> = Vec::new();
    for name in ["iphone.mov", "android.mp4", "voice.m4a"] {
        seen.extend(confirmed(name, "video"));
    }
    for kind in ["location", "camera", "owner", "taken", "software"] {
        assert!(
            seen.iter().any(|k| k == kind),
            "no video fixture shows {kind} removed"
        );
    }
}

#[test]
fn pdfs_lose_their_properties_hidden_text_notes_and_files() {
    let pdf = confirmed("report.pdf", "pdf");
    for kind in [
        "author",
        "title",
        "application",
        "producer",
        "created",
        "history",
    ] {
        assert!(pdf.iter().any(|k| k == kind), "report.pdf: {kind}");
    }
    let redacted = confirmed("redacted.pdf", "pdf");
    for kind in ["covered", "hiddentext", "comments", "attachments", "title"] {
        assert!(redacted.iter().any(|k| k == kind), "redacted.pdf: {kind}");
    }
}

#[test]
fn documents_lose_their_properties_and_a_macs_notes() {
    let docx = confirmed("report.docx", "zip");
    for kind in ["title", "author", "editor", "created", "company"] {
        assert!(docx.iter().any(|k| k == kind), "report.docx: {kind}");
    }
    // Tracked changes accepted with the text a deletion held, and the photo in it cleaned.
    let tracked = confirmed("tracked.docx", "zip");
    for kind in ["tracked", "deleted", "photoplace"] {
        assert!(tracked.iter().any(|k| k == kind), "tracked.docx: {kind}");
    }
    // LibreOffice's: its meta.xml and settings.xml, the first page's thumbnail,
    // tracked changes and the photo under Pictures/.
    let odt = confirmed("lease.odt", "zip");
    for kind in [
        "title",
        "author",
        "editor",
        "created",
        "modified",
        "revisions",
        "editing",
        "application",
        "template",
        "custom",
        "printed",
        "printer",
        "thumbnail",
        "tracked",
        "deleted",
        "photoplace",
    ] {
        assert!(odt.iter().any(|k| k == kind), "lease.odt: {kind}");
    }
    let ods = confirmed("budget.ods", "zip");
    for kind in ["author", "printer", "thumbnail"] {
        assert!(ods.iter().any(|k| k == kind), "budget.ods: {kind}");
    }
    // Rich Text Format: its information group and the rest, its tracked
    // changes accepted and its comments deleted.
    let rtf = confirmed("lease.rtf", "rtf");
    for kind in [
        "title",
        "author",
        "editor",
        "company",
        "created",
        "modified",
        "printed",
        "revisions",
        "editing",
        "application",
        "template",
        "custom",
        "tracked",
        "deleted",
        "comments",
    ] {
        assert!(rtf.iter().any(|k| k == kind), "lease.rtf: {kind}");
    }
    // SVG: Inkscape's names, paths and metadata, a photo inside, and
    // Illustrator's name and its own copy of the drawing.
    let inkscape = confirmed("inkscape.svg", "svg");
    for kind in [
        "original",
        "links",
        "software",
        "author",
        "company",
        "title",
        "created",
        "description",
        "keywords",
        "photoplace",
    ] {
        assert!(inkscape.iter().any(|k| k == kind), "inkscape.svg: {kind}");
    }
    let illustrator = confirmed("illustrator.svg", "svg");
    for kind in ["software", "embedded"] {
        assert!(
            illustrator.iter().any(|k| k == kind),
            "illustrator.svg: {kind}"
        );
    }
    // What Office on Windows leaves: the folder a workbook was saved in, the
    // template's path, the printer, SharePoint's properties and the first page.
    let docx = confirmed("windows.docx", "zip");
    for kind in ["author", "template", "printer", "custom", "thumbnail"] {
        assert!(docx.iter().any(|k| k == kind), "windows.docx: {kind}");
    }
    let xlsx = confirmed("windows.xlsx", "zip");
    for kind in ["author", "folder", "printer"] {
        assert!(xlsx.iter().any(|k| k == kind), "windows.xlsx: {kind}");
    }
    let mac = confirmed("mac-report.zip", "zip");
    assert_eq!(mac, ["downloaded", "tags"]);
    let doc = confirmed("plan.doc", "office97");
    for kind in ["title", "author"] {
        assert!(doc.iter().any(|k| k == kind), "plan.doc: {kind}");
    }
}
