//! The sensors screen: the live readings it polls every two seconds, and the five-minute history.

use tauri::{State, WebviewWindow};
use vitals_ipc::{MAX_SENSOR_HOURS, SensorPoint, SensorsLive};

use super::{AppState, CommandError, open};

/// The latest reading and the last ten minutes. An ask from a shown window keeps the live sampler running; the first
/// one starts it and waits for its first reading. Async: that wait stays off the main thread.
#[tauri::command(async)]
pub fn sensors_live(window: WebviewWindow, state: State<'_, AppState>) -> SensorsLive {
    // A minimized or hidden window still polls: WebView2 keeps its page visible. It gets the last points, and the
    // sampler stops after its idle time; a closed window is gone with its page, and only the five-minute sample runs
    let is_shown = window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false);
    if is_shown {
        #[cfg(windows)]
        crate::sensors::start_live(&state.sensors);
        state.sensors.wait_reading();
    }
    state.sensors.snapshot(chrono::Utc::now().timestamp())
}

/// The five-minute samples of the last `hours` hours, oldest first.
#[tauri::command(async)]
pub fn sensors_history(
    state: State<'_, AppState>,
    hours: u32,
) -> Result<Vec<SensorPoint>, CommandError> {
    if !(1..=MAX_SENSOR_HOURS).contains(&hours) {
        return Err(CommandError::Invalid(format!("history of {hours} hours")));
    }
    let since = chrono::Utc::now().timestamp() - i64::from(hours) * 60 * 60;
    Ok(open(&state)?.sensor_samples_since(since)?)
}
