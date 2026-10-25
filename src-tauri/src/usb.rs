//! Tauri commands for the recovery USB builder: list drives, download from
//! Sony, preflight, build, and clean up after an interrupted run.
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
use crate::domain::types::{Availability, MirrorMode, PackageType};
use crate::download::{self, DownloadError};
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

/// Download an entry's file from Sony into the app's download folder, then
/// check it against the entry's published checksums.
#[tauri::command]
pub async fn download_package(app: AppHandle, state: State<'_, AppState>, key: String) -> Result<DownloadView, String> {
    let entry = find_entry(&state, &key)?;
    buildable(&entry)?;
    let record = entry
        .records
        .iter()
        .filter(|r| r.source_id == sources::sony::SOURCE_ID)
        .find(|r| r.mirrors.iter().any(|m| m.mode == MirrorMode::DirectHttps))
        .ok_or("Sony has no download address for this package.")?;
    let mirror = record.mirrors.iter().find(|m| m.mode == MirrorMode::DirectHttps).expect("filtered above");
    if mirror.availability == Availability::Unavailable {
        return Err("Sony's server no longer offers this file.".into());
    }
    let expected_size = record.published_size_bytes.filter(|_| !record.size_is_approximate);

    let _guard = Flag::take(&state.downloading, "A download is already running.")?;
    state.download_cancel.store(false, Ordering::SeqCst);
    download::clean_partials(&state.download_dir);
    let emitter = app.clone();
    let (path, local) = download::download(
        &state.download_client,
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
}
