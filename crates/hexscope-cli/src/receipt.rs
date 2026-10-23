//! Local clean-copy fingerprints and strict offline receipt checking.

use hexscope_core::{
    crypto::sha2::Sha256,
    receipt::{ReceiptError, copy_receipt_json},
    verification::{VerificationItem, VerificationReason, VerificationReport},
};
use serde::Deserialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::ExitCode,
};

const HASH_BUFFER: usize = 64 * 1024;
const MAX_RECEIPT_BYTES: usize = 16 * 1024 * 1024;

pub fn write_copy_receipt(
    receipt_path: &Path,
    copy_sha256: &str,
    format: &str,
    report: &VerificationReport,
) -> Result<(), String> {
    let json =
        copy_receipt_json(format, Some(copy_sha256), None, report).map_err(format_receipt_error)?;
    if json.len().saturating_add(1) > MAX_RECEIPT_BYTES {
        return Err("verification receipt exceeds the 16 MiB limit".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(receipt_path)
        .map_err(|_| "receipt destination already exists or cannot be created".to_string())?;
    if file
        .write_all(json.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = fs::remove_file(receipt_path);
        return Err("could not write verification receipt".into());
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let [command, copy, receipt] = args else {
        return Err("usage: hexscope receipt verify COPY RECEIPT.json".into());
    };
    if command != "verify" {
        return Err("usage: hexscope receipt verify COPY RECEIPT.json".into());
    }
    verify_copy(Path::new(copy), Path::new(receipt))
}

fn verify_copy(copy: &Path, receipt_path: &Path) -> Result<ExitCode, String> {
    let bytes = read_receipt(receipt_path)?;
    let receipt: Receipt = serde_json::from_slice(&bytes)
        .map_err(|_| "malformed or unsupported receipt".to_string())?;
    let digest = receipt.validate()?;
    let actual = hash_file(copy)?;
    if actual == digest {
        println!(
            "Copy bytes match this receipt. This does not verify who made it and does not certify safety."
        );
        Ok(ExitCode::SUCCESS)
    } else {
        println!("Copy bytes do not match this receipt.");
        Ok(ExitCode::from(1))
    }
}

fn read_receipt(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| "could not read receipt".to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_RECEIPT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "could not read receipt".to_string())?;
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err("receipt exceeds the 16 MiB limit".into());
    }
    Ok(bytes)
}

pub(crate) fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|_| "could not read copy".to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; HASH_BUFFER];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| "could not read copy".to_string())?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hex(&hash.finalize()))
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hex(&hash.finalize())
}

/// A private, stable copy of the output bytes used for both receipt hashing
/// and the subsequent parser read-back.
pub(crate) struct CopySnapshot {
    file: tempfile::NamedTempFile,
    sha256: String,
}

impl CopySnapshot {
    pub(crate) fn path(&self) -> &Path {
        self.file.path()
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }
}

pub(crate) fn snapshot_copy(path: &Path) -> Result<CopySnapshot, String> {
    let mut source = File::open(path).map_err(|_| "could not read copy".to_string())?;
    let mut snapshot = tempfile::NamedTempFile::new()
        .map_err(|_| "could not create verification snapshot".to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; HASH_BUFFER];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| "could not read copy".to_string())?;
        if read == 0 {
            break;
        }
        snapshot
            .write_all(&buffer[..read])
            .map_err(|_| "could not create verification snapshot".to_string())?;
        hash.update(&buffer[..read]);
    }
    snapshot
        .flush()
        .map_err(|_| "could not create verification snapshot".to_string())?;
    Ok(CopySnapshot {
        file: snapshot,
        sha256: hex(&hash.finalize()),
    })
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

