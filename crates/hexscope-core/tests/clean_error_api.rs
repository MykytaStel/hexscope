use hexscope_core::clean::CleanError;

#[test]
fn clean_error_keeps_its_public_variants_exhaustive() {
    let errors = [
        CleanError::NothingToRemove,
        CleanError::Encrypted,
        CleanError::Zip64,
        CleanError::Damaged,
        CleanError::Locked,
        CleanError::Unreadable,
        CleanError::Unsupported,
        CleanError::PictureUnderBox,
        CleanError::JpegUnderBox,
        CleanError::FormContentIncomplete,
    ];

    for error in errors {
        let reason = match error {
            CleanError::NothingToRemove
            | CleanError::Encrypted
            | CleanError::Zip64
            | CleanError::Damaged
            | CleanError::Locked
            | CleanError::Unreadable
            | CleanError::Unsupported
            | CleanError::PictureUnderBox
            | CleanError::JpegUnderBox
            | CleanError::FormContentIncomplete => error.reason(),
        };
        assert!(!reason.is_empty());
    }
}
