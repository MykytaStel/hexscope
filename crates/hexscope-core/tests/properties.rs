use hexscope_core::inflate::{NoTrace, inflate};
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

    #[test]
    fn inflate_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = inflate(&bytes, 1 << 20, &mut NoTrace);
    }
}
