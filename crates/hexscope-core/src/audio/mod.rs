mod comments;
pub(crate) mod docs;
mod flac;
mod id3;
mod ogg;

pub(crate) use id3::removable_tag_ranges;
pub use id3::{is_mp3, parse_mp3};

use crate::facts::DocumentFact;
use crate::model::ParseTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Flac,
    Ogg,
    OggVorbis,
    OggOpus,
}

pub fn parse(data: &[u8]) -> Option<AudioDocument> {
    if is_mp3(data) {
        Some(parse_mp3(data))
    } else if flac::is_flac(data) {
        Some(flac::parse_flac(data))
    } else if ogg::is_ogg(data) {
        Some(ogg::parse_ogg(data))
    } else {
        None
    }
}

/// The parts of an audio file this build can inspect without decoding audio.
#[derive(Debug)]
pub struct AudioDocument {
    pub format: AudioFormat,
    pub tree: ParseTree,
    pub facts: Vec<DocumentFact>,
    /// False when a tag was malformed, truncated, too large, or only partly read.
    pub metadata_complete: bool,
}

#[cfg(test)]
mod tests {
    use super::{is_mp3, parse, parse_mp3};

    fn synchsafe(n: usize) -> [u8; 4] {
        [
            ((n >> 21) & 0x7f) as u8,
            ((n >> 14) & 0x7f) as u8,
            ((n >> 7) & 0x7f) as u8,
            (n & 0x7f) as u8,
        ]
    }

