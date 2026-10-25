//! Local file hashing and comparison against published digests.
//!
//! A match means the bytes equal what a source published. It does not prove
//! who made the file, that it is safe, or that a console will accept it.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::catalog::CatalogEntry;
use crate::domain::types::{HashAlgorithm, PackageType};
use crate::domain::version::FirmwareVersion;

const BUF_SIZE: usize = 1024 * 1024;
const PROGRESS_STEP: u64 = 16 * 1024 * 1024;
/// FAT32's per-file limit; larger files can't be a PS5 PUP on a FAT32 stick.
pub const FAT32_MAX_FILE: u64 = 4 * 1024 * 1024 * 1024 - 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalHashes {
    pub size_bytes: u64,
    pub sha256: String,
    pub md5: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HashError {
    #[error("hashing was cancelled")]
    Cancelled,
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// Hash a file in one streaming pass. `progress(done, total)` is called
/// roughly every 16 MiB and once at the end.
pub fn hash_file(
    path: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<LocalHashes, HashError> {
    let mut file = File::open(path)?;
    let total = file.metadata()?.len();
    hash_reader(&mut file, total, cancel, &mut progress)
}

pub fn hash_reader(
    reader: &mut impl Read,
    total: u64,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<LocalHashes, HashError> {
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();
    let mut buf = vec![0u8; BUF_SIZE];
    let mut done: u64 = 0;
    let mut next_report = PROGRESS_STEP;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(HashError::Cancelled);
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        sha.update(&buf[..n]);
        md5.update(&buf[..n]);
        done += n as u64;
        if done >= next_report {
            progress(done, total);
            next_report = done + PROGRESS_STEP;
        }
    }
    progress(done, total);
    Ok(LocalHashes { size_bytes: done, sha256: hex::encode(sha.finalize()), md5: hex::encode(md5.finalize()) })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    Match,
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashCheck {
    pub algorithm: HashAlgorithm,
    pub expected: String,
    pub source_ids: Vec<String>,
    pub outcome: CheckOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeCheck {
    pub expected_bytes: u64,
    pub approximate: bool,
    pub outcome: CheckOutcome,
}

/// Overall result against one catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryVerdict {
    /// Every published digest for the entry matches the file.
    HashMatches,
    /// At least one published digest differs from the file.
    HashMismatch,
    /// The entry has no published digest to compare with.
    NoPublishedHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryCheck {
    pub key: String,
    pub firmware_version: FirmwareVersion,
    pub package_type: PackageType,
    pub variant_label: Option<String>,
    pub hash_checks: Vec<HashCheck>,
    pub size_check: Option<SizeCheck>,
    pub verdict: EntryVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyReport {
    pub file_name: String,
    pub local: LocalHashes,
    pub exceeds_fat32_limit: bool,
    /// Catalog entries with any digest equal to the file's.
    pub matching_entries: Vec<EntryCheck>,
    /// The entry the user picked to check against, if any.
    pub selected: Option<EntryCheck>,
}

pub fn check_entry(local: &LocalHashes, entry: &CatalogEntry) -> EntryCheck {
    let hash_checks: Vec<HashCheck> = entry
        .evidence
        .hash_claims
        .iter()
        .map(|c| {
            let actual = match c.algorithm {
                HashAlgorithm::Md5 => &local.md5,
                HashAlgorithm::Sha256 => &local.sha256,
            };
            HashCheck {
                algorithm: c.algorithm,
                expected: c.value.clone(),
                source_ids: c.source_ids.clone(),
                outcome: if *actual == c.value { CheckOutcome::Match } else { CheckOutcome::Mismatch },
            }
        })
        .collect();
    let size_check = entry.evidence.best_size_bytes.map(|expected| {
        let outcome = if entry.evidence.best_size_is_approximate {
            // Rounded sizes: accept within 1 MiB of the rounded figure.
            if local.size_bytes.abs_diff(expected) <= 1024 * 1024 {
                CheckOutcome::Match
            } else {
                CheckOutcome::Mismatch
            }
        } else if local.size_bytes == expected {
            CheckOutcome::Match
        } else {
            CheckOutcome::Mismatch
        };
        SizeCheck { expected_bytes: expected, approximate: entry.evidence.best_size_is_approximate, outcome }
    });
    // Any mismatching claim makes the verdict a mismatch, even if another
    // source's claim matches: conflicting sources are shown, never resolved
    // silently in favour of the matching one.
    let verdict = if hash_checks.is_empty() {
        EntryVerdict::NoPublishedHash
    } else if hash_checks.iter().all(|c| c.outcome == CheckOutcome::Match) {
        EntryVerdict::HashMatches
    } else {
        EntryVerdict::HashMismatch
    };
    EntryCheck {
        key: entry.key.clone(),
        firmware_version: entry.firmware_version,
        package_type: entry.package_type,
        variant_label: entry.variant_label.clone(),
        hash_checks,
        size_check,
        verdict,
    }
}

pub fn build_report(
    file_name: String,
    local: LocalHashes,
    entries: &[CatalogEntry],
    selected_key: Option<&str>,
) -> VerifyReport {
    let matching_entries = entries
        .iter()
        .filter(|e| {
            e.evidence.hash_claims.iter().any(|c| match c.algorithm {
                HashAlgorithm::Md5 => c.value == local.md5,
                HashAlgorithm::Sha256 => c.value == local.sha256,
            })
        })
        .map(|e| check_entry(&local, e))
        .collect();
    let selected = selected_key.and_then(|k| entries.iter().find(|e| e.key == k)).map(|e| check_entry(&local, e));
    VerifyReport {
        file_name,
        exceeds_fat32_limit: local.size_bytes > FAT32_MAX_FILE,
        local,
        matching_entries,
        selected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::catalog::{CatalogEntry, merge};
    use crate::domain::manifest::SourcesManifest;
    use crate::domain::types::{PackageRecord, PublishedHash};
    use std::io::Cursor;

    // Known digests of the ASCII bytes "abc".
    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const ABC_MD5: &str = "900150983cd24fb0d6963f7d28e17f72";

    fn hashes_of(bytes: &[u8]) -> LocalHashes {
        let cancel = AtomicBool::new(false);
        hash_reader(&mut Cursor::new(bytes.to_vec()), bytes.len() as u64, &cancel, &mut |_, _| {}).unwrap()
    }

    fn entry(claims: &[(HashAlgorithm, &str, &str)], size: Option<u64>) -> CatalogEntry {
        let manifest = SourcesManifest::parse(include_str!("../../manifests/sources.json")).unwrap();
        let records = claims
            .iter()
            .map(|(alg, value, source)| PackageRecord {
                package_id: format!("{source}:x"),
                source_id: (*source).into(),
                firmware_version: "13.60.00".parse().unwrap(),
                long_version: None,
                build_date: None,
                source_section: None,
                raw_package_label: "Firmware 13.60.00".into(),
                package_type: PackageType::Reinstall,
                variant_label: None,
                published_size_bytes: size,
                size_is_approximate: false,
                raw_size_text: None,
                hashes: vec![PublishedHash {
                    algorithm: *alg,
                    value: (*value).into(),
                    source_id: (*source).into(),
                    derived_from: None,
                    note: None,
                }],
                mirrors: vec![],
                last_checked: "t".into(),
            })
            .collect();
        merge(records, &manifest).remove(0)
    }

    #[test]
    fn known_digests() {
        let h = hashes_of(b"abc");
        assert_eq!(h.sha256, ABC_SHA256);
        assert_eq!(h.md5, ABC_MD5);
        assert_eq!(h.size_bytes, 3);
    }

    #[test]
    fn streams_large_input_with_progress() {
        let data = vec![7u8; 40 * 1024 * 1024 + 5];
        let cancel = AtomicBool::new(false);
        let mut calls = 0;
        let h = hash_reader(&mut Cursor::new(data.clone()), data.len() as u64, &cancel, &mut |done, total| {
            calls += 1;
            assert!(done <= total);
        })
        .unwrap();
        assert_eq!(h.size_bytes, data.len() as u64);
        assert!(calls >= 3);
    }

    #[test]
    fn cancel_stops_hashing() {
        let cancel = AtomicBool::new(true);
        let r = hash_reader(&mut Cursor::new(vec![0u8; 10]), 10, &cancel, &mut |_, _| {});
        assert!(matches!(r, Err(HashError::Cancelled)));
    }

    #[test]
    fn hashes_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("PS5UPDATE.PUP");
        std::fs::write(&p, b"abc").unwrap();
        let h = hash_file(&p, &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(h.sha256, ABC_SHA256);
    }

    #[test]
    fn md5_match() {
        let e = entry(&[(HashAlgorithm::Md5, ABC_MD5, "darthsternie-ps5")], Some(3));
        let c = check_entry(&hashes_of(b"abc"), &e);
        assert_eq!(c.verdict, EntryVerdict::HashMatches);
        assert_eq!(c.size_check.unwrap().outcome, CheckOutcome::Match);
    }

    #[test]
    fn md5_mismatch() {
        let e = entry(&[(HashAlgorithm::Md5, &"0".repeat(32), "darthsternie-ps5")], None);
        assert_eq!(check_entry(&hashes_of(b"abc"), &e).verdict, EntryVerdict::HashMismatch);
    }

    #[test]
    fn sha256_match_and_mismatch() {
        let ok = entry(&[(HashAlgorithm::Sha256, ABC_SHA256, "mewx-ps5-fw-list")], None);
        assert_eq!(check_entry(&hashes_of(b"abc"), &ok).verdict, EntryVerdict::HashMatches);
        let bad = entry(&[(HashAlgorithm::Sha256, &"1".repeat(64), "mewx-ps5-fw-list")], None);
        assert_eq!(check_entry(&hashes_of(b"abc"), &bad).verdict, EntryVerdict::HashMismatch);
    }

    #[test]
    fn no_published_hash() {
        let mut e = entry(&[(HashAlgorithm::Md5, ABC_MD5, "darthsternie-ps5")], None);
        e.evidence.hash_claims.clear();
        assert_eq!(check_entry(&hashes_of(b"abc"), &e).verdict, EntryVerdict::NoPublishedHash);
    }

    #[test]
    fn conflicting_sources_are_not_resolved_as_a_match() {
        let e = entry(
            &[
                (HashAlgorithm::Md5, ABC_MD5, "darthsternie-ps5"),
                (HashAlgorithm::Md5, &"f".repeat(32), "mewx-ps5-fw-list"),
            ],
            None,
        );
        assert_eq!(check_entry(&hashes_of(b"abc"), &e).verdict, EntryVerdict::HashMismatch);
    }

    #[test]
    fn size_mismatch_is_reported() {
        let e = entry(&[(HashAlgorithm::Md5, ABC_MD5, "darthsternie-ps5")], Some(4));
        assert_eq!(check_entry(&hashes_of(b"abc"), &e).size_check.unwrap().outcome, CheckOutcome::Mismatch);
    }

    #[test]
    fn report_finds_matching_entries_and_selected() {
        let e = entry(&[(HashAlgorithm::Md5, ABC_MD5, "darthsternie-ps5")], None);
        let r = build_report("x.pup".into(), hashes_of(b"abc"), std::slice::from_ref(&e), Some(&e.key));
        assert_eq!(r.matching_entries.len(), 1);
        assert!(r.selected.is_some());
        assert!(!r.exceeds_fat32_limit);
        let none = build_report("y".into(), hashes_of(b"xyz"), &[e], None);
        assert!(none.matching_entries.is_empty());
    }
}
