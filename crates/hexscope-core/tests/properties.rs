use hexscope_core::inflate::{Decoder, NoTrace, inflate};
use hexscope_core::parse;
use hexscope_core::png::parse_png;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// The central guarantee of the whole crate.
    #[test]
    fn parse_png_always_returns_a_tree(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let doc = parse_png(&bytes);
        prop_assert!(!doc.tree.is_empty());
    }

    /// A valid signature followed by garbage is the nastiest realistic input:
    /// it gets past the sniff and into the chunk walker.
    #[test]
    fn png_signature_plus_garbage_is_survivable(
        tail in proptest::collection::vec(any::<u8>(), 0..4096)
    ) {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&tail);
        let doc = parse_png(&bytes);
        prop_assert!(!doc.tree.is_empty());
    }

    /// The same guarantee through the format dispatcher, for every format.
    #[test]
    fn parse_any_always_returns_a_tree(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        prop_assert!(!parse(&bytes).tree().is_empty());
    }

    /// A JPEG start followed by garbage gets past the sniff and into the
    /// segment walker, and from there into EXIF when the bytes say APP1.
    #[test]
    fn jpeg_start_plus_garbage_is_survivable(
        tail in proptest::collection::vec(any::<u8>(), 0..4096),
        exif in any::<bool>(),
    ) {
        let mut bytes = vec![0xFF, 0xD8, 0xFF];
        if exif {
            let len = (tail.len() as u16).saturating_add(8);
            bytes.push(0xE1);
            bytes.extend_from_slice(&len.to_be_bytes());
            bytes.extend_from_slice(b"Exif\0\0");
        }
        bytes.extend_from_slice(&tail);
        prop_assert!(!parse(&bytes).tree().is_empty());
    }

    #[test]
    fn inflate_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = inflate(&bytes, 1 << 20, &mut NoTrace);
    }

    #[test]
    fn zip_start_plus_garbage_is_survivable(
        rest in proptest::collection::vec(any::<u8>(), 0..4096)
    ) {
        let mut bytes = b"PK\x03\x04".to_vec();
        bytes.extend(rest);
        let doc = hexscope_core::zip::parse_zip(&bytes);
        prop_assert!(doc.tree.root().is_some());
        for e in &doc.entries {
            prop_assert!(e.data.end() <= bytes.len() as u64);
            let _ = hexscope_core::zip::extract(&bytes, e, 1 << 20);
        }
    }

    #[test]
    fn explaining_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let mut d = Decoder::new(&bytes, 1 << 20);
        while let Some(e) = d.explain_next() {
            let _ = d.tables();
            if e.error.is_some() {
                break;
            }
        }
    }
}
