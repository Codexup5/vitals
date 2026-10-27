mod app_paths;
mod cleanup;
mod collector;
mod commands;
mod disks;
mod dumps;
mod events;
mod import;
mod logging;
mod process;
mod scan;
mod storage;
mod system;
mod tray;
mod updates;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::{Local, Utc};
use tauri::{AppHandle, Manager, WindowEvent};

use crate::collector::Collector;
use crate::storage::Storage;

pub const MAIN_WINDOW: &str = "main";

/// Starts the panel: the log, the tray icon, the collector thread, the main window hidden to the tray on close.
#[must_use]
pub fn run() -> ExitCode {
    // The same exe under UAC for one admin action: it runs the action and exits, no window, no tray
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = cleanup::elevated::handle(&args) {
        return code;
    }
    let result = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::log_ui_error,
            commands::health_overview,
            commands::health_events,
            commands::minidumps,
            commands::reveal_minidump,
            commands::disks::disks_overview,
            commands::disks::scan_start,
            commands::disks::scan_status,
            commands::disks::reveal_path,
            commands::disks::cleanup_preview,
            commands::disks::cleanup_run,
            commands::updates::updates_status,
            commands::updates::updates_check,
            commands::updates::updates_run,
            commands::updates::updates_tools,
            commands::updates::open_update_tool
        ])
        .setup(|app| {
            let data_dir = app_paths::ensure_data_dir()?;
            logging::init(&data_dir);
            app.manage(commands::AppState {
                db_path: data_dir.join(storage::DB_FILE),
                data_dir: data_dir.clone(),
                scan: std::sync::Arc::default(),
                plan: std::sync::Mutex::default(),
                updates: std::sync::Arc::default(),
            });
            let is_russian = system::windows::ui_language_is_russian();
            tray::create(app.handle(), is_russian)?;
            start_collector(app.handle().clone(), data_dir, is_russian);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the panel in the tray; Quit in the tray menu ends it
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == MAIN_WINDOW
            {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    tracing::error!("could not hide the window: {error}");
                }
            }
        })
        .run(tauri::generate_context!());

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("the panel stopped: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The old PowerShell collector's history, imported once per day it lacks (stage 8 retires that collector).
fn legacy_health_json() -> Option<PathBuf> {
    if app_paths::e2e_root().is_some() {
        return None;
    }
    let profile = std::env::var_os("USERPROFILE")?;
    let path = Path::new(&profile)
        .join(".olegpc")
        .join("health")
        .join("health.json");
    path.is_file().then_some(path)
}

fn start_collector(app: AppHandle, data_dir: PathBuf, is_russian: bool) {
    let spawned = std::thread::Builder::new()
        .name("collector".to_owned())
        .spawn(move || {
            let storage = match Storage::open(&data_dir.join(storage::DB_FILE)) {
                Ok(storage) => storage,
                Err(error) => {
                    tracing::error!("the database did not open, the collector is off: {error}");
                    return;
                }
            };
            if let Some(path) = legacy_health_json() {
                match import::import_file(&storage, &path) {
                    Ok(report) => tracing::info!(
                        "health.json: {} days imported, {} skipped",
                        report.imported,
                        report.skipped
                    ),
                    Err(error) => tracing::warn!("health.json was not imported: {error}"),
                }
            }
            let mut collector = Collector::new(storage);
            loop {
                match collector.run_once(Utc::now()) {
                    Ok(summary) => tray::show_summary(&app, &summary, is_russian),
                    Err(error) => {
                        tracing::error!("the collector run failed: {error}");
                        if let Ok(summary) = collector.summary(Local::now().date_naive()) {
                            tray::show_summary(&app, &summary, is_russian);
                        }
                    }
                }
                std::thread::sleep(collector::INTERVAL);
            }
        });
    if let Err(error) = spawned {
        tracing::error!("the collector thread did not start: {error}");
    }
}

pub(crate) fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return Ok(());
    };
    window.unminimize()?;
    window.show()?;
    window.set_focus()
}
