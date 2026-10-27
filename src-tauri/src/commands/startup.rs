//! The startup screen: the programs Windows starts at sign-in and the services, and turning one off or back.

use tauri::State;
use vitals_ipc::{
    ChangeOutcome, ServiceSetArgs, ServicesOverview, StartupOverview, StartupSetArgs,
};

use super::{AppState, CommandError, open};
use crate::services::{self, ServiceError};
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

impl From<ServiceError> for CommandError {
    fn from(error: ServiceError) -> Self {
        match error {
            ServiceError::Storage(error) => Self::Storage(error),
            ServiceError::NotFound(_)
            | ServiceError::Protected(_)
            | ServiceError::NothingToRestore(_)
            | ServiceError::NotAutomatic(_)
            | ServiceError::Forbidden => Self::Invalid(error.to_string()),
            ServiceError::Manager(_) | ServiceError::Elevated(_) => {
                Self::Io(std::io::Error::other(error.to_string()))
            }
        }
    }
}

/// Async: the signature of every service file is checked, which takes a few seconds.
#[tauri::command(async)]
pub fn services_overview(state: State<'_, AppState>) -> Result<ServicesOverview, CommandError> {
    #[cfg(windows)]
    return Ok(services::overview(&open(&state)?)?);
    #[cfg(not(windows))]
    Ok(ServicesOverview {
        services: Vec::new(),
    })
}

/// Async: the change waits for the UAC prompt.
#[tauri::command(async)]
pub fn service_set(
    state: State<'_, AppState>,
    name: String,
    restore: bool,
) -> Result<ChangeOutcome, CommandError> {
    let args = ServiceSetArgs { name, restore };
    let outcome = services::change(&args, &open(&state)?, &state.data_dir)?;
    tracing::info!(
        "service {:?} {}: {outcome:?}",
        args.name,
        if args.restore {
            "restored"
        } else {
            "to manual"
        }
    );
    Ok(outcome)
}
