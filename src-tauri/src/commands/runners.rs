//! The runners screen: the runners and containers, and the switch between the gaming and the coding mode.

use std::sync::Arc;

use tauri::State;
use vitals_ipc::{Mode, ModeStatus, RunnersOverview};

use super::AppState;
use crate::runners::source::{self, DockerStack};

#[tauri::command(async)]
pub fn runners_present() -> bool {
    source::is_present()
}

/// Async: `gh` and `docker` take a few seconds, the window must not wait on the main thread.
#[tauri::command(async)]
pub fn runners_overview() -> RunnersOverview {
    source::overview()
}

/// Starts a switch unless one runs; the status tells when it ends.
#[tauri::command]
pub fn mode_start(state: State<'_, AppState>, mode: Mode) -> bool {
    Arc::clone(&state.mode).start(mode, Arc::new(DockerStack), || {
        chrono::Utc::now().timestamp()
    })
}

#[tauri::command]
pub fn mode_status(state: State<'_, AppState>) -> ModeStatus {
    state.mode.status()
}
