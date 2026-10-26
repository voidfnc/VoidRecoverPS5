//! Source adapters and the refresh that combines them.
//!
//! Each adapter fails closed on layout changes. A failed source keeps its
//! previously cached records, marked stale, instead of losing them or
//! guessing at new columns.

pub mod darthsternie;
pub mod http;
pub mod mewx;
pub mod midnight;
pub mod sony;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cache::CatalogCache;
use crate::domain::catalog::OfficialLatest;
use crate::domain::types::PackageRecord;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
pub enum AdapterError {
    #[error("network error: {0}")]
    Network(String),
    #[error("{url} is larger than the {limit}-byte limit")]
    TooLarge { url: String, limit: usize },
    #[error("source layout changed: {0}")]
    LayoutChanged(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    /// Fetched and parsed in the last refresh.
    Fresh,
    /// Last refresh failed; previous records are still shown.
    Stale,
    /// Last refresh failed and there is nothing cached.
    Failed,
    NeverFetched,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceStatus {
    pub source_id: String,
    pub state: SourceState,
    /// When records were last fetched successfully.
    pub fetched_at: Option<String>,
    pub attempted_at: Option<String>,
    pub record_count: usize,
    pub error: Option<String>,
    pub content_sha256: Option<String>,
    pub final_host: Option<String>,
    /// True when the content hash differs from the previous successful fetch.
    pub changed_since_last_fetch: bool,
}

impl SourceStatus {
    pub fn never(source_id: &str) -> Self {
        Self {
            source_id: source_id.into(),
            state: SourceState::NeverFetched,
            fetched_at: None,
            attempted_at: None,
            record_count: 0,
            error: None,
            content_sha256: None,
            final_host: None,
            changed_since_last_fetch: false,
        }
    }
}

pub struct Fetched<T> {
    pub value: T,
    pub content_sha256: String,
    pub final_url: String,
}

pub fn sha256_hex(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Combine adapter results with the previous cache. Pure, so it is tested
/// without network access.
pub fn assemble(
    previous: &CatalogCache,
    mewx: Result<Fetched<Vec<PackageRecord>>, AdapterError>,
    sony_page: Result<Fetched<OfficialLatest>, AdapterError>,
    darthsternie: Result<Fetched<Vec<PackageRecord>>, AdapterError>,
    midnight: Result<Fetched<Vec<PackageRecord>>, AdapterError>,
    now: &str,
) -> CatalogCache {
    let mut statuses = Vec::new();

    let mewx_records = settle(previous, mewx::SOURCE_ID, mewx, now, &mut statuses, |v| v.len());
    let ds_records = settle(previous, darthsternie::SOURCE_ID, darthsternie, now, &mut statuses, |v| v.len());
    let mn_records = settle(previous, midnight::SOURCE_ID, midnight, now, &mut statuses, |v| v.len());

    // The Sony page yields one value, not records; settle it the same way.
    let mut latest = match sony_page {
        Ok(f) => {
            let prev = previous.status(sony::SOURCE_ID);
            statuses.push(fresh_status(sony::SOURCE_ID, &f, now, 0, prev));
            Some(f.value)
        }
        Err(e) => {
            let prev = previous.official_latest.clone();
            statuses.push(failed_status(previous, sony::SOURCE_ID, &e, now, prev.is_some(), 0));
            prev
        }
    };
    let mut sony_records = sony::records_from_index(&mewx_records, latest.as_mut(), now);
    // Keep earlier probe results for the same URL.
    for r in &mut sony_records {
        for m in &mut r.mirrors {
            if let Some(old) = previous.mirror_by_url(&m.url) {
                m.availability = old.availability;
                m.probed_at = old.probed_at.clone();
                m.availability_note = old.availability_note.clone();
            }
        }
    }
    if let Some(s) = statuses.iter_mut().find(|s| s.source_id == sony::SOURCE_ID) {
        s.record_count = sony_records.len();
    }

    let mut records = mewx_records;
    records.extend(ds_records);
    records.extend(mn_records);
    records.extend(sony_records);
    CatalogCache {
        schema_version: crate::cache::CACHE_SCHEMA_VERSION,
        saved_at: Some(now.to_string()),
        statuses,
        records,
        official_latest: latest,
    }
}

fn settle(
    previous: &CatalogCache,
    source_id: &str,
    result: Result<Fetched<Vec<PackageRecord>>, AdapterError>,
    now: &str,
    statuses: &mut Vec<SourceStatus>,
    count: impl Fn(&Vec<PackageRecord>) -> usize,
) -> Vec<PackageRecord> {
    match result {
        Ok(f) => {
            let prev = previous.status(source_id);
            statuses.push(fresh_status(source_id, &f, now, count(&f.value), prev));
            f.value
        }
        Err(e) => {
            let kept: Vec<PackageRecord> =
                previous.records.iter().filter(|r| r.source_id == source_id).cloned().collect();
            statuses.push(failed_status(previous, source_id, &e, now, !kept.is_empty(), kept.len()));
            kept
        }
    }
}

fn fresh_status<T>(
    source_id: &str,
    f: &Fetched<T>,
    now: &str,
    record_count: usize,
    prev: Option<&SourceStatus>,
) -> SourceStatus {
    SourceStatus {
        source_id: source_id.into(),
        state: SourceState::Fresh,
        fetched_at: Some(now.into()),
        attempted_at: Some(now.into()),
        record_count,
        error: None,
        content_sha256: Some(f.content_sha256.clone()),
        final_host: http::host_of(&f.final_url),
        changed_since_last_fetch: prev
            .and_then(|p| p.content_sha256.as_deref())
            .is_some_and(|old| old != f.content_sha256),
    }
}

fn failed_status(
    previous: &CatalogCache,
    source_id: &str,
    e: &AdapterError,
    now: &str,
    have_old: bool,
    record_count: usize,
) -> SourceStatus {
    let prev = previous.status(source_id);
    SourceStatus {
        source_id: source_id.into(),
        state: if have_old { SourceState::Stale } else { SourceState::Failed },
        fetched_at: prev.and_then(|p| p.fetched_at.clone()),
        attempted_at: Some(now.into()),
        record_count,
        error: Some(e.to_string()),
        content_sha256: prev.and_then(|p| p.content_sha256.clone()),
        final_host: prev.and_then(|p| p.final_host.clone()),
        changed_since_last_fetch: false,
    }
}

/// The archive's two listings, one after the other with the pause its
/// robots.txt asks for. All or nothing: half a listing would drop records.
async fn fetch_midnight(client: &Client, now: &str) -> Result<Fetched<Vec<PackageRecord>>, AdapterError> {
    let mut records = Vec::new();
    let mut hashes = String::new();
    let mut final_url = String::new();
    for (i, listing) in midnight::LISTINGS.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(midnight::CRAWL_DELAY_SECS)).await;
        }
        let page = http::fetch_text(client, &listing.url(), midnight::MAX_BYTES).await?;
        records.extend(midnight::parse_listing(&page.body, *listing, now)?);
        hashes.push_str(&sha256_hex(&page.body));
        final_url = page.final_url;
    }
    Ok(Fetched { value: records, content_sha256: sha256_hex(&hashes), final_url })
}

