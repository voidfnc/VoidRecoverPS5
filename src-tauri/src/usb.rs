//! Tauri commands for the recovery USB builder: list drives, download from
//! Sony or the Midnight Archive, preflight, build, and clean up after an interrupted run.
//!
//! The UI names a catalog entry, a file and a drive. Download addresses come
//! from the catalog, drive facts are re-read here, and the file is accepted
//! only if its hashes match the entry's published checksums.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::builder::{self, BuildError, Leftover, LeftoverAction, Phase, Preflight};
use crate::commands::AppState;
use crate::domain::catalog::{CatalogEntry, merge};
use crate::domain::types::{Availability, Mirror, MirrorMode, PackageType};
use crate::download::{self, DownloadError, Server};
use crate::integrity::{self, EntryCheck, EntryVerdict};
use crate::sources;
use crate::volumes::VolumeInfo;

/// Only update and recovery packages with a known mapping can be written.
/// Special variants and unknown packages never enter the builder.
pub fn buildable(entry: &CatalogEntry) -> Result<(), String> {
    buildable_kind(entry.package_type, entry.variant_label.as_deref())
}

fn buildable_kind(package_type: PackageType, variant: Option<&str>) -> Result<(), String> {
    match package_type {
        PackageType::Update | PackageType::Reinstall if variant.is_none() => Ok(()),
        _ => Err("Only standard update and recovery files can be written to a drive by this app.".into()),
    }
}

fn find_entry(state: &AppState, key: &str) -> Result<CatalogEntry, String> {
    let entries = merge(state.cache.lock().expect("cache lock").records.clone(), &state.manifest);
    entries.into_iter().find(|e| e.key == key).ok_or_else(|| "That catalog entry no longer exists.".to_string())
}

/// Drop a guard flag when the operation ends, however it ends.
struct Flag<'a>(&'a std::sync::atomic::AtomicBool);

impl<'a> Flag<'a> {
    fn take(flag: &'a std::sync::atomic::AtomicBool, busy_msg: &str) -> Result<Self, String> {
        if flag.swap(true, Ordering::SeqCst) { Err(busy_msg.to_string()) } else { Ok(Flag(flag)) }
    }
}

impl Drop for Flag<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
pub struct VolumeView {
    #[serde(flatten)]
    pub info: VolumeInfo,
    pub blockers: Vec<String>,
}

#[tauri::command]
pub async fn list_volumes(state: State<'_, AppState>) -> Result<Vec<VolumeView>, String> {
    let volumes = state.volumes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        volumes.list().map(|v| v.into_iter().map(|info| VolumeView { blockers: info.blockers(), info }).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Clone, Serialize)]
struct Progress {
    done: u64,
    total: u64,
}

#[derive(Serialize)]
pub struct DownloadView {
    pub path: String,
    pub check: EntryCheck,
}

fn file_name_for(entry: &CatalogEntry) -> String {
    let kind = if entry.package_type == PackageType::Reinstall { "reinstall" } else { "update" };
    let build: String = entry
        .build_date
        .as_deref()
        .unwrap_or("unknown")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    format!("PS5_{}_{kind}_{build}.PUP", entry.firmware_version)
}

/// Where a package may be downloaded from. The UI names one of these; the
/// address always comes from the catalog.
const DOWNLOAD_SOURCES: [(&str, Server); 2] =
    [(sources::sony::SOURCE_ID, Server::Sony), (sources::midnight::SOURCE_ID, Server::MidnightArchive)];

