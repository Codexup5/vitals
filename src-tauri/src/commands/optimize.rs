//! The Optimization section: the catalog as it stands on this machine, the plan of the checked settings, apply under
//! one UAC, the journal and its revert, and the Explorer restart some settings need.

use tauri::State;
use vitals_ipc::{
    ApplyOutcome, JournalChange, JournalRun, OptimizeOverview, OptimizePlan, PreviewArgs, TweakId,
};

use super::{AppState, CommandError, open};
use crate::optimize::OptimizeError;

/// One apply or revert at a time: two would share the files of the elevated action and race on the journal.
static RUNNING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn exclusive() -> Result<std::sync::MutexGuard<'static, ()>, CommandError> {
    RUNNING
        .try_lock()
        .map_err(|_| CommandError::Invalid("another optimization run is in progress".to_owned()))
}

impl From<OptimizeError> for CommandError {
    fn from(error: OptimizeError) -> Self {
        match error {
            OptimizeError::Storage(error) => Self::Storage(error),
            OptimizeError::Unknown(_) | OptimizeError::Refused(_) => {
                Self::Invalid(error.to_string())
            }
        }
    }
}

#[cfg(windows)]
fn machine(state: &AppState) -> crate::optimize::windows::Windows {
    crate::optimize::windows::Windows {
        work_dir: state.data_dir.clone(),
    }
}

#[cfg(not(windows))]
fn only_on_windows() -> CommandError {
    CommandError::Invalid("the optimizer exists only on Windows".to_owned())
}

/// Async: the hardware facts ask the graphics driver.
#[cfg_attr(
    windows,
    allow(clippy::unnecessary_wraps, reason = "an error only on other systems")
)]
#[tauri::command(async)]
pub fn optimize_overview(state: State<'_, AppState>) -> Result<OptimizeOverview, CommandError> {
    #[cfg(windows)]
    {
        let facts = crate::optimize::windows::facts();
        let rows = crate::optimize::plan::rows_of(&machine(&state), &facts);
        Ok(OptimizeOverview { facts, rows })
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        Err(only_on_windows())
    }
}

#[cfg_attr(
    windows,
    allow(clippy::unnecessary_wraps, reason = "an error only on other systems")
)]
#[tauri::command(async)]
pub fn optimize_preview(
    state: State<'_, AppState>,
    checked: Vec<TweakId>,
    enable_protection: bool,
) -> Result<OptimizePlan, CommandError> {
    #[cfg(windows)]
    {
        let _ = enable_protection;
        let reader = machine(&state);
        let facts = crate::optimize::windows::facts();
        Ok(crate::optimize::plan::plan_of(&reader, &facts, &checked).to_ipc(&reader))
    }
    #[cfg(not(windows))]
    {
        let _ = (state, checked, enable_protection);
        Err(only_on_windows())
    }
}

/// Async: a run with an admin item waits for the UAC prompt.
#[tauri::command(async)]
pub fn optimize_apply(
    state: State<'_, AppState>,
    checked: Vec<TweakId>,
    enable_protection: bool,
) -> Result<ApplyOutcome, CommandError> {
    let _running = exclusive()?;
    let storage = open(&state)?;
    let args = PreviewArgs {
        checked,
        enable_protection,
    };
    #[cfg(windows)]
    {
        let facts = crate::optimize::windows::facts();
        let now = chrono::Utc::now().timestamp();
        let outcome =
            crate::optimize::apply::apply(&mut machine(&state), &storage, &facts, &args, now)?;
        tracing::info!(
            "optimize: {:?}, run {:?}, {} settings, restore point {:?}",
            outcome.outcome,
            outcome.run,
            outcome.items.len(),
            outcome.restore_point
        );
        Ok(outcome)
    }
    #[cfg(not(windows))]
    {
        let _ = (storage, args);
        Err(only_on_windows())
    }
}

#[tauri::command(async)]
pub fn optimize_journal(state: State<'_, AppState>) -> Result<Vec<JournalRun>, CommandError> {
    let storage = open(&state)?;
    #[cfg(windows)]
    {
        let reader = machine(&state);
        Ok(storage
            .optimize_runs()?
            .into_iter()
            .map(|run| JournalRun {
                run: run.run,
                time: run.time,
                changes: run
                    .changes
                    .iter()
                    .map(|change| JournalChange {
                        id: change.id,
                        change: crate::optimize::plan::shown(&change.change, &reader),
                        reverted_at: change.reverted_at,
                    })
                    .collect(),
            })
            .collect())
    }
    #[cfg(not(windows))]
    {
        let _ = storage;
        Err(only_on_windows())
    }
}

/// Async: a run with an admin change waits for the UAC prompt.
#[tauri::command(async)]
pub fn optimize_revert(state: State<'_, AppState>, run: i64) -> Result<ApplyOutcome, CommandError> {
    let _running = exclusive()?;
    let storage = open(&state)?;
    #[cfg(windows)]
    {
        let now = chrono::Utc::now().timestamp();
        let outcome = crate::optimize::apply::revert(&mut machine(&state), &storage, run, now)?;
        tracing::info!(
            "optimize revert of run {run}: {:?}, {} settings",
            outcome.outcome,
            outcome.items.len()
        );
        Ok(outcome)
    }
    #[cfg(not(windows))]
    {
        let _ = (storage, run);
        Err(only_on_windows())
    }
}

/// Async: Explorer takes a couple of seconds to come back.
#[tauri::command(async)]
pub fn explorer_restart() -> Result<(), CommandError> {
    #[cfg(windows)]
    {
        crate::optimize::windows::restart_explorer()
            .map_err(|message| CommandError::Io(std::io::Error::other(message)))?;
        tracing::info!("Explorer restarted");
        Ok(())
    }
    #[cfg(not(windows))]
    Err(only_on_windows())
}
