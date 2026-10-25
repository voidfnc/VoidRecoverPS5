//! End-to-end check of download -> verify -> write on a real USB drive.
//! Ignored by default: it downloads Sony's current update file (about 1 GB)
//! and writes it to the drive named in `VR5_LIVE_USB_MOUNT` (e.g. `D:\`).
//!
//!     $env:VR5_LIVE_USB_MOUNT = "D:\"
//!     cargo test --test live_usb -- --ignored --nocapture
//!
//! The drive must pass the app's own checks (USB, FAT32/exFAT, not the system
//! drive). The test creates only `PS5\UPDATE\PS5UPDATE.PUP` and removes it,
//! and any folders it created, when it finishes.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use voidrecoverps5_lib::builder::{self, BuildError, TARGET_NAME};
use voidrecoverps5_lib::cache::CatalogCache;
use voidrecoverps5_lib::domain::catalog::merge;
use voidrecoverps5_lib::domain::manifest::SourcesManifest;
use voidrecoverps5_lib::domain::types::{HashAlgorithm, MirrorMode, PackageType};
use voidrecoverps5_lib::download;
use voidrecoverps5_lib::integrity::{self, EntryVerdict};
use voidrecoverps5_lib::sources::{self, http};
use voidrecoverps5_lib::volumes::system_provider;

#[tokio::test]
#[ignore = "downloads ~1 GB and writes to a real USB drive"]
async fn download_verify_and_write_to_real_usb() {
    let mount = std::env::var("VR5_LIVE_USB_MOUNT").expect("set VR5_LIVE_USB_MOUNT, e.g. D:\\");
    let volumes = system_provider().list().unwrap();
    let volume = volumes
        .into_iter()
        .find(|v| v.mount_path.eq_ignore_ascii_case(&mount))
        .unwrap_or_else(|| panic!("no volume mounted at {mount}"));
    println!("drive: {volume:#?}");
    assert!(volume.blockers().is_empty(), "drive not allowed: {:?}", volume.blockers());
    let root = PathBuf::from(&volume.mount_path);
    let had_ps5_dir = root.join("PS5").exists();
    let had_update_dir = builder::target_dir(&root).exists();

    // Catalog and the current update entry.
    let client = http::build_client().unwrap();
    let cache = sources::refresh(&client, &CatalogCache::default()).await;
    let latest = cache.official_latest.clone().expect("official latest");
    let manifest = SourcesManifest::parse(include_str!("../../manifests/sources.json")).unwrap();
    let entries = merge(cache.records.clone(), &manifest);
    let entry = entries
        .iter()
        .find(|e| {
            e.package_type == PackageType::Update
                && e.variant_label.is_none()
                && e.evidence.hash_claims.iter().any(|c| {
                    c.algorithm == HashAlgorithm::Sha256 && c.value.eq_ignore_ascii_case(&latest.update_sha256)
                })
        })
        .expect("catalog entry for Sony's current update");
    println!("entry: {} {:?} build {:?}", entry.firmware_version, entry.package_type, entry.build_date);
    let record = entry.records.iter().find(|r| r.source_id == sources::sony::SOURCE_ID).expect("sony record");
    let mirror = record.mirrors.iter().find(|m| m.mode == MirrorMode::DirectHttps).expect("direct mirror");
    let expected_size = record.published_size_bytes.filter(|_| !record.size_is_approximate);

    // Download with progress.
    let dl_dir = std::env::temp_dir().join("vr5-live-usb-test");
    let _ = std::fs::remove_dir_all(&dl_dir);
    let dl_client = http::build_download_client().unwrap();
    let mut last = 0u64;
    let (src, local) = download::download(
        &dl_client,
        &mirror.url,
        &dl_dir,
        "live-test.PUP",
        expected_size,
        &AtomicBool::new(false),
        |done, total| {
            if done - last >= 100 * 1024 * 1024 || done == total {
                println!("download {done}/{total}");
                last = done;
            }
        },
    )
    .await
    .expect("download");
    let check = integrity::check_entry(&local, entry);
    println!("download {:?} {:?}", check.verdict, local);
    assert_eq!(check.verdict, EntryVerdict::HashMatches, "downloaded file must match the catalog");
    println!("SHA-256 equals the 64-hex segment of Sony's URL: {}", mirror.url.contains(&local.sha256));

    // Preflight, build, refuse overwrite, replace.
    let pre = builder::preflight(local.size_bytes, &volume, &root);
    println!("preflight: {pre:#?}");
    assert!(pre.blockers.is_empty());
    let cancel = AtomicBool::new(false);
    let validate = |l: &integrity::LocalHashes| {
        if integrity::check_entry(l, entry).verdict == EntryVerdict::HashMatches {
            Ok(())
        } else {
            Err("mismatch".to_string())
        }
    };
    let out = builder::build(&src, &root, &builder::new_run_id(), false, &cancel, validate, |p, d, t| {
        if d == t {
            println!("{p:?} done {d}");
        }
    })
    .expect("build");
    assert!(!out.replaced_existing);
    let target = builder::target_dir(&root).join(TARGET_NAME);
    assert_eq!(std::fs::metadata(&target).unwrap().len(), local.size_bytes);

    let again = builder::build(&src, &root, &builder::new_run_id(), false, &cancel, validate, |_, _, _| {});
    assert!(matches!(again, Err(BuildError::TargetExists)));
    let replaced =
        builder::build(&src, &root, &builder::new_run_id(), true, &cancel, validate, |_, _, _| {}).expect("replace");
    assert!(replaced.replaced_existing);
    assert!(builder::find_leftovers(&root).is_empty(), "no .tmp/.bak may remain");
    let rehash = integrity::hash_file(&target, &cancel, |_, _| {}).unwrap();
    assert_eq!(rehash, local);
    println!("OK: file on drive re-hashed to {}", rehash.sha256);

    // Clean up only what this test created.
    cleanup(&root, had_ps5_dir, had_update_dir);
    let _ = std::fs::remove_dir_all(&dl_dir);
}

