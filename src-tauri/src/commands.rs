//! Tauri commands. The UI asks for specific actions; it never passes shell
//! commands, and network or file access happens only here.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::cache::CatalogCache;
use crate::domain::catalog::{CatalogEntry, OfficialLatest, SourceBehind, merge, sources_behind};
use crate::domain::manifest::{SourceRecord, SourcesManifest};
use crate::domain::types::MirrorMode;
use crate::integrity::{self, LocalHashes, VerifyReport};
use crate::sources::{self, SourceStatus};
use crate::volumes::VolumeProvider;

pub struct AppState {
    pub manifest: SourcesManifest,
    pub cache: Mutex<CatalogCache>,
    pub cache_note: Mutex<Option<String>>,
    pub cache_path: PathBuf,
    pub client: reqwest::Client,
    pub refreshing: AtomicBool,
    pub hash_cancel: Arc<AtomicBool>,
    pub volumes: Arc<dyn VolumeProvider>,
    /// No whole-request timeout, for large downloads.
    pub download_client: reqwest::Client,
    pub download_dir: PathBuf,
    pub download_cancel: AtomicBool,
    pub downloading: AtomicBool,
    pub build_cancel: Arc<AtomicBool>,
    pub building: AtomicBool,
    pub notebook: Mutex<crate::notebook::Notebook>,
    pub notebook_note: Mutex<Option<String>>,
    pub notebook_path: PathBuf,
    pub scan_cancel: Arc<AtomicBool>,
    pub scanning: AtomicBool,
}

#[derive(Serialize)]
pub struct CatalogView {
    pub catalog_version: String,
    pub app_version: &'static str,
    pub sources: Vec<SourceRecord>,
    pub statuses: Vec<SourceStatus>,
    pub entries: Vec<CatalogEntry>,
    pub official_latest: Option<OfficialLatest>,
    pub sources_behind: Vec<SourceBehind>,
    pub saved_at: Option<String>,
    pub cache_note: Option<String>,
}

fn view(state: &AppState) -> CatalogView {
    let cache = state.cache.lock().expect("cache lock").clone();
    let entries = merge(cache.records.clone(), &state.manifest);
    let behind = sources_behind(&entries, cache.official_latest.as_ref());
    // Every adapter-backed source gets a status row, even before the first fetch.
    let mut statuses = cache.statuses.clone();
    for s in state.manifest.sources.iter().filter(|s| s.adapter.is_some()) {
        if !statuses.iter().any(|st| st.source_id == s.source_id) {
            statuses.push(SourceStatus::never(&s.source_id));
        }
    }
    CatalogView {
        catalog_version: state.manifest.catalog_version.clone(),
        app_version: env!("CARGO_PKG_VERSION"),
        sources: state.manifest.sources.clone(),
        statuses,
        entries,
        official_latest: cache.official_latest.clone(),
        sources_behind: behind,
        saved_at: cache.saved_at.clone(),
        cache_note: state.cache_note.lock().expect("note lock").clone(),
    }
}

fn save(state: &AppState, cache: &CatalogCache) {
    let note = cache.save(&state.cache_path).err().map(|e| format!("The catalog could not be saved locally ({e})."));
    *state.cache_note.lock().expect("note lock") = note;
}

#[tauri::command]
pub fn get_catalog(state: State<'_, AppState>) -> CatalogView {
    view(&state)
}

/// Fetch all sources. Only runs when the user asks for it.
#[tauri::command]
pub async fn refresh_catalog(state: State<'_, AppState>) -> Result<CatalogView, String> {
    if state.refreshing.swap(true, Ordering::SeqCst) {
        return Err("A refresh is already running.".into());
    }
    let previous = state.cache.lock().expect("cache lock").clone();
    let fresh = sources::refresh(&state.client, &previous).await;
    save(&state, &fresh);
    *state.cache.lock().expect("cache lock") = fresh;
    state.refreshing.store(false, Ordering::SeqCst);
    Ok(view(&state))
}

/// Check whether Sony still serves the files for one catalog entry.
#[tauri::command]
pub async fn probe_sony(state: State<'_, AppState>, key: String) -> Result<CatalogView, String> {
    let mut cache = state.cache.lock().expect("cache lock").clone();
    let entries = merge(cache.records.clone(), &state.manifest);
    let entry = entries.iter().find(|e| e.key == key).ok_or("That catalog entry no longer exists.")?;
    let urls: Vec<String> = entry
        .records
        .iter()
        .filter(|r| r.source_id == sources::sony::SOURCE_ID)
        .flat_map(|r| r.mirrors.iter())
        .filter(|m| m.mode == MirrorMode::DirectHttps)
        .map(|m| m.url.clone())
        .collect();
    if urls.is_empty() {
        return Err("Sony has no download address for this package.".into());
    }
    for url in urls {
        sources::probe_sony_url(&state.client, &mut cache, &url).await;
    }
    save(&state, &cache);
    *state.cache.lock().expect("cache lock") = cache;
    Ok(view(&state))
}

#[derive(Clone, Serialize)]
struct Progress {
    done: u64,
    total: u64,
}

/// Hash a file the user picked and compare it with the catalog. Read-only.
#[tauri::command]
pub async fn verify_local_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    selected_key: Option<String>,
) -> Result<VerifyReport, String> {
    let path = PathBuf::from(path);
    let meta = std::fs::metadata(&path).map_err(|e| format!("Can't read that file: {e}"))?;
    if !meta.is_file() {
        return Err("That is not a file.".into());
    }
    let cancel = state.hash_cancel.clone();
    cancel.store(false, Ordering::SeqCst);
    let emitter = app.clone();
    let hash_path = path.clone();
    let local = tauri::async_runtime::spawn_blocking(move || {
        integrity::hash_file(&hash_path, &cancel, |done, total| {
            let _ = emitter.emit("verify-progress", Progress { done, total });
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(report(&state, &path, local, selected_key.as_deref()))
}

/// Re-compare already computed hashes against a different catalog entry.
#[tauri::command]
pub fn compare_local(
    state: State<'_, AppState>,
    file_name: String,
    local: LocalHashes,
    selected_key: Option<String>,
) -> VerifyReport {
    let entries = merge(state.cache.lock().expect("cache lock").records.clone(), &state.manifest);
    integrity::build_report(file_name, local, &entries, selected_key.as_deref())
}

/// Language hints for the window's *Automatic* setting.
#[tauri::command]
pub fn system_languages() -> crate::system_language::SystemLanguages {
    crate::system_language::read()
}

#[tauri::command]
pub fn cancel_verify(state: State<'_, AppState>) {
    state.hash_cancel.store(true, Ordering::SeqCst);
}

fn report(state: &AppState, path: &Path, local: LocalHashes, selected_key: Option<&str>) -> VerifyReport {
    let entries = merge(state.cache.lock().expect("cache lock").records.clone(), &state.manifest);
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    integrity::build_report(file_name, local, &entries, selected_key)
}
