//! Adapter for Darthsternie's PS5 firmware archive (WordPress + TablePress).
//! Reads the WordPress REST JSON for the page; the HTML is the same tables.

use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::AdapterError;
use crate::domain::manifest::is_https_url;
use crate::domain::types::{
    Availability, HashAlgorithm, Mirror, MirrorMode, MirrorScope, PackageRecord, PackageType, PublishedHash,
    normalize_hex, parse_size_text,
};
use crate::domain::version::FirmwareVersion;

pub const SOURCE_ID: &str = "darthsternie-ps5";
pub const REST_URL: &str = "https://darthsternie.net/wp-json/wp/v2/pages?slug=ps5-firmwares";
pub const MAX_BYTES: usize = 4 * 1024 * 1024;

const EXPECTED_HEADER: [&str; 5] = ["Version", "MD5 Hash", "Filesize", "MEGA", "1Fichier"];
const SECTIONS: [(&str, PackageType); 2] =
    [("Official Firmwares", PackageType::Update), ("Recovery Firmwares", PackageType::Reinstall)];

#[derive(Deserialize)]
struct WpPage {
    content: WpContent,
}

#[derive(Deserialize)]
struct WpContent {
    rendered: String,
}

pub struct Parsed {
    pub records: Vec<PackageRecord>,
    /// SHA-256 of the rendered tables; the page's `modified` date is not
    /// maintained, so changes are detected by content.
    pub content_sha256: String,
}

pub fn parse_rest_json(json: &str, checked_at: &str) -> Result<Parsed, AdapterError> {
    let pages: Vec<WpPage> = serde_json::from_str(json)
        .map_err(|e| AdapterError::LayoutChanged(format!("Darthsternie REST response: {e}")))?;
    let page = pages
        .into_iter()
        .next()
        .ok_or_else(|| AdapterError::LayoutChanged("Darthsternie REST response has no page".into()))?;
    parse_html(&page.content.rendered, checked_at)
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("static selector")
}