fn cleanup(root: &Path, had_ps5_dir: bool, had_update_dir: bool) {
    let _ = std::fs::remove_file(builder::target_dir(root).join(TARGET_NAME));
    // FAT drives can lag behind a delete; retry the folder removals briefly.
    let remove_dir = |dir: PathBuf| {
        for _ in 0..10 {
            if std::fs::remove_dir(&dir).is_ok() || !dir.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        eprintln!("could not remove {}; delete it by hand", dir.display());
    };
    if !had_update_dir {
        remove_dir(builder::target_dir(root));
    }
    if !had_ps5_dir {
        remove_dir(root.join("PS5"));
    }
}

/// Read-only snapshot of the drive, then (only with `VR5_LIVE_EJECT=1`) eject it.
/// After the eject the drive must be unplugged and plugged in again to be used.
#[test]
#[ignore = "touches a real USB drive; ejects it when VR5_LIVE_EJECT=1"]
fn snapshot_and_eject_real_usb() {
    let mount = std::env::var("VR5_LIVE_USB_MOUNT").expect("set VR5_LIVE_USB_MOUNT, e.g. D:\\");
    let provider = system_provider();
    let volume = provider
        .list()
        .unwrap()
        .into_iter()
        .find(|v| v.mount_path.eq_ignore_ascii_case(&mount))
        .unwrap_or_else(|| panic!("no volume mounted at {mount}"));
    assert!(volume.blockers().is_empty(), "drive not allowed: {:?}", volume.blockers());

    let started = std::time::Instant::now();
    let scanned =
        voidrecoverps5_lib::notebook::scan(Path::new(&volume.mount_path), &AtomicBool::new(false), |_| {}).unwrap();
    println!(
        "scanned {} entries in {:?} (truncated={}, skipped={})",
        scanned.entries.len(),
        started.elapsed(),
        scanned.truncated,
        scanned.skipped
    );
    assert!(!scanned.entries.is_empty());

    if std::env::var("VR5_LIVE_EJECT").as_deref() == Ok("1") {
        let fresh = provider.recheck(&volume).expect("same drive still present");
        match provider.eject(&fresh) {
            Ok(()) => println!("ejected {}", fresh.mount_path),
            Err(e) => panic!("eject failed: {e}"),
        }
    }
}
