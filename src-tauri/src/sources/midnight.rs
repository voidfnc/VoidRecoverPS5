//! Adapter for the Midnight Channel Archives' PS5 firmware listings.
//!
//! The archive serves a file by its checksum, so each listed row gives a
//! direct https download address that carries the SHA-256 the app then
//! re-checks on the downloaded bytes. The two listing pages are read when the
//! catalog is refreshed, with the pause its robots.txt asks for between
//! requests. The download address itself is only ever requested when the user
//! chooses to download from the archive.

use std::sync::LazyLock;

use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use super::AdapterError;
use crate::domain::types::{
    Availability, HashAlgorithm, Mirror, MirrorMode, MirrorScope, PackageRecord, PackageType, PublishedHash,
    normalize_hex, parse_size_text,
};
use crate::domain::version::FirmwareVersion;

pub const SOURCE_ID: &str = "midnight-archive-ps5";
pub const HOST_NAME: &str = "Midnight Archive";
pub const MAX_BYTES: usize = 1024 * 1024;
/// robots.txt asks for `Crawl-delay: 5`.
pub const CRAWL_DELAY_SECS: u64 = 5;
const DOWNLOAD_BASE: &str = "https://archive.midnightchannel.net/SonyPS/Firmware/download";
const LISTING_BASE: &str = "https://archive.midnightchannel.net/SonyPS/Firmware/index.php?cat=";

/// The two PS5 listings, in the order they are fetched.
pub const LISTINGS: [Listing; 2] = [
    Listing { category: "PS5SYS", package_type: PackageType::Update, word: "System" },
    Listing { category: "PS5REC", package_type: PackageType::Reinstall, word: "Recovery" },
];

#[derive(Debug, Clone, Copy)]
pub struct Listing {
    pub category: &'static str,
    pub package_type: PackageType,
    /// The word the row descriptions use for this listing.
    pub word: &'static str,
}

impl Listing {
    pub fn url(&self) -> String {
        format!("{LISTING_BASE}{}", self.category)
    }
}

static DESCRIPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^PS5 Official Retail (System|Recovery) Firmware v(\d{1,2}\.\d{2}(?:\.\d{2})?)(?: \((.+)\))?$")
        .expect("static regex")
});
static ROW_MD5: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"showInfo\('([0-9a-fA-F]{32})'\)").expect("static regex"));

/// The address the archive serves a file at, built only from a validated
/// SHA-256 and never from page text.
pub fn download_url(sha256: &str) -> Option<String> {
    (sha256.len() == 64 && sha256.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| format!("{DOWNLOAD_BASE}/{sha256}/PS5UPDATE.PUP"))
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("static selector")
}