fn format_receipt_error(error: ReceiptError) -> String {
    error.to_string()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    version: u8,
    hexscope_version: String,
    copy: Copy,
    verification: Verification,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Copy {
    format: String,
    sha256: Option<String>,
    fingerprint_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Verification {
    schema_version: u8,
    removed: Vec<Item>,
    present: Vec<Item>,
    unchecked: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    kind: String,
    reason: String,
    unexpected: bool,
}

impl Receipt {
    fn validate(&self) -> Result<&str, String> {
        if self.schema != "hexscope.clean-copy-receipt"
            || self.version != 1
            || self.hexscope_version.is_empty()
            || self.verification.schema_version != 1
        {
            return Err("malformed or unsupported receipt".into());
        }

        let report = self.verification.to_report()?;
        match (
            self.copy.sha256.as_deref(),
            self.copy.fingerprint_reason.as_deref(),
        ) {
            (Some(digest), None) => {
                copy_receipt_json(&self.copy.format, Some(digest), None, &report)
                    .map_err(|_| "malformed or unsupported receipt".to_string())?;
                Ok(digest)
            }
            (None, Some("size_limit")) | (None, None) => {
                Err("receipt has no copy fingerprint".into())
            }
            _ => Err("malformed or unsupported receipt".into()),
        }
    }
}

impl Verification {
    fn to_report(&self) -> Result<VerificationReport, String> {
        let map = |items: &[Item], group: Group| {
            items
                .iter()
                .map(|item| {
                    let reason = reason(&item.reason)?;
                    let valid_group = match group {
                        Group::Removed => reason == VerificationReason::Removed,
                        Group::Present => matches!(
                            reason,
                            VerificationReason::StillPresent
                                | VerificationReason::ValueChangedSameKind
                                | VerificationReason::UnexpectedOutput
                        ),
                        Group::Unchecked => matches!(
                            reason,
                            VerificationReason::NoStableValue
                                | VerificationReason::CoverageIncomplete
                                | VerificationReason::ParseIncomplete
                                | VerificationReason::NoComparableFindings
                                | VerificationReason::VerificationSkipped
                        ),
                    };
                    if !valid_group
                        || item.unexpected != (reason == VerificationReason::UnexpectedOutput)
                    {
                        return None;
                    }
                    Some(VerificationItem {
                        kind: item.kind.clone(),
                        reason,
                        unexpected: item.unexpected,
                    })
                })
                .collect::<Option<Vec<_>>>()
        };
        Ok(VerificationReport {
            schema_version: self.schema_version,
            removed: map(&self.removed, Group::Removed)
                .ok_or_else(|| "malformed or unsupported receipt".to_string())?,
            present: map(&self.present, Group::Present)
                .ok_or_else(|| "malformed or unsupported receipt".to_string())?,
            unchecked: map(&self.unchecked, Group::Unchecked)
                .ok_or_else(|| "malformed or unsupported receipt".to_string())?,
        })
    }
}

#[derive(Clone, Copy)]
enum Group {
    Removed,
    Present,
    Unchecked,
}

fn reason(code: &str) -> Option<VerificationReason> {
    match code {
        "removed" => Some(VerificationReason::Removed),
        "still_present" => Some(VerificationReason::StillPresent),
        "value_changed_same_kind" => Some(VerificationReason::ValueChangedSameKind),
        "no_stable_value" => Some(VerificationReason::NoStableValue),
        "coverage_incomplete" => Some(VerificationReason::CoverageIncomplete),
        "parse_incomplete" => Some(VerificationReason::ParseIncomplete),
        "unexpected_output" => Some(VerificationReason::UnexpectedOutput),
        "no_comparable_findings" => Some(VerificationReason::NoComparableFindings),
        "verification_skipped" => Some(VerificationReason::VerificationSkipped),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn snapshot_copy_hashes_the_same_bytes_it_preserves_for_verification() {
        let mut source = tempfile::NamedTempFile::new().unwrap();
        source.write_all(b"verified snapshot").unwrap();
        let snapshot = snapshot_copy(source.path()).unwrap();

        source.as_file_mut().set_len(0).unwrap();
        source.write_all(b"changed after snapshot").unwrap();

        assert_eq!(
            std::fs::read(snapshot.path()).unwrap(),
            b"verified snapshot"
        );
        assert_eq!(snapshot.sha256(), hash_file(snapshot.path()).unwrap());
    }

    #[test]
    fn file_hash_streams_across_multiple_fixed_size_buffers() {
        let bytes: Vec<u8> = (0..HASH_BUFFER * 3 + 17)
            .map(|index| (index * 19) as u8)
            .collect();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();

        assert_eq!(
            hash_file(file.path()).unwrap(),
            hex(&hexscope_core::crypto::sha2::sha256(&bytes))
        );
    }

    #[test]
    fn in_memory_hash_matches_streaming_file_hash() {
        let bytes = b"receipt fingerprint";
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();

        assert_eq!(hash_bytes(bytes), hash_file(file.path()).unwrap());
    }

    #[test]
    fn large_generated_receipts_remain_readable_by_the_offline_verifier() {
        let report = VerificationReport {
            schema_version: 1,
            removed: (0..800)
                .map(|index| VerificationItem {
                    kind: format!("metadata-kind-{index:04}-{}", "x".repeat(80)),
                    reason: VerificationReason::Removed,
                    unexpected: false,
                })
                .collect(),
            present: Vec::new(),
            unchecked: Vec::new(),
        };
        let receipt_file = tempfile::tempdir().unwrap();
        let receipt_path = receipt_file.path().join("receipt.json");

        write_copy_receipt(&receipt_path, &"a".repeat(64), "zip", &report).unwrap();

        assert!(std::fs::metadata(&receipt_path).unwrap().len() > 64 * 1024);
        let bytes = read_receipt(&receipt_path).unwrap();
        let receipt: Receipt = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(receipt.validate().unwrap(), "a".repeat(64));
    }

    #[test]
    fn generated_receipt_at_the_byte_limit_includes_its_newline() {
        let mut report = VerificationReport {
            schema_version: 1,
            removed: vec![VerificationItem {
                kind: String::new(),
                reason: VerificationReason::Removed,
                unexpected: false,
            }],
            present: Vec::new(),
            unchecked: Vec::new(),
        };
        let empty_json = copy_receipt_json("zip", Some(&"a".repeat(64)), None, &report).unwrap();
        report.removed[0].kind = "x".repeat(MAX_RECEIPT_BYTES - empty_json.len() - 1);
        let receipt_file = tempfile::tempdir().unwrap();
        let receipt_path = receipt_file.path().join("receipt.json");

        write_copy_receipt(&receipt_path, &"a".repeat(64), "zip", &report).unwrap();

        let bytes = read_receipt(&receipt_path).unwrap();
        assert_eq!(bytes.len(), MAX_RECEIPT_BYTES);
        assert_eq!(bytes.last(), Some(&b'\n'));
    }

    #[test]
    fn receipt_reader_rejects_one_byte_over_the_limit() {
        let receipt_file = tempfile::NamedTempFile::new().unwrap();
        receipt_file
            .as_file()
            .set_len((MAX_RECEIPT_BYTES + 1) as u64)
            .unwrap();

        assert_eq!(
            read_receipt(receipt_file.path()).unwrap_err(),
            "receipt exceeds the 16 MiB limit"
        );
    }

    #[test]
    fn oversized_generated_receipts_fail_before_creating_the_destination() {
        let report = VerificationReport {
            schema_version: 1,
            removed: vec![VerificationItem {
                kind: "x".repeat(MAX_RECEIPT_BYTES),
                reason: VerificationReason::Removed,
                unexpected: false,
            }],
            present: Vec::new(),
            unchecked: Vec::new(),
        };
        let receipt_file = tempfile::tempdir().unwrap();
        let receipt_path = receipt_file.path().join("receipt.json");

        let result = write_copy_receipt(&receipt_path, &"a".repeat(64), "zip", &report);

        assert_eq!(
            result.unwrap_err(),
            "verification receipt exceeds the 16 MiB limit"
        );
        assert!(!receipt_path.exists());
    }

    #[test]
    fn verification_receipt_rejects_reason_codes_in_the_wrong_group() {
        for raw in [
            r#"{"schema_version":1,"removed":[{"kind":"location","reason":"still_present","unexpected":false}],"present":[],"unchecked":[]}"#,
            r#"{"schema_version":1,"removed":[],"present":[{"kind":"location","reason":"removed","unexpected":false}],"unchecked":[]}"#,
            r#"{"schema_version":1,"removed":[],"present":[],"unchecked":[{"kind":"location","reason":"unexpected_output","unexpected":true}]}"#,
            r#"{"schema_version":1,"removed":[],"present":[{"kind":"location","reason":"unexpected_output","unexpected":false}],"unchecked":[]}"#,
        ] {
            let verification: Verification = serde_json::from_str(raw).unwrap();
            assert!(verification.to_report().is_err(), "accepted {raw}");
        }
    }
}
