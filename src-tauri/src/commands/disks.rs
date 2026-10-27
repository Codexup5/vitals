//! The disks screen: volumes and drive health, the background scan of the home, its treemap and cleanup preview.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::State;
use vitals_ipc::{CleanupPreview, CleanupReport, DisksOverview, ScanResult, ScanStatus};

use super::{AppState, CommandError, open};
use crate::cleanup::{Places, plan};

fn places() -> Result<Places, CommandError> {
    if let Some(root) = crate::app_paths::e2e_root() {
        return Ok(Places::under(&root));
    }
    let home = home()?;
    Places::from_env(home.clone(), code_roots(&home))
        .ok_or_else(|| CommandError::Invalid("LOCALAPPDATA or APPDATA is not set".to_owned()))
}

fn home() -> Result<PathBuf, CommandError> {
    if let Some(root) = crate::app_paths::e2e_root() {
        return Ok(Places::under(&root).home);
    }
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
    let places = places()?;
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

/// Lists what the checked candidates of the last scan will delete. The plan stays in memory until the run.
#[tauri::command(async)]
pub fn cleanup_preview(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<CleanupPreview, CommandError> {
    let scan =
        latest_scan(&state)?.ok_or_else(|| CommandError::Invalid("no scan yet".to_owned()))?;
    let chosen: Vec<_> = scan
        .candidates
        .into_iter()
        .filter(|candidate| ids.contains(&candidate.id))
        .collect();
    if chosen.is_empty() {
        return Err(CommandError::Invalid(
            "no candidate of the last scan was chosen".to_owned(),
        ));
    }
    let plan_id = u64::try_from(chrono::Utc::now().timestamp_micros()).unwrap_or_default();
    let plan = plan::preview(plan_id, &chosen, &places()?, std::time::SystemTime::now());
    let summary = plan.summary();
    *state
        .plan
        .lock()
        .map_err(|_| CommandError::Invalid("the plan is poisoned".to_owned()))? = Some(plan);
    Ok(summary)
}

/// Executes the preview with this id, logs it and takes what it freed off the stored scan.
#[tauri::command(async)]
pub fn cleanup_run(
    state: State<'_, AppState>,
    plan_id: u64,
) -> Result<CleanupReport, CommandError> {
    let plan = state
        .plan
        .lock()
        .map_err(|_| CommandError::Invalid("the plan is poisoned".to_owned()))?
        .take()
        .filter(|plan| plan.id == plan_id)
        .ok_or_else(|| CommandError::Invalid(format!("no preview {plan_id}: preview again")))?;
    let report = plan::run(
        &plan,
        &places()?,
        &state.data_dir,
        chrono::Utc::now().timestamp(),
    );
    if let Err(error) = plan::log(&report, &state.data_dir) {
        tracing::error!("the cleanup was not logged: {error}");
    }
    if let Some(mut scan) = latest_scan(&state)? {
        plan::shrink(&mut scan.candidates, &report);
        match serde_json::to_string(&scan) {
            Ok(body) => open(&state)?.put_scan(scan.finished_at, &body)?,
            Err(error) => tracing::warn!("the scan was not updated after the cleanup: {error}"),
        }
    }
    Ok(report)
}