fn text_of(el: ElementRef<'_>) -> String {
    el.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse one listing page. Fails closed on any change to the table header, on
/// a row it cannot read in full, and on an empty table.
pub fn parse_listing(html: &str, listing: Listing, checked_at: &str) -> Result<Vec<PackageRecord>, AdapterError> {
    let bad = |what: String| AdapterError::LayoutChanged(format!("Midnight Archive {}: {what}", listing.category));
    let doc = Html::parse_document(html);
    let header: Vec<String> = doc.select(&sel("tr th")).map(text_of).collect();
    if header != ["", "SHA256", "Size", "Description & Link", "File Added"] {
        return Err(bad(format!("table header is {header:?}")));
    }
    let td = sel("td");
    let a = sel("a[href]");
    let info = sel("span.ui-icon-info");
    let mut out = Vec::new();
    for (i, row) in doc.select(&sel("tr")).filter(|r| r.select(&sel("th")).next().is_none()).enumerate() {
        let row_bad = |what: String| bad(format!("row {}: {what}", i + 1));
        let cells: Vec<ElementRef<'_>> = row.select(&td).collect();
        if cells.len() != 5 {
            return Err(row_bad(format!("expected 5 cells, found {}", cells.len())));
        }
        let sha = normalize_hex(&text_of(cells[1]), HashAlgorithm::Sha256)
            .ok_or_else(|| row_bad(format!("SHA256 cell {:?}", text_of(cells[1]))))?;
        let link = cells[3].select(&a).next().ok_or_else(|| row_bad("no download link".into()))?;
        let href = link.value().attr("href").unwrap_or("");
        if !href.starts_with(&format!("download/{sha}/")) {
            return Err(row_bad(format!("download link {href:?} does not carry the listed SHA-256")));
        }
        let description = text_of(link);
        let caps = DESCRIPTION.captures(&description).ok_or_else(|| row_bad(format!("description {description:?}")))?;
        if &caps[1] != listing.word {
            return Err(row_bad(format!("{description:?} is in the wrong listing")));
        }
        let version: FirmwareVersion = caps[2].parse().map_err(|_| row_bad(format!("version in {description:?}")))?;
        let note = caps.get(3).map(|m| m.as_str().to_string());

        let mut hashes = vec![PublishedHash {
            algorithm: HashAlgorithm::Sha256,
            value: sha.clone(),
            source_id: SOURCE_ID.into(),
            derived_from: None,
            note: None,
        }];
        let md5 = cells[0]
            .select(&info)
            .filter_map(|s| s.value().attr("onclick"))
            .find_map(|o| ROW_MD5.captures(o).and_then(|c| normalize_hex(&c[1], HashAlgorithm::Md5)));
        if let Some(value) = md5 {
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
        let kind = if listing.package_type == PackageType::Update { "update" } else { "recovery" };
        let suffix = note.as_deref().map(|n| format!(":{n}")).unwrap_or_default();
        out.push(PackageRecord {
            package_id: format!("{SOURCE_ID}:{version}:{kind}:{}{suffix}", &sha[..12]),
            source_id: SOURCE_ID.into(),
            firmware_version: version,
            long_version: None,
            build_date: None,
            source_section: Some(listing.category.to_string()),
            raw_package_label: description,
            package_type: listing.package_type,
            variant_label: None,
            published_size_bytes: size.map(|s| s.0),
            size_is_approximate: size.is_some_and(|s| s.1),
            raw_size_text: (!size_text.is_empty()).then_some(size_text),
            hashes,
            mirrors: vec![Mirror {
                host: HOST_NAME.into(),
                url: download_url(&sha).expect("sha256 was validated above"),
                mode: MirrorMode::DirectHttps,
                scope: MirrorScope::File,
                availability: Availability::Unknown,
                probed_at: None,
                availability_note: None,
            }],
            last_checked: checked_at.to_string(),
        });
    }
    if out.is_empty() {
        return Err(bad("the table has no rows".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYS: &str = include_str!("../../../tests/fixtures/sources/midnight_ps5sys.html");
    const REC: &str = include_str!("../../../tests/fixtures/sources/midnight_ps5rec.html");

    fn sys() -> Vec<PackageRecord> {
        parse_listing(SYS, LISTINGS[0], "t").unwrap()
    }

    #[test]
    fn parses_update_listing() {
        let recs = sys();
        assert_eq!(recs.len(), 63);
        let r = recs.iter().find(|r| r.firmware_version.to_string() == "14.20.00").unwrap();
        assert_eq!(r.package_type, PackageType::Update);
        assert_eq!(
            r.hash(HashAlgorithm::Sha256).unwrap().value,
            "0f81be6fd890bc04925b3953e0862dfc2ead68edfdc33f31c3c60d0c267f68e4"
        );
        assert_eq!(r.hash(HashAlgorithm::Md5).unwrap().value, "e8c8b7d66731846395ddfa79acce7ac1");
        assert!(r.size_is_approximate);
        assert_eq!(
            r.mirrors[0].url,
            "https://archive.midnightchannel.net/SonyPS/Firmware/download/0f81be6fd890bc04925b3953e0862dfc2ead68edfdc33f31c3c60d0c267f68e4/PS5UPDATE.PUP"
        );
        assert_eq!(r.mirrors[0].mode, MirrorMode::DirectHttps);
        assert_eq!(r.mirrors[0].host, HOST_NAME);
    }

    #[test]
    fn parses_recovery_listing() {
        let recs = parse_listing(REC, LISTINGS[1], "t").unwrap();
        assert_eq!(recs.len(), 60);
        assert!(recs.iter().all(|r| r.package_type == PackageType::Reinstall));
        assert!(recs.iter().all(|r| r.mirrors[0].url.ends_with("/PS5UPDATE.PUP")));
    }

    #[test]
    fn game_disc_notes_are_kept_in_the_raw_label() {
        let r = sys().into_iter().find(|r| r.firmware_version.to_string() == "01.14.00").unwrap();
        assert!(r.raw_package_label.contains("Demon's Souls"));
        assert!(r.variant_label.is_none());
    }

    #[test]
    fn record_ids_are_unique() {
        let recs = sys();
        let mut ids: Vec<&str> = recs.iter().map(|r| r.package_id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), recs.len());
    }

    #[test]
    fn changed_header_fails_closed() {
        let changed = SYS.replace("<strong>SHA256</strong>", "<strong>CRC</strong>");
        assert!(matches!(parse_listing(&changed, LISTINGS[0], "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn link_that_does_not_match_the_listed_hash_fails_closed() {
        let tampered = SYS.replacen("download/0f81be6f", "download/0f81be6e", 1);
        assert!(matches!(parse_listing(&tampered, LISTINGS[0], "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn unreadable_description_fails_closed() {
        let changed = SYS.replacen("PS5 Official Retail System Firmware v14.20", "PS5 Firmware 14.20", 1);
        assert!(matches!(parse_listing(&changed, LISTINGS[0], "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn recovery_page_is_not_accepted_as_the_update_listing() {
        assert!(matches!(parse_listing(REC, LISTINGS[0], "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn empty_table_fails_closed() {
        let head = &SYS[..SYS.find("</tr>").unwrap() + 5];
        let empty = format!("{head}</table></body></html>");
        assert!(matches!(parse_listing(&empty, LISTINGS[0], "t"), Err(AdapterError::LayoutChanged(_))));
    }

    #[test]
    fn download_url_needs_a_lowercase_sha256() {
        assert!(download_url(&"a".repeat(64)).is_some());
        assert!(download_url(&"A".repeat(64)).is_none());
        assert!(download_url("../etc").is_none());
        assert!(download_url(&"a".repeat(63)).is_none());
    }
}
