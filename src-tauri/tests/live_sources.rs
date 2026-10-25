//! Live checks against the real source pages. Ignored by default so automated
//! tests never depend on third-party sites. Run by hand after adapter changes:
//!
//!     cargo test --test live_sources -- --ignored --nocapture

use voidrecoverps5_lib::cache::CatalogCache;
use voidrecoverps5_lib::domain::catalog::merge;
use voidrecoverps5_lib::domain::manifest::SourcesManifest;
use voidrecoverps5_lib::domain::types::Availability;
use voidrecoverps5_lib::sources::{self, SourceState, http};

#[tokio::test]
#[ignore = "hits live websites"]
async fn live_refresh_parses_every_source() {
    let client = http::build_client().unwrap();
    let cache = sources::refresh(&client, &CatalogCache::default()).await;
    for s in &cache.statuses {
        println!(
            "{:<20} {:?} records={} host={:?} error={:?}",
            s.source_id, s.state, s.record_count, s.final_host, s.error
        );
        assert_eq!(s.state, SourceState::Fresh, "{} failed: {:?}", s.source_id, s.error);
    }
    let latest = cache.official_latest.as_ref().expect("official latest");
    println!("Sony current build {} -> version {:?}", latest.build_date, latest.version);
    let current = cache.mirror_by_url(&latest.reinstall_url).expect("current recovery mirror");
    println!("current recovery availability {:?} {:?}", current.availability, current.availability_note);
    assert_eq!(current.availability, Availability::Available);

    let manifest = SourcesManifest::parse(include_str!("../../manifests/sources.json")).unwrap();
    let entries = merge(cache.records.clone(), &manifest);
    let conflicts = entries.iter().filter(|e| e.evidence.md5_conflict || e.evidence.sha256_conflict).count();
    println!("catalog entries={} with source conflicts={}", entries.len(), conflicts);
}