/// The mirror to download `entry` from at `source_id`, and the exact size to
/// expect. Refuses entries whose sources disagree about the checksum, and
/// mirrors the server is known not to serve.
pub fn pick_download(entry: &CatalogEntry, source_id: &str) -> Result<(Server, Mirror, Option<u64>), String> {
    let Some((_, server)) = DOWNLOAD_SOURCES.iter().find(|(id, _)| *id == source_id) else {
        return Err("That download source is not supported.".into());
    };
    if entry.evidence.sha256_conflict || entry.evidence.md5_conflict || entry.evidence.size_conflict {
        return Err("The sources disagree about this file's checksum or size, so the app will not download it. See the catalog entry.".into());
    }
    let mirror = entry
        .records
        .iter()
        .filter(|r| r.source_id == source_id)
        .flat_map(|r| r.mirrors.iter())
        .find(|m| m.mode == MirrorMode::DirectHttps)
        .ok_or_else(|| match server {
            Server::Sony => "Sony has no download address for this package.".to_string(),
            Server::MidnightArchive => "The Midnight Archive has no download address for this package.".to_string(),
        })?;
    if mirror.availability == Availability::Unavailable {
        return Err("Sony's server no longer offers this file.".into());
    }
    let expected = entry.evidence.best_size_bytes.filter(|_| !entry.evidence.best_size_is_approximate);
    Ok((*server, mirror.clone(), expected))
}

/// Download an entry's file into the app's download folder, then check it
/// against the entry's published checksums. `source` is Sony unless the user
/// chose the Midnight Archive.
#[tauri::command]
pub async fn download_package(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    source: Option<String>,
) -> Result<DownloadView, String> {
    let entry = find_entry(&state, &key)?;
    buildable(&entry)?;
    let (server, mirror, expected_size) = pick_download(&entry, source.as_deref().unwrap_or(sources::sony::SOURCE_ID))?;

    let _guard = Flag::take(&state.downloading, "A download is already running.")?;
    state.download_cancel.store(false, Ordering::SeqCst);
    download::clean_partials(&state.download_dir);
    let emitter = app.clone();
    let (path, local) = download::download(
        &state.download_client,
        server,
        &mirror.url,
        &state.download_dir,
        &file_name_for(&entry),
        expected_size,
        &state.download_cancel,
        |done, total| {
            let _ = emitter.emit("download-progress", Progress { done, total });
        },
    )
    .await
    .map_err(|e: DownloadError| e.to_string())?;

    let check = integrity::check_entry(&local, &entry);
    if check.verdict != EntryVerdict::HashMatches {
        let _ = std::fs::remove_file(&path);
        return Err(
            "The downloaded file does not match the checksums published for this entry, so it was deleted.".into()
        );
    }
    Ok(DownloadView { path: path.to_string_lossy().into_owned(), check })
}

#[tauri::command]
pub fn cancel_download(state: State<'_, AppState>) {
    state.download_cancel.store(true, Ordering::SeqCst);
}

#[derive(Serialize)]
pub struct PreflightView {
    pub preflight: Preflight,
    pub volume: VolumeInfo,
    pub source_name: String,
    pub source_size: u64,
}

