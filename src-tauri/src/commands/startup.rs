//! The startup screen: the programs Windows starts at sign-in, and turning one on or off.

use tauri::State;
use vitals_ipc::{ChangeOutcome, StartupOverview, StartupSetArgs};

use super::{AppState, CommandError};
use crate::startup::{self, StartupError};

impl From<StartupError> for CommandError {
    fn from(error: StartupError) -> Self {
        match error {
            StartupError::NotFound(_) | StartupError::Protected(_) | StartupError::NotMachine => {
                Self::Invalid(error.to_string())
            }
            StartupError::Registry(_) | StartupError::Elevated(_) => {
                Self::Io(std::io::Error::other(error.to_string()))
            }
        }
    }
}

/// Async: every signature is checked, which takes a moment.
#[tauri::command(async)]
pub fn startup_overview() -> StartupOverview {
    #[cfg(windows)]
    return startup::overview();
    #[cfg(not(windows))]
    StartupOverview {
        entries: Vec::new(),
        measured_boots: 0,
    }
}

/// Async: an entry of every user waits for the UAC prompt.
#[tauri::command(async)]
pub fn startup_set(
    state: State<'_, AppState>,
    source: vitals_ipc::StartupSource,
    name: String,
    enable: bool,
) -> Result<ChangeOutcome, CommandError> {
    let change = StartupSetArgs {
        source,
        name,
        enable,
    };
    let outcome = startup::set_enabled(&change, &state.data_dir)?;
    tracing::info!(
        "startup {:?} {:?} -> {}: {outcome:?}",
        change.source,
        change.name,
        if change.enable { "on" } else { "off" }
    );
    Ok(outcome)
}
