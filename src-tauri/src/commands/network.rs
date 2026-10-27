//! The network screen: the ways out of the PC, the samples of the last day, a measurement now, the speed test.

use std::sync::Arc;

use tauri::State;
use vitals_ipc::{NetworkOverview, SpeedStatus};

use super::{AppState, CommandError, open};
use crate::network;

const DAY_SECONDS: i64 = 24 * 60 * 60;

fn overview_of(state: &AppState) -> Result<NetworkOverview, CommandError> {
    let since = chrono::Utc::now().timestamp() - DAY_SECONDS;
    Ok(NetworkOverview {
        #[cfg(windows)]
        adapters: network::adapters(),
        #[cfg(not(windows))]
        adapters: Vec::new(),
        samples: open(state)?.net_samples_since(since)?,
        internet_host: network::INTERNET_HOST.to_string(),
    })
}

/// Async: IP Helper and the database are read off the main thread.
#[tauri::command(async)]
pub fn network_overview(state: State<'_, AppState>) -> Result<NetworkOverview, CommandError> {
    overview_of(&state)
}

/// Measures now (two bursts of ten pings, a few seconds), keeps the sample, and answers with the overview.
#[tauri::command(async)]
pub fn network_probe(state: State<'_, AppState>) -> Result<NetworkOverview, CommandError> {
    #[cfg(windows)]
    if let Some(sample) = network::sample(chrono::Utc::now().timestamp()) {
        open(&state)?.put_net_sample(&sample)?;
    }
    overview_of(&state)
}

#[tauri::command]
pub fn speed_test_start(state: State<'_, AppState>) -> bool {
    Arc::clone(&state.speed).start(state.data_dir.clone())
}

#[tauri::command]
pub fn speed_test_status(state: State<'_, AppState>) -> SpeedStatus {
    state.speed.status()
}
