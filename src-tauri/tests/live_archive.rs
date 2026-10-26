//! Live check of the Midnight Archive path: refresh the catalog from the real
//! sources, download one file from the archive by its SHA-256 and compare.
//! Ignored by default: it contacts the archive and downloads one file
//! (about 830 MB for the default entry).
//!
//!     cargo test --test live_archive -- --ignored --nocapture
//!
//! `VR5_LIVE_ARCHIVE_KEY` picks another catalog entry, e.g. `14.20.00|update`.

use std::sync::atomic::AtomicBool;

use voidrecoverps5_lib::cache::CatalogCache;
use voidrecoverps5_lib::domain::catalog::merge;
use voidrecoverps5_lib::domain::manifest::SourcesManifest;
use voidrecoverps5_lib::domain::types::HashAlgorithm;
use voidrecoverps5_lib::download::{self, Server};
use voidrecoverps5_lib::sources::{self, http, midnight};

#[tokio::test]
#[ignore = "contacts the Midnight Archive and downloads one file"]
async fn download_from_the_archive_and_compare() {
    let key = std::env::var("VR5_LIVE_ARCHIVE_KEY").unwrap_or_else(|_| "02.20.00|update".into());
    let client = http::build_client().unwrap();
    let cache = sources::refresh(&client, &CatalogCache::default()).await;
    let status = cache.status(midnight::SOURCE_ID).expect("archive status");
    println!("archive: {:?}, {} records, host {:?}", status.state, status.record_count, status.final_host);
    let manifest = SourcesManifest::parse(include_str!("../../manifests/sources.json")).unwrap();
    let entries = merge(cache.records.clone(), &manifest);
    let entry = entries.iter().find(|e| e.key == key).expect("entry in the live catalog");
    for c in &entry.evidence.hash_claims {
        println!("claim {:?} {} from {:?}", c.algorithm, c.value, c.source_ids);
    }
    let record = entry.records.iter().find(|r| r.source_id == midnight::SOURCE_ID).expect("archive record");
    let mirror = &record.mirrors[0];
    println!("url {}", mirror.url);
    let dir = tempfile::tempdir().unwrap();
    let expected = entry.evidence.best_size_bytes.filter(|_| !entry.evidence.best_size_is_approximate);
    let (path, local) = download::download(
        &http::build_download_client().unwrap(),
        Server::MidnightArchive,
        &mirror.url,
        dir.path(),
        "live.PUP",
        expected,
        &AtomicBool::new(false),
        |_, _| {},
    )
    .await
    .unwrap();
    println!("downloaded {} bytes\nsha256 {}\nmd5    {}", local.size_bytes, local.sha256, local.md5);
    let sha_claims: Vec<_> =
        entry.evidence.hash_claims.iter().filter(|c| c.algorithm == HashAlgorithm::Sha256).collect();
    assert!(sha_claims.iter().any(|c| c.value == local.sha256), "downloaded bytes match no published SHA-256");
    let _ = std::fs::remove_file(path);
}
