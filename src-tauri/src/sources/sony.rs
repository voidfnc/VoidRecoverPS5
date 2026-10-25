//! Sony's own download host for PS5 system software.
//!
//! The public support page links the current update (`sys_`) and recovery
//! (`rec_`) files. Older files are sometimes still served at URLs built from
//! the MewX index (build date + digest); availability is probed with HEAD.

use std::sync::LazyLock;

use regex::Regex;

use super::AdapterError;
use crate::domain::catalog::OfficialLatest;
use crate::domain::types::{
    Availability, HashAlgorithm, Mirror, MirrorMode, MirrorScope, PackageRecord, PackageType, PublishedHash,
};

pub const SOURCE_ID: &str = "sony-ps5-cdn";
pub const SUPPORT_PAGE: &str = "https://www.playstation.com/en-us/support/hardware/ps5/system-software/";
pub const MAX_BYTES: usize = 3 * 1024 * 1024;
const CDN_BASE: &str =
    "https://pc.ps5.update.playstation.net/update/ps5/official/tJMRE80IbXnE9YuG0jzTXgKEjIMoabr6/image";
const URL_HASH_NOTE: &str = "Taken from Sony's download URL. MewX documents this path segment as the file's SHA-256; not yet independently confirmed.";

static CDN_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"https://pc\.ps5\.update\.playstation\.net/update/ps5/official/[A-Za-z0-9]+/image/(\d{4}_\d{4})/(sys|rec)_([0-9a-f]{64})/PS5UPDATE\.PUP"#,
    )
    .expect("static regex")
});

pub fn cdn_url(build_date: &str, package_type: PackageType, sha256: &str) -> Option<String> {
    let prefix = match package_type {
        PackageType::Update => "sys",
        PackageType::Reinstall => "rec",
        _ => return None,
    };
    Some(format!("{CDN_BASE}/{build_date}/{prefix}_{sha256}/PS5UPDATE.PUP"))
}

/// Extract the current update and recovery links from the support page.
/// Exactly one distinct link of each kind must be present.
pub fn parse_support_page(html: &str) -> Result<OfficialLatest, AdapterError> {
    let mut update: Option<(String, String, String)> = None;
    let mut reinstall: Option<(String, String, String)> = None;
    for cap in CDN_LINK.captures_iter(html) {
        let found = (cap[1].to_string(), cap[3].to_string(), cap[0].to_string());
        let slot = if &cap[2] == "sys" { &mut update } else { &mut reinstall };
        match slot {
            Some(existing) if *existing != found => {
                return Err(AdapterError::LayoutChanged(
                    "Sony's support page links more than one distinct file of the same kind".into(),
                ));
            }
            _ => *slot = Some(found),
        }
    }
    let (Some(u), Some(r)) = (update, reinstall) else {
        return Err(AdapterError::LayoutChanged(
            "Sony's support page no longer has both an update and a reinstallation link".into(),
        ));
    };
    if u.0 != r.0 {
        return Err(AdapterError::LayoutChanged(
            "Sony's update and reinstallation links have different build dates".into(),
        ));
    }
    Ok(OfficialLatest {
        build_date: u.0,
        update_sha256: u.1,
        reinstall_sha256: r.1,
        update_url: u.2,
        reinstall_url: r.2,
        version: None,
        page_url: SUPPORT_PAGE.into(),
    })
}

fn sony_mirror(url: String) -> Mirror {
    Mirror {
        host: "Sony CDN".into(),
        url,
        mode: MirrorMode::DirectHttps,
        scope: MirrorScope::File,
        availability: Availability::Unknown,
        probed_at: None,
        availability_note: None,
    }
}

/// Build Sony CDN records for every MewX update/recovery row that has a build
/// date and SHA-256, and resolve the current release's version through the index.
///
/// The current release's records carry the URL digest as a hash claim (Sony's
/// own data). Older URLs are built from MewX values, so they carry no extra
/// hash claim: that would just repeat MewX.
pub fn records_from_index(
    index: &[PackageRecord],
    latest: Option<&mut OfficialLatest>,
    checked_at: &str,
) -> Vec<PackageRecord> {
    let mut latest = latest;
    let mut out = Vec::new();
    for r in index {
        if !matches!(r.package_type, PackageType::Update | PackageType::Reinstall) || r.variant_label.is_some() {
            continue;
        }
        let (Some(build), Some(sha)) = (r.build_date.as_deref(), r.hash(HashAlgorithm::Sha256)) else { continue };
        let Some(url) = cdn_url(build, r.package_type, &sha.value) else { continue };

        let is_current = latest.as_deref().is_some_and(|l| {
            l.build_date == build
                && match r.package_type {
                    PackageType::Update => l.update_sha256 == sha.value,
                    _ => l.reinstall_sha256 == sha.value,
                }
        });
        if is_current && let Some(l) = latest.as_deref_mut() {
            l.version = Some(r.firmware_version);
        }
        let hashes = if is_current {
            vec![PublishedHash {
                algorithm: HashAlgorithm::Sha256,
                value: sha.value.clone(),
                source_id: SOURCE_ID.into(),
                derived_from: None,
                note: Some(URL_HASH_NOTE.into()),
            }]
        } else {
            Vec::new()
        };
        let kind = if r.package_type == PackageType::Update { "update" } else { "recovery" };
        out.push(PackageRecord {
            package_id: format!("{SOURCE_ID}:{}:{kind}", r.firmware_version),
            source_id: SOURCE_ID.into(),
            firmware_version: r.firmware_version,
            long_version: r.long_version.clone(),
            build_date: Some(build.to_string()),
            source_section: Some(if is_current { "Current release (support page)" } else { "Built from index" }.into()),
            raw_package_label: format!("{kind} file, build {build}"),
            package_type: r.package_type,
            variant_label: None,
            published_size_bytes: None,
            size_is_approximate: false,
            raw_size_text: None,
            hashes,
            mirrors: vec![sony_mirror(url)],
            last_checked: checked_at.to_string(),
        });
    }
    out
}