fn text_of(el: ElementRef<'_>) -> String {
    el.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse_html(html: &str, checked_at: &str) -> Result<Parsed, AdapterError> {
    let doc = Html::parse_fragment(html);
    let heading_or_table = sel("h1, h2, h3, h4, h5, h6, table");
    let mut current_heading: Option<String> = None;
    let mut records = Vec::new();
    let mut seen_sections = Vec::new();

    for el in doc.select(&heading_or_table) {
        if el.value().name() != "table" {
            current_heading = Some(text_of(el));
            continue;
        }
        let heading = current_heading.clone().unwrap_or_default();
        let Some((section, package_type)) = SECTIONS.iter().find(|(name, _)| *name == heading) else {
            return Err(AdapterError::LayoutChanged(format!(
                "Darthsternie page has a table under an unknown heading {heading:?}"
            )));
        };
        if seen_sections.contains(section) {
            return Err(AdapterError::LayoutChanged(format!("Darthsternie section {section:?} appears twice")));
        }
        seen_sections.push(*section);
        parse_table(el, section, *package_type, checked_at, &mut records)?;
    }
    for (name, _) in SECTIONS {
        if !seen_sections.contains(&name) {
            return Err(AdapterError::LayoutChanged(format!("Darthsternie section {name:?} is missing")));
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(html.as_bytes());
    Ok(Parsed { records, content_sha256: hex::encode(hasher.finalize()) })
}

fn parse_table(
    table: ElementRef<'_>,
    section: &str,
    section_type: PackageType,
    checked_at: &str,
    out: &mut Vec<PackageRecord>,
) -> Result<(), AdapterError> {
    let header: Vec<String> = table.select(&sel("thead th")).map(text_of).collect();
    if header != EXPECTED_HEADER {
        return Err(AdapterError::LayoutChanged(format!(
            "Darthsternie {section:?} header is {header:?}, expected {EXPECTED_HEADER:?}"
        )));
    }
    let rows: Vec<ElementRef<'_>> = table.select(&sel("tbody tr")).collect();
    if rows.is_empty() {
        return Err(AdapterError::LayoutChanged(format!("Darthsternie {section:?} table has no rows")));
    }
    let td = sel("td");
    let a = sel("a[href]");
    for (i, row) in rows.into_iter().enumerate() {
        let bad = |what: String| AdapterError::LayoutChanged(format!("Darthsternie {section:?} row {}: {what}", i + 1));
        let cells: Vec<ElementRef<'_>> = row.select(&td).collect();
        if cells.len() != EXPECTED_HEADER.len() {
            return Err(bad(format!("expected 5 cells, found {}", cells.len())));
        }
        let label = text_of(cells[0]);
        let (version, variant) = parse_label(&label).ok_or_else(|| bad(format!("version cell {label:?}")))?;

        let md5_text = text_of(cells[1]);
        let mut hashes = Vec::new();
        if !md5_text.is_empty() {
            let value =
                normalize_hex(&md5_text, HashAlgorithm::Md5).ok_or_else(|| bad(format!("MD5 cell {md5_text:?}")))?;
            hashes.push(PublishedHash {
                algorithm: HashAlgorithm::Md5,
                value,
                source_id: SOURCE_ID.into(),
                derived_from: None,
                note: None,
            });
        }

        let size_text = text_of(cells[2]);
        let size = parse_size_text(&size_text);

        let mut mirrors = Vec::new();
        for (cell, host) in [(cells[3], "MEGA"), (cells[4], "1fichier")] {
            if let Some(link) = cell.select(&a).next() {
                let url = link.value().attr("href").unwrap_or("").trim().to_string();
                if !is_https_url(&url) {
                    continue; // never surface non-https mirror links
                }
                let scope = if url.contains("/folder/") { MirrorScope::Folder } else { MirrorScope::File };
                mirrors.push(Mirror {
                    host: host.into(),
                    url,
                    mode: MirrorMode::ExternalBrowser,
                    scope,
                    availability: Availability::BrowserOnly,
                    probed_at: None,
                    availability_note: None,
                });
            }
        }

        let package_type = if variant.is_some() { PackageType::SpecialVariant } else { section_type };
        let type_slug = if section_type == PackageType::Update { "official" } else { "recovery" };
        let variant_suffix = variant.as_deref().map(|v| format!(":{v}")).unwrap_or_default();
        out.push(PackageRecord {
            package_id: format!("{SOURCE_ID}:{version}:{type_slug}{variant_suffix}"),
            source_id: SOURCE_ID.into(),
            firmware_version: version,
            long_version: None,
            build_date: None,
            source_section: Some(section.to_string()),
            raw_package_label: label,
            package_type,
            variant_label: variant,
            published_size_bytes: size.map(|s| s.0),
            size_is_approximate: size.is_some_and(|s| s.1),
            raw_size_text: (!size_text.is_empty()).then_some(size_text),
            hashes,
            mirrors,
            last_checked: checked_at.to_string(),
        });
    }
    Ok(())
}

/// "Firmware 08.20.02 sys_ex" -> (08.20.02, Some("sys_ex"))
fn parse_label(label: &str) -> Option<(FirmwareVersion, Option<String>)> {
    let rest = label.strip_prefix("Firmware ")?;
    let mut parts = rest.split_whitespace();
    let version = parts.next()?.parse().ok()?;
    let variant: Vec<&str> = parts.collect();
    Some((version, (!variant.is_empty()).then(|| variant.join(" "))))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/sources/darthsternie_page.json");
    const CHANGED: &str = include_str!("../../../tests/fixtures/sources/darthsternie_page_changed_header.json");

    fn parsed() -> Parsed {
        parse_rest_json(FIXTURE, "t").unwrap()
    }

    #[test]
    fn parses_both_sections_with_raw_labels() {
        let p = parsed();
        let update = p.records.iter().find(|r| r.package_id == "darthsternie-ps5:14.10.00:official").unwrap();
        assert_eq!(update.package_type, PackageType::Update);
        assert_eq!(update.source_section.as_deref(), Some("Official Firmwares"));
        assert_eq!(update.raw_package_label, "Firmware 14.10.00");
        assert_eq!(update.hash(HashAlgorithm::Md5).unwrap().value, "f7853a0fa97a9a7828ac548798012782");
        assert_eq!(update.published_size_bytes, None, "newer rows have no size");

        let recovery = p.records.iter().find(|r| r.package_id == "darthsternie-ps5:14.10.00:recovery").unwrap();
        assert_eq!(recovery.package_type, PackageType::Reinstall);
        assert!(p.records.len() > 100);
    }

    #[test]
    fn sys_ex_rows_are_special_variants_with_folder_links() {
        let p = parsed();
        let sys_ex = p.records.iter().find(|r| r.raw_package_label == "Firmware 08.20.02 sys_ex").unwrap();
        assert_eq!(sys_ex.package_type, PackageType::SpecialVariant);
        assert_eq!(sys_ex.variant_label.as_deref(), Some("sys_ex"));
        assert!(sys_ex.size_is_approximate);
        let mega = sys_ex.mirrors.iter().find(|m| m.host == "MEGA").unwrap();
        assert_eq!(mega.scope, MirrorScope::Folder);
        assert_eq!(mega.mode, MirrorMode::ExternalBrowser);
        assert_eq!(mega.availability, Availability::BrowserOnly);
    }

    #[test]
    fn fixture_links_are_synthetic() {
        for r in parsed().records {
            for m in r.mirrors {
                assert!(m.url.to_ascii_lowercase().contains("fixture"), "real link in fixture: {}", m.url);
            }
        }
    }

    #[test]
    fn changed_header_fails_closed() {
        assert!(matches!(parse_rest_json(CHANGED, "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn unknown_section_or_missing_section_fails_closed() {
        let p: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let html = p[0]["content"]["rendered"].as_str().unwrap();
        let renamed = html.replace(">Recovery Firmwares<", ">Beta Firmwares<");
        assert!(matches!(parse_html(&renamed, "t"), Err(AdapterError::LayoutChanged(_))));
        let official_only = &html[..html.find("Recovery Firmwares").unwrap()];
        assert!(matches!(parse_html(official_only, "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn bad_version_cell_fails_closed() {
        let p: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let html = p[0]["content"]["rendered"].as_str().unwrap().replacen("Firmware 14.10.00", "FW fourteen", 1);
        assert!(matches!(parse_html(&html, "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn content_hash_is_stable() {
        assert_eq!(parsed().content_sha256, parsed().content_sha256);
    }

    #[test]
    fn label_parsing() {
        assert_eq!(parse_label("Firmware 14.10.00").unwrap().0.to_string(), "14.10.00");
        assert_eq!(parse_label("Firmware 06.00.01 sys_ex").unwrap().1.as_deref(), Some("sys_ex"));
        assert!(parse_label("14.10.00").is_none());
    }
}
