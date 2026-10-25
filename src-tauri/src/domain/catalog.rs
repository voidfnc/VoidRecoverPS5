//! Merge per-source package records into one catalog entry per
//! (version, package type, variant), with transparent cross-source evidence.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::manifest::SourcesManifest;
use super::types::{Availability, HashAlgorithm, PackageRecord, PackageType};
use super::version::FirmwareVersion;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashClaim {
    pub algorithm: HashAlgorithm,
    pub value: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossSourceMatch {
    pub algorithm: HashAlgorithm,
    pub value: String,
    pub source_a: String,
    pub source_b: String,
    /// False when one source is recorded as derived from the other.
    pub independent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeClaim {
    pub bytes: u64,
    pub approximate: bool,
    pub source_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub hash_claims: Vec<HashClaim>,
    pub sha256_conflict: bool,
    pub md5_conflict: bool,
    pub sizes: Vec<SizeClaim>,
    /// Two exact sizes disagree.
    pub size_conflict: bool,
    pub cross_source: Vec<CrossSourceMatch>,
    /// Exact size if any source gives one, else the first approximate size.
    pub best_size_bytes: Option<u64>,
    pub best_size_is_approximate: bool,
    /// Availability of the Sony CDN copy, if a Sony record exists.
    pub sony_availability: Option<Availability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub key: String,
    pub firmware_version: FirmwareVersion,
    pub package_type: PackageType,
    pub variant_label: Option<String>,
    /// Sony build date when the records state one. Sony has shipped more than
    /// one build under the same version number; each build is its own entry.
    pub build_date: Option<String>,
    pub records: Vec<PackageRecord>,
    pub evidence: Evidence,
    /// Other entries that list a file with the same checksum, e.g. a source
    /// that names the second 08.20.00 build "08.20.02".
    pub also_listed_as: Vec<EntryRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryRef {
    pub key: String,
    pub firmware_version: FirmwareVersion,
    pub package_type: PackageType,
    pub variant_label: Option<String>,
    pub build_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficialLatest {
    pub build_date: String,
    pub update_sha256: String,
    pub reinstall_sha256: String,
    pub update_url: String,
    pub reinstall_url: String,
    /// Resolved through the MewX index; None when the index doesn't know the build yet.
    pub version: Option<FirmwareVersion>,
    pub page_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceBehind {
    pub source_id: String,
    pub newest_version: FirmwareVersion,
    pub official_latest: FirmwareVersion,
}

pub fn entry_key(version: FirmwareVersion, package_type: PackageType, variant: Option<&str>) -> String {
    let t = serde_json::to_value(package_type).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
    match variant {
        Some(v) => format!("{version}|{t}|{v}"),
        None => format!("{version}|{t}"),
    }
}

/// Whether two sources' agreement counts as independent.
pub fn is_independent(manifest: &SourcesManifest, a: &str, b: &str) -> bool {
    let derived = |x: &str, y: &str| manifest.source(x).is_some_and(|s| s.derived_from.iter().any(|d| d == y));
    !(derived(a, b) || derived(b, a))
}

pub fn merge(records: Vec<PackageRecord>, manifest: &SourcesManifest) -> Vec<CatalogEntry> {
    let mut groups: BTreeMap<(FirmwareVersion, PackageType, Option<String>), Vec<PackageRecord>> = BTreeMap::new();
    for r in records {
        groups.entry((r.firmware_version, r.package_type, r.variant_label.clone())).or_default().push(r);
    }
    let mut entries = Vec::new();
    for ((version, package_type, variant), records) in groups {
        let base_key = entry_key(version, package_type, variant.as_deref());
        let builds = split_by_build(records);
        let split = builds.len() > 1;
        for (build_date, records) in builds {
            let key = match (&build_date, split) {
                (_, false) => base_key.clone(),
                (Some(b), true) => format!("{base_key}|{b}"),
                (None, true) => format!("{base_key}|build-not-stated"),
            };
            let evidence = evidence_for(&records, manifest);
            entries.push(CatalogEntry {
                key,
                firmware_version: version,
                package_type,
                variant_label: variant.clone(),
                build_date,
                records,
                evidence,
                also_listed_as: Vec::new(),
            });
        }
    }
    link_same_files(&mut entries);
    // Newest first; within a version: update, reinstall, special, unknown; newest build first.
    entries.sort_by(|a, b| {
        b.firmware_version
            .cmp(&a.firmware_version)
            .then(a.package_type.cmp(&b.package_type))
            .then(a.variant_label.cmp(&b.variant_label))
            .then(b.build_date.cmp(&a.build_date))
    });
    entries
}

/// Split one (version, type, variant) group by Sony build date when the
/// records name more than one build. Records without a build date join the
/// build they share a checksum with, or a "build not stated" group.
fn split_by_build(records: Vec<PackageRecord>) -> Vec<(Option<String>, Vec<PackageRecord>)> {
    let mut builds: Vec<String> = records.iter().filter_map(|r| r.build_date.clone()).collect();
    builds.sort();
    builds.dedup();
    if builds.len() <= 1 {
        return vec![(builds.pop(), records)];
    }
    let (dated, undated): (Vec<_>, Vec<_>) = records.into_iter().partition(|r| r.build_date.is_some());
    let mut buckets: Vec<(Option<String>, Vec<PackageRecord>)> =
        builds.into_iter().rev().map(|b| (Some(b), Vec::new())).collect();
    for r in dated {
        let slot = buckets.iter_mut().find(|(b, _)| b == &r.build_date).expect("build collected above");
        slot.1.push(r);
    }
    let mut unplaced = Vec::new();
    for r in undated {
        let shares = |bucket: &[PackageRecord]| {
            bucket.iter().any(|o| {
                o.hashes.iter().any(|h| r.hashes.iter().any(|x| x.algorithm == h.algorithm && x.value == h.value))
            })
        };
        let matching: Vec<usize> = buckets.iter().enumerate().filter(|(_, (_, b))| shares(b)).map(|(i, _)| i).collect();
        match matching.as_slice() {
            [only] => buckets[*only].1.push(r),
            _ => unplaced.push(r),
        }
    }
    if !unplaced.is_empty() {
        buckets.push((None, unplaced));
    }
    buckets
}

/// Point entries at other entries that publish the same checksum.
fn link_same_files(entries: &mut [CatalogEntry]) {
    let mut by_hash: BTreeMap<(HashAlgorithm, String), Vec<usize>> = BTreeMap::new();
    for (i, e) in entries.iter().enumerate() {
        for c in &e.evidence.hash_claims {
            by_hash.entry((c.algorithm, c.value.clone())).or_default().push(i);
        }
    }
    let mut links: Vec<Vec<usize>> = vec![Vec::new(); entries.len()];
    for idxs in by_hash.values() {
        for &a in idxs {
            for &b in idxs {
                if a != b && !links[a].contains(&b) {
                    links[a].push(b);
                }
            }
        }
    }
    let refs: Vec<EntryRef> = entries
        .iter()
        .map(|e| EntryRef {
            key: e.key.clone(),
            firmware_version: e.firmware_version,
            package_type: e.package_type,
            variant_label: e.variant_label.clone(),
            build_date: e.build_date.clone(),
        })
        .collect();
    for (i, e) in entries.iter_mut().enumerate() {
        e.also_listed_as = links[i].iter().map(|&j| refs[j].clone()).collect();
    }
}

fn evidence_for(records: &[PackageRecord], manifest: &SourcesManifest) -> Evidence {
    let mut hash_claims: Vec<HashClaim> = Vec::new();
    for r in records {
        for h in &r.hashes {
            match hash_claims.iter_mut().find(|c| c.algorithm == h.algorithm && c.value == h.value) {
                Some(c) => {
                    if !c.source_ids.contains(&h.source_id) {
                        c.source_ids.push(h.source_id.clone());
                    }
                }
                None => hash_claims.push(HashClaim {
                    algorithm: h.algorithm,
                    value: h.value.clone(),
                    source_ids: vec![h.source_id.clone()],
                }),
            }
        }
    }
    let distinct = |alg| hash_claims.iter().filter(|c| c.algorithm == alg).count();
    let sha256_conflict = distinct(HashAlgorithm::Sha256) > 1;
    let md5_conflict = distinct(HashAlgorithm::Md5) > 1;

    let mut cross_source = Vec::new();
    for c in &hash_claims {
        for (i, a) in c.source_ids.iter().enumerate() {
            for b in &c.source_ids[i + 1..] {
                cross_source.push(CrossSourceMatch {
                    algorithm: c.algorithm,
                    value: c.value.clone(),
                    source_a: a.clone(),
                    source_b: b.clone(),
                    independent: is_independent(manifest, a, b),
                });
            }
        }
    }

    let sizes: Vec<SizeClaim> = records
        .iter()
        .filter_map(|r| {
            r.published_size_bytes.map(|bytes| SizeClaim {
                bytes,
                approximate: r.size_is_approximate,
                source_id: r.source_id.clone(),
            })
        })
        .collect();
    let mut exact: Vec<u64> = sizes.iter().filter(|s| !s.approximate).map(|s| s.bytes).collect();
    exact.sort_unstable();
    exact.dedup();
    let size_conflict = exact.len() > 1;
    let (best_size_bytes, best_size_is_approximate) = match exact.first() {
        Some(b) => (Some(*b), false),
        None => (sizes.first().map(|s| s.bytes), !sizes.is_empty()),
    };

    let sony_availability = records
        .iter()
        .filter(|r| r.source_id == crate::sources::sony::SOURCE_ID)
        .flat_map(|r| r.mirrors.iter())
        .map(|m| m.availability)
        .next();

    Evidence {
        hash_claims,
        sha256_conflict,
        md5_conflict,
        sizes,
        size_conflict,
        cross_source,
        best_size_bytes,
        best_size_is_approximate,
        sony_availability,
    }
}

/// Sources whose newest listed version is older than Sony's current release.
pub fn sources_behind(entries: &[CatalogEntry], latest: Option<&OfficialLatest>) -> Vec<SourceBehind> {
    let Some(official) = latest.and_then(|l| l.version) else { return Vec::new() };
    let mut newest: BTreeMap<&str, FirmwareVersion> = BTreeMap::new();
    for e in entries {
        for r in &e.records {
            let slot = newest.entry(r.source_id.as_str()).or_insert(r.firmware_version);
            if r.firmware_version > *slot {
                *slot = r.firmware_version;
            }
        }
    }
    newest
        .into_iter()
        .filter(|(id, v)| *v < official && *id != crate::sources::sony::SOURCE_ID)
        .map(|(id, v)| SourceBehind { source_id: id.to_string(), newest_version: v, official_latest: official })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::PublishedHash;

    fn manifest() -> SourcesManifest {
        SourcesManifest::parse(include_str!("../../../manifests/sources.json")).unwrap()
    }

    fn rec(source: &str, version: &str, t: PackageType, md5: &str, size: Option<(u64, bool)>) -> PackageRecord {
        PackageRecord {
            package_id: format!("{source}:{version}:{t:?}"),
            source_id: source.into(),
            firmware_version: version.parse().unwrap(),
            long_version: None,
            build_date: None,
            source_section: None,
            raw_package_label: format!("Firmware {version}"),
            package_type: t,
            variant_label: None,
            published_size_bytes: size.map(|s| s.0),
            size_is_approximate: size.is_some_and(|s| s.1),
            raw_size_text: None,
            hashes: vec![PublishedHash {
                algorithm: HashAlgorithm::Md5,
                value: md5.into(),
                source_id: source.into(),
                derived_from: None,
                note: None,
            }],
            mirrors: vec![],
            last_checked: "t".into(),
        }
    }

    #[test]
    fn merges_by_version_and_type_and_marks_derived_agreement() {
        let m = manifest();
        let md5 = "a".repeat(32);
        let entries = merge(
            vec![
                rec("darthsternie-ps5", "13.60.00", PackageType::Reinstall, &md5, None),
                rec("mewx-ps5-fw-list", "13.60.00", PackageType::Reinstall, &md5, Some((1_404_953_088, false))),
                rec("mewx-ps5-fw-list", "13.60.00", PackageType::Update, &"b".repeat(32), None),
            ],
            &m,
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].package_type, PackageType::Update);
        let re = &entries[1];
        assert_eq!(re.records.len(), 2);
        assert!(!re.evidence.md5_conflict);
        assert_eq!(re.evidence.cross_source.len(), 1);
        assert!(!re.evidence.cross_source[0].independent, "MewX is derived from Darthsternie");
        assert_eq!(re.evidence.best_size_bytes, Some(1_404_953_088));
    }

    #[test]
    fn rebuilt_versions_split_by_build_and_link_renamed_listings() {
        use crate::sources::{darthsternie, mewx};
        let mut records = mewx::parse(include_str!("../../../tests/fixtures/sources/mewx_readme.md"), "t").unwrap();
        records.extend(
            darthsternie::parse_rest_json(include_str!("../../../tests/fixtures/sources/darthsternie_page.json"), "t")
                .unwrap()
                .records,
        );
        let entries = merge(records, &manifest());
        let v820: Vec<&CatalogEntry> = entries
            .iter()
            .filter(|e| e.firmware_version.to_string() == "08.20.00" && e.package_type == PackageType::Update)
            .collect();
        assert_eq!(v820.len(), 2, "two Sony builds of 08.20.00");
        assert!(v820.iter().all(|e| !e.evidence.md5_conflict && !e.evidence.sha256_conflict));

        let first = v820.iter().find(|e| e.build_date.as_deref() == Some("2023_1020")).unwrap();
        assert!(
            first.records.iter().any(|r| r.source_id == "darthsternie-ps5"),
            "Darthsternie's 08.20.00 joins the build with the same MD5"
        );
        let second = v820.iter().find(|e| e.build_date.as_deref() == Some("2023_1101")).unwrap();
        assert!(second.key.ends_with("|2023_1101"));
        assert!(
            second.also_listed_as.iter().any(|r| r.firmware_version.to_string() == "08.20.02"),
            "Darthsternie names this build 08.20.02"
        );

        let conflicts = entries.iter().filter(|e| e.evidence.md5_conflict || e.evidence.sha256_conflict).count();
        assert_eq!(conflicts, 0, "fixture sources agree once builds are separated");
    }

    #[test]
    fn single_build_keeps_plain_key() {
        let mut r = rec("mewx-ps5-fw-list", "13.60.00", PackageType::Reinstall, &"a".repeat(32), None);
        r.build_date = Some("2026_0717".into());
        let entries = merge(vec![r], &manifest());
        assert_eq!(entries[0].key, "13.60.00|reinstall");
        assert_eq!(entries[0].build_date.as_deref(), Some("2026_0717"));
    }

    #[test]
    fn conflicting_hashes_are_flagged_not_resolved() {
        let entries = merge(
            vec![
                rec("darthsternie-ps5", "14.00.00", PackageType::Reinstall, &"a".repeat(32), None),
                rec("mewx-ps5-fw-list", "14.00.00", PackageType::Reinstall, &"c".repeat(32), None),
            ],
            &manifest(),
        );
        assert!(entries[0].evidence.md5_conflict);
        assert_eq!(entries[0].evidence.hash_claims.len(), 2);
        assert!(entries[0].evidence.cross_source.is_empty());
    }

    #[test]
    fn sorted_newest_first() {
        let entries = merge(
            vec![
                rec("darthsternie-ps5", "9.60.00", PackageType::Update, &"a".repeat(32), None),
                rec("darthsternie-ps5", "10.01.00", PackageType::Update, &"b".repeat(32), None),
            ],
            &manifest(),
        );
        assert_eq!(entries[0].firmware_version.to_string(), "10.01.00");
    }

    #[test]
    fn approximate_size_used_only_without_exact() {
        let entries = merge(
            vec![rec("darthsternie-ps5", "8.20.00", PackageType::Update, &"a".repeat(32), Some((850_000_000, true)))],
            &manifest(),
        );
        assert!(entries[0].evidence.best_size_is_approximate);
        assert!(!entries[0].evidence.size_conflict);
    }

    #[test]
    fn behind_flag_compares_against_official_latest() {
        let entries =
            merge(vec![rec("darthsternie-ps5", "14.10.00", PackageType::Update, &"a".repeat(32), None)], &manifest());
        let latest = OfficialLatest {
            build_date: "2026_1014".into(),
            update_sha256: "0".repeat(64),
            reinstall_sha256: "1".repeat(64),
            update_url: String::new(),
            reinstall_url: String::new(),
            version: Some("14.20.00".parse().unwrap()),
            page_url: String::new(),
        };
        let behind = sources_behind(&entries, Some(&latest));
        assert_eq!(behind.len(), 1);
        assert_eq!(behind[0].source_id, "darthsternie-ps5");
        assert!(sources_behind(&entries, None).is_empty());
    }
}