/// Interpret a HEAD response from the Sony CDN. Sony pulls old files by
/// serving them with Content-Length 0.
pub fn classify(
    status: u16,
    content_length: Option<u64>,
    expected_size: Option<u64>,
) -> (Availability, Option<String>) {
    match (status, content_length) {
        (200, Some(0)) => (Availability::Unavailable, Some("Sony serves this file as 0 bytes (pulled)".into())),
        (200, Some(n)) => match expected_size {
            Some(exp) if exp != n => (
                Availability::Unavailable,
                Some(format!("Sony serves {n} bytes but the index lists {exp}; not offered")),
            ),
            _ => (Availability::Available, None),
        },
        (200, None) => (Availability::Unknown, Some("Sony did not report a size".into())),
        (404 | 410, _) => (Availability::Unavailable, Some("No longer served by Sony".into())),
        (s, _) => (Availability::Unknown, Some(format!("Sony answered HTTP {s}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::mewx;

    const PAGE: &str = include_str!("../../../tests/fixtures/sources/sony_support_page.html");
    const MEWX: &str = include_str!("../../../tests/fixtures/sources/mewx_readme.md");

    #[test]
    fn parses_current_links() {
        let l = parse_support_page(PAGE).unwrap();
        assert_eq!(l.build_date, "2026_1014");
        assert_eq!(l.update_sha256, "0f81be6fd890bc04925b3953e0862dfc2ead68edfdc33f31c3c60d0c267f68e4");
        assert!(l.reinstall_url.contains("/rec_549cf0d5"));
    }

    #[test]
    fn missing_link_fails_closed() {
        let only_update = PAGE.replace("/rec_", "/xxx_");
        assert!(matches!(parse_support_page(&only_update), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn two_different_links_of_one_kind_fail_closed() {
        let extra =
            format!("{PAGE} <a href=\"{}\">x</a>", cdn_url("2026_0928", PackageType::Update, &"a".repeat(64)).unwrap());
        assert!(matches!(parse_support_page(&extra), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn builds_urls_and_resolves_current_version() {
        let index = mewx::parse(MEWX, "t").unwrap();
        let mut latest = parse_support_page(PAGE).unwrap();
        let recs = records_from_index(&index, Some(&mut latest), "t");
        assert_eq!(latest.version.map(|v| v.to_string()).as_deref(), Some("14.20.00"));

        let cur = recs.iter().find(|r| r.package_id == "sony-ps5-cdn:14.20.00:recovery").unwrap();
        assert_eq!(cur.mirrors[0].url, latest.reinstall_url);
        assert_eq!(cur.hashes.len(), 1, "current release carries Sony's URL digest");

        let old = recs.iter().find(|r| r.package_id == "sony-ps5-cdn:13.60.00:recovery").unwrap();
        assert!(old.hashes.is_empty(), "built URLs don't repeat MewX's hash");
        assert!(old.mirrors[0].url.contains("/2026_0717/rec_e210451f"));
        assert!(recs.iter().all(|r| r.variant_label.is_none()), "sys_ex rows are not mapped to Sony URLs");
    }

    #[test]
    fn availability_classification() {
        assert_eq!(classify(200, Some(1_404_953_088), Some(1_404_953_088)).0, Availability::Available);
        assert_eq!(classify(200, Some(0), None).0, Availability::Unavailable);
        assert_eq!(classify(404, None, None).0, Availability::Unavailable);
        assert_eq!(classify(200, Some(5), Some(6)).0, Availability::Unavailable);
        assert_eq!(classify(503, None, None).0, Availability::Unknown);
    }

    #[test]
    fn no_url_for_special_variants() {
        assert!(cdn_url("2023_1101", PackageType::SpecialVariant, &"a".repeat(64)).is_none());
    }
}
