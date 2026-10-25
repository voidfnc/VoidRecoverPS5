//! Tauri commands for the storage / path notebook. Folder paths come from the
//! native picker; the app only reads listings, and writes only the notebook
//! file in its own data folder plus an export file the user chose.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::commands::AppState;
use crate::notebook::{self, Diff, Format, Kind, SnapshotSummary};

fn save(state: &AppState) -> Result<(), String> {
    let nb = state.notebook.lock().expect("notebook lock");
    nb.save(&state.notebook_path).map_err(|e| format!("The notebook could not be saved: {e}"))
}

#[derive(Serialize)]
pub struct NotebookView {
    pub snapshots: Vec<SnapshotSummary>,
    pub note: Option<String>,
}

fn view(state: &AppState) -> NotebookView {
    NotebookView {
        snapshots: state.notebook.lock().expect("notebook lock").snapshots.iter().map(|s| s.summary()).collect(),
        note: state.notebook_note.lock().expect("note lock").clone(),
    }
}

#[tauri::command]
pub fn notebook_list(state: State<'_, AppState>) -> NotebookView {
    view(&state)
}

#[derive(Clone, Serialize)]
struct ScanProgress {
    count: u64,
}

/// Take a read-only snapshot of a folder the user picked.
#[tauri::command]
pub async fn notebook_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    label: String,
    note: String,
    kind: Kind,
) -> Result<NotebookView, String> {
    if state.scanning.swap(true, Ordering::SeqCst) {
        return Err("A scan is already running.".into());
    }
    state.scan_cancel.store(false, Ordering::SeqCst);
    let cancel = state.scan_cancel.clone();
    let root = PathBuf::from(path);
    let scanned = {
        let root = root.clone();
        tauri::async_runtime::spawn_blocking(move || {
            notebook::scan(&root, &cancel, |count| {
                let _ = app.emit("notebook-progress", ScanProgress { count });
            })
        })
        .await
    };
    state.scanning.store(false, Ordering::SeqCst);
    let scanned = scanned.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;

    let now = chrono::Utc::now();
    let label = {
        let l = label.trim();
        if l.is_empty() {
            root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.to_string_lossy().into())
        } else {
            l.chars().take(80).collect()
        }
    };
    let snap = notebook::snapshot_from(
        format!("s{}", now.timestamp_millis()),
        now.to_rfc3339(),
        label,
        note.chars().take(500).collect(),
        kind,
        &root,
        scanned,
    );
    state.notebook.lock().expect("notebook lock").add(snap)?;
    save(&state)?;
    Ok(view(&state))
}

#[tauri::command]
pub fn notebook_cancel(state: State<'_, AppState>) {
    state.scan_cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn notebook_delete(state: State<'_, AppState>, id: String) -> Result<NotebookView, String> {
    state.notebook.lock().expect("notebook lock").snapshots.retain(|s| s.id != id);
    save(&state)?;
    Ok(view(&state))
}

#[tauri::command]
pub fn notebook_edit(
    state: State<'_, AppState>,
    id: String,
    label: String,
    note: String,
) -> Result<NotebookView, String> {
    {
        let mut nb = state.notebook.lock().expect("notebook lock");
        let s = nb.snapshots.iter_mut().find(|s| s.id == id).ok_or("That snapshot no longer exists.")?;
        s.label = label.trim().chars().take(80).collect();
        s.note = note.chars().take(500).collect();
    }
    save(&state)?;
    Ok(view(&state))
}

#[tauri::command]
pub fn notebook_compare(state: State<'_, AppState>, older: String, newer: String) -> Result<Diff, String> {
    let nb = state.notebook.lock().expect("notebook lock");
    let a = nb.get(&older).ok_or("The first snapshot no longer exists.")?;
    let b = nb.get(&newer).ok_or("The second snapshot no longer exists.")?;
    Ok(notebook::compare(a, b))
}

/// Write a snapshot to a file chosen in the save dialog. Redaction is on
/// unless the UI turns it off explicitly.
#[tauri::command]
pub fn notebook_export(
    state: State<'_, AppState>,
    id: String,
    path: String,
    format: Format,
    redact: bool,
) -> Result<(), String> {
    let path = PathBuf::from(path);
    let ext_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(if format == Format::Json { "json" } else { "csv" }));
    if !ext_ok {
        return Err("The file name must end in .json or .csv to match the chosen format.".into());
    }
    let bytes = {
        let nb = state.notebook.lock().expect("notebook lock");
        let snap = nb.get(&id).ok_or("That snapshot no longer exists.")?;
        let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
        notebook::export(snap, format, redact, &user, &chrono::Utc::now().to_rfc3339())
    };
    notebook::write_atomic(&path, &bytes).map_err(|e| format!("Could not write the file: {e}"))
}
