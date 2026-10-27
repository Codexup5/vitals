//! The disks screen: volumes and drive health, the background scan of the home, its treemap and cleanup preview.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::State;
use vitals_ipc::{DisksOverview, ScanResult, ScanStatus};

use super::{AppState, CommandError, open};
use crate::cleanup::Places;

fn home() -> Result<PathBuf, CommandError> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| CommandError::Invalid("USERPROFILE is not set".to_owned()))
}

/// The brief, §3: code roots are where project caches and `node_modules` are looked for; the user's choice of
/// 27.10 is `Desktop\Code`, the settings screen will make the list editable.
fn code_roots(home: &Path) -> Vec<PathBuf> {
    vec![home.join("Desktop").join("Code")]
}

fn latest_scan(state: &AppState) -> Result<Option<ScanResult>, CommandError> {
    let body = open(state)?.latest_scan()?;
    Ok(body.and_then(|body| {
        serde_json::from_str(&body)
            .map_err(|error| tracing::warn!("the stored scan does not read: {error}"))
            .ok()
    }))
}

/// Async: Tauri runs it on a worker thread, whose COM apartment WMI can set up (the main thread has its own)
#[tauri::command(async)]
pub fn disks_overview(state: State<'_, AppState>) -> Result<DisksOverview, CommandError> {
    Ok(DisksOverview {
        volumes: crate::disks::windows::volumes(),
        physical_disks: crate::disks::windows::physical_disks(),
        scan: latest_scan(&state)?,
    })
}

#[tauri::command]
pub fn scan_start(state: State<'_, AppState>) -> Result<bool, CommandError> {
    let home = home()?;
    let places = Places::from_env(code_roots(&home))
        .ok_or_else(|| CommandError::Invalid("LOCALAPPDATA is not set".to_owned()))?;
    let db_path = state.db_path.clone();
    Ok(Arc::clone(&state.scan).start(home, places, move |result| {
        let stored = serde_json::to_string(&result)
            .map_err(|error| error.to_string())
            .and_then(|body| {
                crate::storage::Storage::open(&db_path)
                    .and_then(|storage| storage.put_scan(result.finished_at, &body))
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = stored {
            tracing::error!("the scan result was not saved: {error}");
        }
    }))
}

#[tauri::command]
pub fn scan_status(state: State<'_, AppState>) -> ScanStatus {
    ScanStatus {
        is_running: state.scan.is_running.load(Ordering::SeqCst),
        files: state.scan.progress.files.load(Ordering::Relaxed),
        bytes: state.scan.progress.bytes.load(Ordering::Relaxed),
    }
}

/// Opens Explorer on a folder or file the last scan listed; any other path is refused.
#[tauri::command]
pub fn reveal_path(state: State<'_, AppState>, path: &str) -> Result<(), CommandError> {
    let scan =
        latest_scan(&state)?.ok_or_else(|| CommandError::Invalid("no scan yet".to_owned()))?;
    let wanted = Path::new(path);
    let is_listed = scan.tiles.iter().any(|tile| tile.path == wanted)
        || scan
            .candidates
            .iter()
            .any(|candidate| candidate.path.as_deref() == Some(wanted));
    if !is_listed || !wanted.exists() {
        return Err(CommandError::Invalid(format!(
            "not a path of the last scan: {path}"
        )));
    }
    let argument = if wanted.is_dir() {
        wanted.display().to_string()
    } else {
        format!("/select,{}", wanted.display())
    };
    std::process::Command::new("explorer.exe")
        .arg(argument)
        .spawn()?;
    Ok(())
}