/// Fetch all sources, then probe the two current Sony files.
pub async fn refresh(client: &Client, previous: &CatalogCache) -> CatalogCache {
    let now = now_rfc3339();
    let (m, s, d, n) = tokio::join!(
        http::fetch_text(client, mewx::RAW_URL, mewx::MAX_BYTES),
        http::fetch_text(client, sony::SUPPORT_PAGE, sony::MAX_BYTES),
        http::fetch_text(client, darthsternie::REST_URL, darthsternie::MAX_BYTES),
        fetch_midnight(client, &now),
    );
    let mewx = m.and_then(|r| {
        Ok(Fetched { value: mewx::parse(&r.body, &now)?, content_sha256: sha256_hex(&r.body), final_url: r.final_url })
    });
    let sony_page = s.and_then(|r| {
        Ok(Fetched {
            value: sony::parse_support_page(&r.body)?,
            content_sha256: sha256_hex(&r.body),
            final_url: r.final_url,
        })
    });
    let darthsternie = d.and_then(|r| {
        let parsed = darthsternie::parse_rest_json(&r.body, &now)?;
        Ok(Fetched { value: parsed.records, content_sha256: parsed.content_sha256, final_url: r.final_url })
    });
    let mut cache = assemble(previous, mewx, sony_page, darthsternie, n, &now);
    if let Some(latest) = cache.official_latest.clone() {
        for url in [latest.update_url, latest.reinstall_url] {
            probe_sony_url(client, &mut cache, &url).await;
        }
    }
    cache
}

