//! The network screen: the ways out of the PC, the samples of the last day, a measurement now, the speed test.

use std::sync::Arc;

use std::sync::atomic::Ordering;

use tauri::State;
use vitals_ipc::{DevicesOverview, NetworkOverview, SpeedStatus};

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

/// The devices the scans saw: this PC and the router by the way out, the new ones for a week.
#[tauri::command(async)]
pub fn devices_overview(state: State<'_, AppState>) -> Result<DevicesOverview, CommandError> {
    let known = open(&state)?.known_devices()?;
    #[cfg(windows)]
    let way_out = network::windows::adapters()
        .into_iter()
        .find(|adapter| adapter.gateway.is_some());
    #[cfg(not(windows))]
    let way_out: Option<network::RawAdapter> = None;
    let this = way_out
        .as_ref()
        .and_then(|adapter| adapter.mac)
        .map(network::devices::mac_text);
    let gateway = way_out
        .as_ref()
        .and_then(|adapter| adapter.gateway)
        .map(|gateway| gateway.to_string());
    let devices = network::devices::devices_of(
        &known,
        this.as_deref(),
        gateway.as_deref(),
        chrono::Utc::now().timestamp(),
    );
    Ok(DevicesOverview {
        last_scan: known.iter().map(|device| device.last_seen).max(),
        devices,
        is_scanning: state.devices.is_running.load(Ordering::SeqCst),
    })
}

/// Starts a scan of the network in the background unless one runs; the overview tells when it ends.
#[tauri::command]
pub fn devices_scan(state: State<'_, AppState>) -> bool {
    if state.devices.is_running.load(Ordering::SeqCst) {
        return false;
    }
    let job = Arc::clone(&state.devices);
    let db_path = state.db_path.clone();
    std::thread::Builder::new()
        .name("devices".to_owned())
        .spawn(move || {
            job.run_once(|| network::devices::scan_and_store(&db_path));
        })
        .map_err(|error| tracing::error!("the devices scan did not start: {error}"))
        .is_ok()
}

#[tauri::command]
pub fn speed_test_start(state: State<'_, AppState>) -> bool {
    Arc::clone(&state.speed).start(state.data_dir.clone())
}

#[tauri::command]
pub fn speed_test_status(state: State<'_, AppState>) -> SpeedStatus {
    state.speed.status()
}
