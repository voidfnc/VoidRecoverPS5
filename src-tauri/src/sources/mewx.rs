//! Adapter for the MewX "PS5 official firmware list" README tables.
//! <https://github.com/MewX/ps5-official-firmware-list> (Apache-2.0)

use super::AdapterError;
use crate::domain::types::{HashAlgorithm, PackageRecord, PackageType, PublishedHash, normalize_hex, parse_size_text};

pub const SOURCE_ID: &str = "mewx-ps5-fw-list";
pub const RAW_URL: &str = "https://raw.githubusercontent.com/MewX/ps5-official-firmware-list/main/README.md";
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

const EXPECTED_HEADER: [&str; 7] =
    ["Firmware Version (Long)", "Version (Short)", "Type", "Build Date", "sha256", "md5 checksum", "size"];

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

pub fn is_build_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 9 && b[4] == b'_' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
}

/// Parse every firmware table in the README. Fails closed if any table's
/// header differs from the known layout or a digest column holds a non-digest.
pub fn parse(markdown: &str, checked_at: &str) -> Result<Vec<PackageRecord>, AdapterError> {
    let mut records = Vec::new();
    let mut tables = 0;
    let mut in_table = false;
    for (idx, line) in markdown.lines().enumerate() {
        let lineno = idx + 1;
        let t = line.trim();
        if !t.starts_with('|') {
            in_table = false;
            continue;
        }
        if is_separator(t) {
            continue;
        }
        let row = cells(t);
        if !in_table {
            // First row of a table is its header.
            in_table = true;
            let looks_like_fw_table = row.iter().any(|c| c.eq_ignore_ascii_case("sha256"));
            if !looks_like_fw_table {
                in_table = false;
                continue;
            }
            if row != EXPECTED_HEADER {
                return Err(AdapterError::LayoutChanged(format!(
                    "MewX table header at line {lineno} is {row:?}, expected {EXPECTED_HEADER:?}"
                )));
            }
            tables += 1;
            continue;
        }
        records.push(parse_row(&row, lineno, checked_at)?);
    }
    if tables == 0 {
        return Err(AdapterError::LayoutChanged("no firmware tables found in the MewX README".into()));
    }
    if records.is_empty() {
        return Err(AdapterError::LayoutChanged("MewX firmware tables have no rows".into()));
    }
    Ok(records)
}

fn parse_row(row: &[String], lineno: usize, checked_at: &str) -> Result<PackageRecord, AdapterError> {
    let bad = |what: &str| AdapterError::LayoutChanged(format!("MewX line {lineno}: {what}"));
    if row.len() != EXPECTED_HEADER.len() {
        return Err(bad(&format!("expected {} cells, found {}", EXPECTED_HEADER.len(), row.len())));
    }
    let (long, short, type_text, build, sha, md5, size) =
        (&row[0], &row[1], &row[2], &row[3], &row[4], &row[5], &row[6]);

    let mut short_parts = short.split_whitespace();
    let version = short_parts
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| bad(&format!("version {short:?} is not a firmware version")))?;
    let variant: Option<String> = {
        let rest: Vec<&str> = short_parts.collect();
        (!rest.is_empty()).then(|| rest.join(" "))
    };

    // Type cells carry an emoji prefix ("🆙 Update", "❤️‍🩹 Recovery").
    let type_word: String = type_text.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let base_type = match type_word.as_str() {
        "Update" => PackageType::Update,
        "Recovery" => PackageType::Reinstall,
        _ => PackageType::Unknown,
    };
    let package_type = if variant.is_some() { PackageType::SpecialVariant } else { base_type };

    let mut hashes = Vec::new();
    for (alg, text) in [(HashAlgorithm::Sha256, sha), (HashAlgorithm::Md5, md5)] {
        if text.is_empty() || text == "-" {
            continue;
        }
        let value = normalize_hex(text, alg).ok_or_else(|| bad(&format!("{alg:?} column holds {text:?}")))?;
        hashes.push(PublishedHash {
            algorithm: alg,
            value,
            source_id: SOURCE_ID.into(),
            derived_from: None,
            note: None,
        });
    }

    let parsed_size = parse_size_text(size);
    let type_label = if type_word.is_empty() { type_text.as_str() } else { type_word.as_str() };
    let variant_suffix = variant.as_deref().map(|v| format!(":{v}")).unwrap_or_default();
    Ok(PackageRecord {
        package_id: format!("{SOURCE_ID}:{version}:{type_label}{variant_suffix}").to_ascii_lowercase(),
        source_id: SOURCE_ID.into(),
        firmware_version: version,
        long_version: (!long.is_empty()).then(|| long.clone()),
        build_date: is_build_date(build).then(|| build.clone()),
        source_section: None,
        raw_package_label: format!("{long} {type_label}").trim().to_string(),
        package_type,
        variant_label: variant,
        published_size_bytes: parsed_size.map(|s| s.0),
        size_is_approximate: parsed_size.is_some_and(|s| s.1),
        raw_size_text: (!size.is_empty()).then(|| size.clone()),
        hashes,
        mirrors: Vec::new(),
        last_checked: checked_at.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/sources/mewx_readme.md");
    const CHANGED: &str = include_str!("../../../tests/fixtures/sources/mewx_readme_changed_columns.md");

    #[test]
    fn parses_update_recovery_and_sys_ex_rows() {
        let recs = parse(FIXTURE, "t").unwrap();
        let rec_1420 = recs
            .iter()
            .find(|r| r.firmware_version.to_string() == "14.20.00" && r.package_type == PackageType::Reinstall)
            .unwrap();
        assert_eq!(rec_1420.build_date.as_deref(), Some("2026_1014"));
        assert_eq!(rec_1420.published_size_bytes, Some(1_423_693_312));
        assert!(!rec_1420.size_is_approximate);
        assert_eq!(
            rec_1420.hash(HashAlgorithm::Sha256).unwrap().value,
            "549cf0d5febeca28eb0716abf086639cb216b1400ead8059de13956d0d302762"
        );

        let sys_ex = recs.iter().find(|r| r.variant_label.as_deref() == Some("sys_ex")).unwrap();
        assert_eq!(sys_ex.package_type, PackageType::SpecialVariant);
        assert!(sys_ex.size_is_approximate, "rounded sizes like 811MB are approximate");
        assert!(sys_ex.raw_package_label.contains("Update"), "raw label keeps the source's type");
    }

    #[test]
    fn changed_column_order_fails_closed() {
        let err = parse(CHANGED, "t").unwrap_err();
        assert!(matches!(err, AdapterError::LayoutChanged(_)), "{err:?}");
    }

    #[test]
    fn shifted_digest_fails_closed() {
        let shifted = FIXTURE.replacen(
            "| 0f81be6fd890bc04925b3953e0862dfc2ead68edfdc33f31c3c60d0c267f68e4 |",
            "| 1266086912 B |",
            1,
        );
        assert!(matches!(parse(&shifted, "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn empty_document_fails_closed() {
        assert!(matches!(parse("# nothing here", "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn build_date_shape() {
        assert!(is_build_date("2026_1014"));
        assert!(!is_build_date("2020"));
        assert!(!is_build_date("2026-1014"));
    }
}
