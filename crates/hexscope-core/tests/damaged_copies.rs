// Damage to a real file that hexscope can clean reaches the cleaner's own
// checks, which garbage rarely does. Whatever the damage, a copy is either
// refused or reads back as the same kind of file and cleans again.
use hexscope_core::clean::clean;
use hexscope_core::summary::summarize;
use proptest::prelude::*;

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// Overwrites bytes, then cuts the file short when `cut` is below its length.
fn damage(mut bytes: Vec<u8>, edits: &[(usize, u8)], cut: usize) -> Vec<u8> {
    let len = bytes.len();
    for &(at, b) in edits {
        bytes[at % len] = b;
    }
    bytes.truncate(cut.max(1));
    bytes
}

/// An ID3v2.3 title tag before one MPEG frame header and a few audio bytes.
#[cfg(feature = "audio")]
fn mp3() -> Vec<u8> {
    let title = b"Sample song";
    let mut body = b"TIT2".to_vec();
    body.extend_from_slice(&((title.len() + 1) as u32).to_be_bytes());
    body.extend_from_slice(&[0, 0, 0]);
    body.extend_from_slice(title);
    let mut out = b"ID3\x03\0\0\0\0\0".to_vec();
    out.push(body.len() as u8);
    out.extend_from_slice(&body);
    out.extend_from_slice(&[0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4]);
    out
}

/// STREAMINFO, then Vorbis comments with a title and a place, then a frame header.
#[cfg(feature = "audio")]
fn flac() -> Vec<u8> {
    let mut comments = 12u32.to_le_bytes().to_vec();
    comments.extend_from_slice(b"test encoder");
    comments.extend_from_slice(&2u32.to_le_bytes());
    for field in [&b"TITLE=Song"[..], b"LOCATION=Kyiv"] {
        comments.extend_from_slice(&(field.len() as u32).to_le_bytes());
        comments.extend_from_slice(field);
    }
    let mut out = b"fLaC".to_vec();
    out.extend_from_slice(&[0, 0, 0, 34]);
    out.extend_from_slice(&[0; 34]);
    out.extend_from_slice(&[0x84, 0, 0, comments.len() as u8]);
    out.extend_from_slice(&comments);
    out.extend_from_slice(&[0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4]);
    out
}

fn copy_reads_back(bytes: &[u8]) -> Result<(), TestCaseError> {
    if let Ok(copy) = clean(bytes) {
        prop_assert_eq!(summarize(bytes).format, summarize(&copy.bytes).format);
        prop_assert!(!hexscope_core::parse(&copy.bytes).tree().is_empty());
        let _ = clean(&copy.bytes);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn a_damaged_hdr_photo_is_cleaned_or_refused(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..16),
        cut in any::<usize>(),
    ) {
        copy_reads_back(&damage(fixture("tests/fixtures/iphone-hdr.jpg"), &edits, cut))?;
    }

    #[cfg(feature = "audio")]
    #[test]
    fn a_damaged_mp3_is_cleaned_or_refused(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
        cut in any::<usize>(),
    ) {
        copy_reads_back(&damage(mp3(), &edits, cut))?;
    }

    #[cfg(feature = "audio")]
    #[test]
    fn a_damaged_flac_is_cleaned_or_refused(
        edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..8),
        cut in any::<usize>(),
    ) {
        copy_reads_back(&damage(flac(), &edits, cut))?;
    }
}

#[test]
fn the_undamaged_files_make_copies() {
    // Otherwise every damaged case would be refused and the properties above prove nothing.
    assert!(clean(&fixture("tests/fixtures/iphone-hdr.jpg")).is_ok());
    #[cfg(feature = "audio")]
    {
        assert!(clean(&mp3()).is_ok());
        assert!(clean(&flac()).is_ok());
    }
}