/// Everything the user should see before writing. Reads metadata only; the
/// file's hashes are checked while it is copied.
#[tauri::command]
pub async fn preflight_build(
    state: State<'_, AppState>,
    key: String,
    source_path: String,
    volume_id: String,
) -> Result<PreflightView, String> {
    let entry = find_entry(&state, &key)?;
    buildable(&entry)?;
    let source = PathBuf::from(&source_path);
    let meta = std::fs::metadata(&source).map_err(|e| format!("Can't read that file: {e}"))?;
    if !meta.is_file() {
        return Err("That is not a file.".into());
    }
    let volumes = state.volumes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let volume = volumes
            .list()?
            .into_iter()
            .find(|v| v.id == volume_id)
            .ok_or_else(|| "The drive is no longer connected.".to_string())?;
        let preflight = builder::preflight(meta.len(), &volume, std::path::Path::new(&volume.mount_path));
        Ok(PreflightView {
            preflight,
            volume,
            source_name: source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            source_size: meta.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct BuildView {
    pub target: String,
    pub replaced_existing: bool,
    pub check: EntryCheck,
    pub volume_label: String,
    pub mount_path: String,
}

#[derive(Clone, Serialize)]
struct BuildProgress {
    phase: Phase,
    done: u64,
    total: u64,
}

/// Write the file to the drive. `expected` is the drive as the user saw it;
/// it must still match what the system reports right now.
#[tauri::command]
pub async fn build_usb(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    source_path: String,
    expected: VolumeInfo,
    replace_existing: bool,
) -> Result<BuildView, String> {
    let entry = find_entry(&state, &key)?;
    buildable(&entry)?;
    let _guard = Flag::take(&state.building, "A USB build is already running.")?;
    state.build_cancel.store(false, Ordering::SeqCst);
    let volumes = state.volumes.clone();
    let cancel = state.build_cancel.clone();
    let source = PathBuf::from(source_path);

    tauri::async_runtime::spawn_blocking(move || {
        let volume = volumes.recheck(&expected)?;
        let root = PathBuf::from(&volume.mount_path);
        let size = std::fs::metadata(&source).map_err(|e| format!("Can't read that file: {e}"))?.len();
        if let Some(first) = builder::preflight(size, &volume, &root).blockers.first() {
            return Err(first.clone());
        }
        let mut verdict: Option<EntryCheck> = None;
        let entry_for_check = entry.clone();
        let outcome = builder::build(
            &source,
            &root,
            &builder::new_run_id(),
            replace_existing,
            &cancel,
            |local| {
                let check = integrity::check_entry(local, &entry_for_check);
                let ok = check.verdict == EntryVerdict::HashMatches;
                verdict = Some(check);
                if ok {
                    Ok(())
                } else {
                    Err("This file does not match the checksums published for the selected entry, so nothing was \
                         written to the drive."
                        .to_string())
                }
            },
            |phase, done, total| {
                let _ = app.emit("build-progress", BuildProgress { phase, done, total });
            },
        )
        .map_err(|e: BuildError| e.to_string())?;
        Ok(BuildView {
            target: format!("{}PS5\\UPDATE\\{}", volume.mount_path, builder::TARGET_NAME),
            replaced_existing: outcome.replaced_existing,
            check: verdict.expect("validate ran before success"),
            volume_label: volume.label,
            mount_path: volume.mount_path,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_build(state: State<'_, AppState>) {
    state.build_cancel.store(true, Ordering::SeqCst);
}

#[derive(Serialize)]
pub struct LeftoverGroup {
    pub volume_id: String,
    pub volume_label: String,
    pub mount_path: String,
    pub leftovers: Vec<Leftover>,
}

/// Look for files an interrupted build left on allowed drives.
#[tauri::command]
pub async fn scan_leftovers(state: State<'_, AppState>) -> Result<Vec<LeftoverGroup>, String> {
    if state.building.load(Ordering::SeqCst) {
        return Ok(Vec::new());
    }
    let volumes = state.volumes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let groups = volumes
            .list()?
            .into_iter()
            .filter(|v| v.blockers().is_empty())
            .filter_map(|v| {
                let leftovers = builder::find_leftovers(std::path::Path::new(&v.mount_path));
                (!leftovers.is_empty()).then_some(LeftoverGroup {
                    volume_id: v.id,
                    volume_label: v.label,
                    mount_path: v.mount_path,
                    leftovers,
                })
            })
            .collect();
        Ok(groups)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftoverChoice {
    Delete,
    Restore,
}

#[tauri::command]
pub async fn resolve_leftover(
    state: State<'_, AppState>,
    volume_id: String,
    file_name: String,
    action: LeftoverChoice,
) -> Result<String, String> {
    let _guard = Flag::take(&state.building, "A USB build is running; try again when it has finished.")?;
    let volumes = state.volumes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let volume = volumes
            .list()?
            .into_iter()
            .find(|v| v.id == volume_id)
            .ok_or_else(|| "The drive is no longer connected.".to_string())?;
        if let Some(first) = volume.blockers().first() {
            return Err(first.clone());
        }
        let action = match action {
            LeftoverChoice::Delete => LeftoverAction::Delete,
            LeftoverChoice::Restore => LeftoverAction::Restore,
        };
        builder::resolve_leftover(std::path::Path::new(&volume.mount_path), &file_name, action)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Eject a drive after a build. `expected` is the drive as the user saw it.
#[tauri::command]
pub async fn eject_volume(state: State<'_, AppState>, expected: VolumeInfo) -> Result<(), String> {
    if state.building.load(Ordering::SeqCst) {
        return Err("A USB build is still running.".into());
    }
    let volumes = state.volumes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let volume = volumes.recheck(&expected)?;
        volumes.eject(&volume)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_update_and_recovery_entries_are_buildable() {
        assert!(buildable_kind(PackageType::Update, None).is_ok());
        assert!(buildable_kind(PackageType::Reinstall, None).is_ok());
        assert!(buildable_kind(PackageType::Update, Some("Lite")).is_err());
        assert!(buildable_kind(PackageType::SpecialVariant, None).is_err());
        assert!(buildable_kind(PackageType::Unknown, None).is_err());
    }

    mod download_choice {
        use super::*;
        use crate::domain::catalog::merge;
        use crate::domain::manifest::SourcesManifest;
        use crate::sources::{darthsternie, mewx, midnight, sony};

        fn entries() -> Vec<CatalogEntry> {
            let mut records = mewx::parse(include_str!("../../tests/fixtures/sources/mewx_readme.md"), "t").unwrap();
            let mut latest =
                sony::parse_support_page(include_str!("../../tests/fixtures/sources/sony_support_page.html")).unwrap();
            let sony_records = sony::records_from_index(&records, Some(&mut latest), "t");
            records.extend(sony_records);
            records.extend(
                darthsternie::parse_rest_json(include_str!("../../tests/fixtures/sources/darthsternie_page.json"), "t")
                    .unwrap()
                    .records,
            );
            records.extend(
                midnight::parse_listing(
                    include_str!("../../tests/fixtures/sources/midnight_ps5sys.html"),
                    midnight::LISTINGS[0],
                    "t",
                )
                .unwrap(),
            );
            records.extend(
                midnight::parse_listing(
                    include_str!("../../tests/fixtures/sources/midnight_ps5rec.html"),
                    midnight::LISTINGS[1],
                    "t",
                )
                .unwrap(),
            );
            let manifest = SourcesManifest::parse(include_str!("../../manifests/sources.json")).unwrap();
            merge(records, &manifest)
        }

        fn entry(key: &str) -> CatalogEntry {
            entries().into_iter().find(|e| e.key == key).unwrap_or_else(|| panic!("{key}"))
        }

        #[test]
        fn archive_mirror_is_offered_for_an_agreed_entry() {
            let e = entry("14.20.00|update");
            let (server, mirror, size) = pick_download(&e, midnight::SOURCE_ID).unwrap();
            assert_eq!(server, Server::MidnightArchive);
            assert!(mirror.url.contains("/download/0f81be6fd890bc04925b3953e0862dfc2ead68edfdc33f31c3c60d0c267f68e4/"));
            assert_eq!(size, Some(1_266_086_912), "the exact MewX size is used, not the archive's rounded one");
            let (server, mirror, _) = pick_download(&e, sony::SOURCE_ID).unwrap();
            assert_eq!(server, Server::Sony);
            assert!(mirror.url.starts_with("https://pc.ps5.update.playstation.net/"));
        }

        #[test]
        fn the_two_sources_never_cross() {
            let e = entry("14.20.00|update");
            for id in [sony::SOURCE_ID, midnight::SOURCE_ID] {
                let (_, m, _) = pick_download(&e, id).unwrap();
                assert_eq!(m.host == midnight::HOST_NAME, id == midnight::SOURCE_ID);
            }
        }

        #[test]
        fn disputed_checksum_blocks_every_download() {
            let e = entry("09.00.00|reinstall");
            assert!(e.evidence.sha256_conflict, "the archive lists a different 09.00 recovery file than MewX");
            assert!(pick_download(&e, midnight::SOURCE_ID).is_err());
            assert!(pick_download(&e, sony::SOURCE_ID).is_err());
        }

        #[test]
        fn unknown_source_is_refused() {
            assert!(pick_download(&entry("14.20.00|update"), "darthsternie-ps5").is_err());
            assert!(pick_download(&entry("14.20.00|update"), "https://example.org/x").is_err());
        }
    }
}