    fn frame(id: &[u8], payload: &[u8], version: u8) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(id);
        match version {
            2 => out.extend_from_slice(&[
                ((payload.len() >> 16) & 0xff) as u8,
                ((payload.len() >> 8) & 0xff) as u8,
                (payload.len() & 0xff) as u8,
            ]),
            3 => out.extend_from_slice(&(payload.len() as u32).to_be_bytes()),
            4 => out.extend_from_slice(&synchsafe(payload.len())),
            _ => unreachable!(),
        }
        if version != 2 {
            out.extend_from_slice(&[0, 0]);
        }
        out.extend_from_slice(payload);
        out
    }

    fn tag(version: u8, body: &[u8]) -> Vec<u8> {
        let mut out = b"ID3".to_vec();
        out.extend_from_slice(&[version, 0, 0]);
        out.extend_from_slice(&synchsafe(body.len()));
        out.extend_from_slice(body);
        out
    }

    fn padded(value: &[u8], len: usize) -> Vec<u8> {
        let mut out = vec![b' '; len];
        let copied = value.len().min(len);
        out[..copied].copy_from_slice(&value[..copied]);
        out
    }

    fn vorbis_comments(vendor: &[u8], fields: &[&[u8]], framing: bool) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        out.extend_from_slice(vendor);
        out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        for field in fields {
            out.extend_from_slice(&(field.len() as u32).to_le_bytes());
            out.extend_from_slice(field);
        }
        if framing {
            out.push(1);
        }
        out
    }

    fn flac(fields: &[&[u8]]) -> Vec<u8> {
        let comments = vorbis_comments(b"test encoder", fields, false);
        let mut out = b"fLaC".to_vec();
        out.extend_from_slice(&[0, 0, 0, 34]); // mandatory STREAMINFO block
        out.extend_from_slice(&[0; 34]);
        out.push(0x84); // last block, VORBIS_COMMENT
        out.extend_from_slice(&[
            ((comments.len() >> 16) & 0xff) as u8,
            ((comments.len() >> 8) & 0xff) as u8,
            (comments.len() & 0xff) as u8,
        ]);
        out.extend_from_slice(&comments);
        out
    }

    fn ogg_page(serial: u32, sequence: u32, flags: u8, laces: &[u8], body: &[u8]) -> Vec<u8> {
        let mut out = b"OggS".to_vec();
        out.extend_from_slice(&[0, flags]);
        out.extend_from_slice(&[0; 8]); // granule position
        out.extend_from_slice(&serial.to_le_bytes());
        out.extend_from_slice(&sequence.to_le_bytes());
        out.extend_from_slice(&[0; 4]); // filled with the Ogg CRC below
        out.push(laces.len() as u8);
        out.extend_from_slice(laces);
        out.extend_from_slice(body);
        crate::ogg_crc::write_checksum(&mut out);
        out
    }

    fn ogg_packet_fixture(codec: &str, comments: &[u8]) -> Vec<u8> {
        let identification = if codec == "vorbis" {
            let mut packet = vec![1];
            packet.extend_from_slice(b"vorbis");
            packet.extend_from_slice(&0u32.to_le_bytes());
            packet.push(2);
            packet.extend_from_slice(&44_100u32.to_le_bytes());
            packet.extend_from_slice(&[0; 12]);
            packet.push(0x86);
            packet.push(1);
            packet
        } else {
            let mut packet = b"OpusHead".to_vec();
            packet.extend_from_slice(&[1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            packet
        };
        let mut out = ogg_page(7, 0, 2, &[identification.len() as u8], &identification);
        if comments.len() <= 255 {
            out.extend(ogg_page(7, 1, 0, &[comments.len() as u8], comments));
        } else {
            out.extend(ogg_page(7, 1, 0, &[255], &comments[..255]));
            out.extend(ogg_page(
                7,
                2,
                1,
                &[(comments.len() - 255) as u8],
                &comments[255..],
            ));
        }
        out
    }

    fn values<'a>(doc: &'a super::AudioDocument, kind: &str) -> Vec<&'a str> {
        doc.facts
            .iter()
            .filter(|f| f.kind == kind)
            .map(|f| f.text.as_str())
            .collect()
    }

    #[test]
    fn reads_id3v22_text_and_links_it_to_source_bytes() {
        let mut bytes = tag(2, &frame(b"TT2", b"\0Sample title", 2));
        bytes.extend_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
        let doc = parse_mp3(&bytes);

        assert_eq!(values(&doc, "title"), ["Sample title"]);
        assert!(doc.metadata_complete);
        let fact = &doc.facts[0];
        assert_eq!(doc.tree.get(fact.node).range.start, 16);
        assert_eq!(doc.tree.get(fact.node).range.len, 13);
    }

    #[test]
    fn id3v22_unsynchronisation_is_reported_as_incomplete() {
        let mut bytes = tag(2, &[]);
        bytes[5] = 0x80; // ID3v2.2 tag-level unsynchronisation

        let doc = parse_mp3(&bytes);

        assert!(!doc.metadata_complete);
        assert!(doc.tree.nodes().iter().any(|node| {
            node.label
                .contains("unsupported compression or unsynchronisation transform")
        }));
    }

    #[test]
    fn reads_id3v23_text_and_comment_frames() {
        let mut body = frame(b"TIT2", b"\x03A v2.3 title", 3);
        body.extend_from_slice(&frame(b"COMM", b"\0eng\0Comment text", 3));
        let doc = parse_mp3(&tag(3, &body));

        assert_eq!(values(&doc, "title"), ["A v2.3 title"]);
        assert_eq!(values(&doc, "comment"), ["Comment text"]);
        assert!(doc.metadata_complete);
    }

    #[test]
    fn reads_id3v24_text_frames() {
        let doc = parse_mp3(&tag(4, &frame(b"TIT2", b"\x03A v2.4 title", 4)));
        assert_eq!(values(&doc, "title"), ["A v2.4 title"]);
        assert!(doc.metadata_complete);
    }

    #[test]
    fn preserves_duplicate_id3_values_in_source_order() {
        let mut body = frame(b"TPE1", b"\x03First", 4);
        body.extend_from_slice(&frame(b"TPE1", b"\x03Second", 4));
        let doc = parse_mp3(&tag(4, &body));
        assert_eq!(values(&doc, "artist"), ["First", "Second"]);
    }

    #[test]
    fn unsupported_id3_metadata_frames_are_not_reported_complete() {
        // User-defined text can carry arbitrary metadata, so silently
        // skipping it must not make the file look fully inspected.
        let doc = parse_mp3(&tag(4, &frame(b"TXXX", b"\x03Custom\0Value", 4)));
        assert!(!doc.metadata_complete);
        assert!(
            doc.tree.nodes().iter().any(|node| {
                node.label.contains("not inspected") && node.label.contains("TXXX")
            })
        );
    }

    #[test]
    fn reserved_id3_frame_flags_are_reported_as_incomplete() {
        let mut body = frame(b"TIT2", b"\x03A title", 4);
        body[8] = 0x80; // reserved v2.4 status flag
        let doc = parse_mp3(&tag(4, &body));
        assert!(!doc.metadata_complete);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|node| { node.label.contains("reserved ID3 frame flags") })
        );
    }

    #[test]
    fn reads_id3v1_fields_from_the_trailing_tag() {
        let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
        let start = bytes.len();
        bytes.extend_from_slice(b"TAG");
        bytes.extend_from_slice(&padded(b"Legacy title", 30));
        bytes.extend_from_slice(&padded(b"Legacy artist", 30));
        bytes.extend_from_slice(&padded(b"Legacy album", 30));
        bytes.extend_from_slice(b"1999");
        bytes.extend_from_slice(&padded(b"A legacy comment", 28));
        bytes.push(0);
        bytes.push(13);
        bytes.push(17);
        assert_eq!(bytes.len() - start, 128);

        let doc = parse_mp3(&bytes);
        assert_eq!(values(&doc, "title"), ["Legacy title"]);
        assert_eq!(values(&doc, "artist"), ["Legacy artist"]);
        assert_eq!(values(&doc, "album"), ["Legacy album"]);
        assert_eq!(values(&doc, "year"), ["1999"]);
        assert_eq!(values(&doc, "comment"), ["A legacy comment"]);
        assert_eq!(values(&doc, "track"), ["13"]);
        assert!(
            doc.facts
                .iter()
                .all(|f| doc.tree.get(f.node).range.start >= start as u64)
        );
    }

    #[test]
    fn invalid_text_encoding_is_incomplete_and_never_becomes_a_fact() {
        let doc = parse_mp3(&tag(4, &frame(b"TIT2", &[3, 0xff], 4)));
        assert!(values(&doc, "title").is_empty());
        assert!(!doc.metadata_complete);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.kind == crate::model::NodeKind::Error)
        );
    }

    #[test]
    fn invalid_synchsafe_size_is_incomplete() {
        let mut bytes = b"ID3\x04\0\0".to_vec();
        bytes.extend_from_slice(&[0x80, 0, 0, 0]);
        let doc = parse_mp3(&bytes);
        assert!(!doc.metadata_complete);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.kind == crate::model::NodeKind::Error)
        );
    }

    #[test]
    fn truncated_frame_payload_is_incomplete() {
        let mut body = b"TIT2".to_vec();
        body.extend_from_slice(&synchsafe(5));
        body.extend_from_slice(&[0, 0, 3, b'x']);
        let doc = parse_mp3(&tag(4, &body));
        assert!(!doc.metadata_complete);
        assert!(values(&doc, "title").is_empty());
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.kind == crate::model::NodeKind::Error)
        );
    }

    #[test]
    fn oversized_id3_tag_is_not_walked() {
        let mut bytes = b"ID3\x04\0\0".to_vec();
        bytes.extend_from_slice(&synchsafe(16 * 1024 * 1024 + 1));
        let doc = parse_mp3(&bytes);
        assert!(!doc.metadata_complete);
        assert!(doc.facts.is_empty());
    }

    #[test]
    fn recognizes_mpeg_audio_without_metadata_tags() {
        let bytes = [0xff, 0xfb, 0x90, 0x64];
        assert!(is_mp3(&bytes));
        let doc = parse_mp3(&bytes);
        assert!(doc.facts.is_empty());
        assert!(doc.metadata_complete);
    }

    #[test]
    fn ape_metadata_after_audio_is_not_mistaken_for_a_complete_check() {
        for id3v1 in [false, true] {
            let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
            bytes.extend_from_slice(b"APETAGEX");
            bytes.extend_from_slice(&2000u32.to_le_bytes());
            bytes.extend_from_slice(&32u32.to_le_bytes());
            bytes.extend_from_slice(&[0; 16]);
            if id3v1 {
                bytes.extend_from_slice(b"TAG");
                bytes.extend_from_slice(&[0; 125]);
            }
            let doc = parse_mp3(&bytes);
            assert!(!doc.metadata_complete);
            let warning = doc
                .tree
                .nodes()
                .iter()
                .find(|n| n.label == "APE audio metadata is partially inspected")
                .unwrap();
            assert_eq!(warning.range.start, 4);
            assert_eq!(warning.range.len, 32);
            assert!(
                doc.tree
                    .nodes()
                    .iter()
                    .any(|n| n.label == "audio frames (not decoded)" && n.range.len == 4)
            );
        }
    }

    #[test]
    fn malformed_ape_size_still_marks_metadata_incomplete() {
        let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
        bytes.extend_from_slice(b"APETAGEX");
        bytes.extend_from_slice(&[0; 24]);
        assert!(!parse_mp3(&bytes).metadata_complete);
    }

    #[test]
    fn reads_bounded_ape_text_but_keeps_the_overall_check_partial() {
        let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
        let mut item = Vec::new();
        item.extend_from_slice(&5u32.to_le_bytes());
        item.extend_from_slice(&0u32.to_le_bytes());
        item.extend_from_slice(b"Artist\0Alice");
        bytes.extend_from_slice(&item);
        bytes.extend_from_slice(b"APETAGEX");
        bytes.extend_from_slice(&2000u32.to_le_bytes());
        bytes.extend_from_slice(&((item.len() + 32) as u32).to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        let doc = parse_mp3(&bytes);
        assert_eq!(values(&doc, "artist"), ["Alice"]);
        assert!(!doc.metadata_complete);
        let fact = &doc.facts[0];
        assert_eq!(
            &bytes[doc.tree.get(fact.node).range.start as usize..][..5],
            b"Alice"
        );
    }

    #[test]
    fn ape_binary_and_invalid_text_are_not_reported_as_text_facts() {
        for (flags, value) in [(2u32, b"Alice".as_slice()), (0, b"\xfflice".as_slice())] {
            let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
            let mut item = Vec::new();
            item.extend_from_slice(&(value.len() as u32).to_le_bytes());
            item.extend_from_slice(&flags.to_le_bytes());
            item.extend_from_slice(b"Artist\0");
            item.extend_from_slice(value);
            bytes.extend_from_slice(&item);
            bytes.extend_from_slice(b"APETAGEX");
            bytes.extend_from_slice(&2000u32.to_le_bytes());
            bytes.extend_from_slice(&((item.len() + 32) as u32).to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&[0; 12]);
            let doc = parse_mp3(&bytes);
            assert!(doc.facts.is_empty());
            assert!(!doc.metadata_complete);
        }
    }

    #[test]
    fn reads_flac_vorbis_comments_and_preserves_unknown_duplicates() {
        let bytes = flac(&[
            b"TITLE=Sample",
            b"ARTIST=First",
            b"ARTIST=Second",
            b"X-CUSTOM=kept",
        ]);
        let doc = parse(&bytes).expect("FLAC signature is recognized");
        assert_eq!(doc.format, super::AudioFormat::Flac);
        assert_eq!(values(&doc, "title"), ["Sample"]);
        assert_eq!(values(&doc, "artist"), ["First", "Second"]);
        assert_eq!(values(&doc, "audio-tag"), ["X-CUSTOM: kept"]);
        assert_eq!(values(&doc, "encoder"), ["test encoder"]);
        assert!(doc.metadata_complete);
    }

    #[test]
    fn rejects_a_truncated_flac_comment_length_without_panicking() {
        let mut bytes = b"fLaC\x00\0\0\x22".to_vec();
        bytes.extend_from_slice(&[0; 34]);
        bytes.extend_from_slice(b"\x84\0\0\x08\x08\0\0\0\x01x");
        let doc = parse(&bytes).expect("FLAC signature is recognized");
        assert!(!doc.metadata_complete);
        assert!(doc.facts.is_empty());
    }

    #[test]
    fn reads_ogg_vorbis_tags_from_a_page_spanning_comment_packet() {
        let mut fields = vec![b"TITLE=Across pages".to_vec()];
        fields.push([b"DESCRIPTION=".as_slice(), &vec![b'x'; 280]].concat());
        let refs: Vec<&[u8]> = fields.iter().map(Vec::as_slice).collect();
        let comments = [
            b"\x03vorbis".as_slice(),
            &vorbis_comments(b"vorbis encoder", &refs, true),
        ]
        .concat();
        let doc =
            parse(&ogg_packet_fixture("vorbis", &comments)).expect("Ogg signature is recognized");
        assert_eq!(doc.format, super::AudioFormat::OggVorbis);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.label == "Vorbis comment packet")
        );
        assert_eq!(values(&doc, "title"), ["Across pages"]);
        assert_eq!(values(&doc, "encoder"), ["vorbis encoder"]);
        assert!(doc.metadata_complete);
        assert!(doc.facts.iter().any(|f| f.text.len() == 280));
    }

    #[test]
    fn reads_ogg_opus_tags_and_rejects_untrusted_lengths() {
        let opus_tags = [
            b"OpusTags".as_slice(),
            &vorbis_comments(b"opus encoder", &[b"ARTIST=Opus artist"], false),
        ]
        .concat();
        let doc =
            parse(&ogg_packet_fixture("opus", &opus_tags)).expect("Ogg signature is recognized");
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.label == "Opus tags packet")
        );
        assert_eq!(values(&doc, "artist"), ["Opus artist"]);
        assert!(doc.metadata_complete);

        let mut invalid = b"\x03vorbis".to_vec();
        invalid.extend_from_slice(&u32::MAX.to_le_bytes());
        let broken = parse(&ogg_packet_fixture("vorbis", &invalid)).unwrap();
        assert!(!broken.metadata_complete);
        assert!(broken.facts.is_empty());
    }

    #[test]
    fn rejects_a_bad_ogg_page_checksum_before_trusting_its_tags() {
        let tags = [
            b"\x03vorbis".as_slice(),
            &vorbis_comments(b"encoder", &[b"TITLE=Sample"], true),
        ]
        .concat();
        let mut bytes = ogg_packet_fixture("vorbis", &tags);
        let title = bytes
            .windows(b"TITLE=Sample".len())
            .position(|window| window == b"TITLE=Sample")
            .unwrap();
        bytes[title] ^= 1;

        let doc = parse(&bytes).unwrap();
        assert!(!doc.metadata_complete);
        assert!(doc.facts.is_empty());
        assert!(doc.tree.nodes().iter().any(|node| {
            node.label == "invalid Ogg page checksum" && node.kind == crate::model::NodeKind::Error
        }));
    }

    #[test]
    fn checked_in_ogg_fuzz_seeds_have_valid_page_checksums() {
        for name in ["ogg-opus.ogg", "ogg-vorbis.ogg"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fuzz/seeds/parse_audio")
                .join(name);
            let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
            let doc = parse(&bytes).expect("Ogg signature is recognized");
            assert!(doc.metadata_complete, "{name} should have complete tags");
        }
    }

    #[test]
    fn unsupported_ogg_codecs_are_identified_but_not_reported_complete() {
        let bytes = ogg_page(7, 0, 2, &[7], b"\x80theora");
        let doc = parse(&bytes).expect("Ogg container is recognized");
        assert_eq!(doc.format, super::AudioFormat::Ogg);
        assert!(!doc.metadata_complete);
        assert!(doc.facts.is_empty());
    }

    #[test]
    fn mpeg_layers_one_and_two_are_not_misidentified_as_mp3() {
        assert!(is_mp3(&[0xff, 0xfb, 0x90, 0x64])); // MPEG-1 Layer III
        assert!(!is_mp3(&[0xff, 0xff, 0x90, 0x64])); // MPEG-1 Layer I
        assert!(!is_mp3(&[0xff, 0xfd, 0x90, 0x64])); // MPEG-1 Layer II
    }

    #[test]
    fn every_truncation_of_audio_metadata_headers_is_survivable() {
        let mp3 = tag(4, &frame(b"TIT2", b"\x03Sample", 4));
        let flac = flac(&[b"TITLE=Sample"]);
        let vorbis = vorbis_comments(b"encoder", &[b"TITLE=Sample"], true);
        let comments = [b"\x03vorbis".as_slice(), vorbis.as_slice()].concat();
        let ogg = ogg_packet_fixture("vorbis", &comments);

        for bytes in [mp3, flac, ogg] {
            for end in 0..=bytes.len() {
                let _ = parse(&bytes[..end]);
            }
        }
    }
}
