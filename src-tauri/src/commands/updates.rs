//! The updates screen: the background check and run of winget, and the native tools the panel only opens.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use vitals_ipc::{UpdateTool, UpdateTools, UpdatesStatus};

use super::{AppState, CommandError};
use crate::updates::{gpu_tool, winget::WingetHost};

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn program_files() -> PathBuf {
    std::env::var_os("ProgramFiles")
        .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from)
}

#[tauri::command]
pub fn updates_status(state: State<'_, AppState>) -> UpdatesStatus {
    state.updates.status()
}

/// Starts a check unless the job is busy; the status tells when it ends.
#[tauri::command]
pub fn updates_check(state: State<'_, AppState>) -> bool {
    Arc::clone(&state.updates).check(Arc::new(WingetHost), now)
}

/// Starts the upgrades of the checked packages of the last check; any other id is ignored.
#[tauri::command]
pub fn updates_run(state: State<'_, AppState>, ids: Vec<String>) -> Result<bool, CommandError> {
    if ids.is_empty() {
        return Err(CommandError::Invalid("no package was chosen".to_owned()));
    }
    Ok(Arc::clone(&state.updates).run(&ids, Arc::new(WingetHost), now))
}

#[tauri::command]
pub fn updates_tools() -> UpdateTools {
    UpdateTools {
        has_gpu_tool: gpu_tool(&program_files()).is_some(),
        #[cfg(windows)]
        firmware: crate::updates::windows::firmware(),
        #[cfg(not(windows))]
        firmware: vitals_ipc::Firmware {
            board: None,
            version: None,
            released: None,
        },
    }
}

/// Opens Windows Update, the Store's updates or the GPU vendor's app: fixed targets, nothing from the window.
#[tauri::command]
pub fn open_update_tool(tool: UpdateTool) -> Result<(), CommandError> {
    match tool {
        UpdateTool::WindowsUpdate => {
            std::process::Command::new("explorer.exe")
                .arg("ms-settings:windowsupdate")
                .spawn()?;
        }
        UpdateTool::Store => {
            std::process::Command::new("explorer.exe")
                .arg("ms-windows-store://downloadsandupdates")
                .spawn()?;
        }
        UpdateTool::GpuDriver => {
            let exe = gpu_tool(&program_files())
                .ok_or_else(|| CommandError::Invalid("no GPU vendor app".to_owned()))?;
            std::process::Command::new(exe).spawn()?;
        }
    }
    Ok(())
}