/// HEAD one Sony CDN URL and record the result on every mirror with that URL.
pub async fn probe_sony_url(client: &Client, cache: &mut CatalogCache, url: &str) {
    let expected = cache.expected_exact_size_for_url(url);
    let (availability, note) = match http::head(client, url).await {
        Ok(h) => sony::classify(h.status, h.content_length, expected),
        Err(e) => (crate::domain::types::Availability::Unknown, Some(e.to_string())),
    };
    let now = now_rfc3339();
    for r in &mut cache.records {
        for m in &mut r.mirrors {
            if m.url == url {
                m.availability = availability;
                m.probed_at = Some(now.clone());
                m.availability_note = note.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEWX: &str = include_str!("../../../tests/fixtures/sources/mewx_readme.md");
    const SONY: &str = include_str!("../../../tests/fixtures/sources/sony_support_page.html");
    const DS: &str = include_str!("../../../tests/fixtures/sources/darthsternie_page.json");
    const MN_SYS: &str = include_str!("../../../tests/fixtures/sources/midnight_ps5sys.html");
    const MN_REC: &str = include_str!("../../../tests/fixtures/sources/midnight_ps5rec.html");

    fn mn_ok() -> Result<Fetched<Vec<PackageRecord>>, AdapterError> {
        let mut v = midnight::parse_listing(MN_SYS, midnight::LISTINGS[0], "t").unwrap();
        v.extend(midnight::parse_listing(MN_REC, midnight::LISTINGS[1], "t").unwrap());
        Ok(Fetched { value: v, content_sha256: sha256_hex(MN_SYS), final_url: String::new() })
    }

    fn ok_all(prev: &CatalogCache) -> CatalogCache {
        let ds = darthsternie::parse_rest_json(DS, "t").unwrap();
        assemble(
            prev,
            Ok(Fetched {
                value: mewx::parse(MEWX, "t").unwrap(),
                content_sha256: sha256_hex(MEWX),
                final_url: mewx::RAW_URL.into(),
            }),
            Ok(Fetched {
                value: sony::parse_support_page(SONY).unwrap(),
                content_sha256: sha256_hex(SONY),
                final_url: sony::SUPPORT_PAGE.into(),
            }),
            Ok(Fetched {
                value: ds.records,
                content_sha256: ds.content_sha256,
                final_url: darthsternie::REST_URL.into(),
            }),
            mn_ok(),
            "t1",
        )
    }

    #[test]
    fn all_sources_fresh() {
        let c = ok_all(&CatalogCache::default());
        assert!(c.statuses.iter().all(|s| s.state == SourceState::Fresh));
        assert_eq!(c.official_latest.as_ref().unwrap().version.unwrap().to_string(), "14.20.00");
        assert!(c.records.iter().any(|r| r.source_id == sony::SOURCE_ID));
    }

    #[test]
    fn failed_source_keeps_previous_records_as_stale() {
        let first = ok_all(&CatalogCache::default());
        let ds_count = first.records.iter().filter(|r| r.source_id == darthsternie::SOURCE_ID).count();
        let second = assemble(
            &first,
            Ok(Fetched {
                value: mewx::parse(MEWX, "t").unwrap(),
                content_sha256: sha256_hex(MEWX),
                final_url: String::new(),
            }),
            Err(AdapterError::Network("offline".into())),
            Err(AdapterError::LayoutChanged("columns moved".into())),
            mn_ok(),
            "t2",
        );
        let ds = second.status(darthsternie::SOURCE_ID).unwrap();
        assert_eq!(ds.state, SourceState::Stale);
        assert_eq!(ds.fetched_at.as_deref(), Some("t1"));
        assert!(ds.error.as_deref().unwrap().contains("columns moved"));
        assert_eq!(second.records.iter().filter(|r| r.source_id == darthsternie::SOURCE_ID).count(), ds_count);
        assert!(second.official_latest.is_some(), "previous official latest kept");
        assert_eq!(second.status(sony::SOURCE_ID).unwrap().state, SourceState::Stale);
    }

    #[test]
    fn failure_with_nothing_cached_is_failed() {
        let c = assemble(
            &CatalogCache::default(),
            Err(AdapterError::Network("x".into())),
            Err(AdapterError::Network("x".into())),
            Err(AdapterError::Network("x".into())),
            Err(AdapterError::Network("x".into())),
            "t",
        );
        assert!(c.statuses.iter().all(|s| s.state == SourceState::Failed));
        assert!(c.records.is_empty());
    }

    #[test]
    fn content_change_is_detected() {
        let first = ok_all(&CatalogCache::default());
        let changed = format!("{MEWX}\n");
        let second = assemble(
            &first,
            Ok(Fetched {
                value: mewx::parse(&changed, "t").unwrap(),
                content_sha256: sha256_hex(&changed),
                final_url: String::new(),
            }),
            Err(AdapterError::Network("x".into())),
            Err(AdapterError::Network("x".into())),
            Err(AdapterError::Network("x".into())),
            "t2",
        );
        assert!(second.status(mewx::SOURCE_ID).unwrap().changed_since_last_fetch);
    }

    #[test]
    fn probe_results_survive_a_refresh() {
        let mut first = ok_all(&CatalogCache::default());
        let url = first.official_latest.as_ref().unwrap().reinstall_url.clone();
        for r in &mut first.records {
            for m in &mut r.mirrors {
                if m.url == url {
                    m.availability = crate::domain::types::Availability::Available;
                }
            }
        }
        let second = ok_all(&first);
        assert_eq!(second.mirror_by_url(&url).unwrap().availability, crate::domain::types::Availability::Available);
    }
}
