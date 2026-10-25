pub mod builder;
pub mod cache;
pub mod commands;
pub mod domain;
pub mod download;
pub mod integrity;
pub mod notebook;
pub mod notebook_commands;
pub mod sources;
pub mod system_language;
pub mod usb;
pub mod volumes;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tauri::Manager;

use cache::CatalogCache;
use commands::AppState;
use domain::manifest::SourcesManifest;

/// The source registry is compiled in, so registry changes ship only with an
/// app update (no unsigned remote manifest).
const BUNDLED_MANIFEST: &str = include_str!("../../manifests/sources.json");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manifest = SourcesManifest::parse(BUNDLED_MANIFEST).expect("bundled sources.json is validated by tests");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let cache_path = app.path().app_data_dir()?.join("catalog-cache.json");
            let (cache, note) = CatalogCache::load(&cache_path);
            let notebook_path = app.path().app_data_dir()?.join("notebook.json");
            let (notebook, notebook_note) = notebook::Notebook::load(&notebook_path);
            app.manage(AppState {
                manifest,
                cache: Mutex::new(cache),
                cache_note: Mutex::new(note),
                cache_path,
                client: sources::http::build_client()?,
                refreshing: AtomicBool::new(false),
                hash_cancel: Arc::new(AtomicBool::new(false)),
                volumes: Arc::from(volumes::system_provider()),
                download_client: sources::http::build_download_client()?,
                download_dir: app.path().app_data_dir()?.join("downloads"),
                download_cancel: AtomicBool::new(false),
                downloading: AtomicBool::new(false),
                build_cancel: Arc::new(AtomicBool::new(false)),
                building: AtomicBool::new(false),
                notebook: Mutex::new(notebook),
                notebook_note: Mutex::new(notebook_note),
                notebook_path,
                scan_cancel: Arc::new(AtomicBool::new(false)),
                scanning: AtomicBool::new(false),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_catalog,
            commands::refresh_catalog,
            commands::probe_sony,
            commands::verify_local_file,
            commands::compare_local,
            commands::cancel_verify,
            commands::system_languages,
            usb::list_volumes,
            usb::download_package,
            usb::cancel_download,
            usb::preflight_build,
            usb::build_usb,
            usb::cancel_build,
            usb::scan_leftovers,
            usb::resolve_leftover,
            usb::eject_volume,
            notebook_commands::notebook_list,
            notebook_commands::notebook_scan,
            notebook_commands::notebook_cancel,
            notebook_commands::notebook_delete,
            notebook_commands::notebook_edit,
            notebook_commands::notebook_compare,
            notebook_commands::notebook_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
