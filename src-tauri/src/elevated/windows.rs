//! Starting the panel's own exe under UAC for one admin action, and waiting for it.
#![allow(
    unsafe_code,
    reason = "ShellExecuteExW and the process handle of the elevated action live in this file"
)]

use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HSTRING, PCWSTR, w};

/// How an elevated start ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Elevated {
    /// The action ran; its exit code
    Exited(u32),
    /// The user said no at the UAC prompt
    Declined,
    /// It never started
    Failed(String),
    /// It started and was lost track of: what it did is unknown
    Lost(String),
}

/// Starts `exe` with `parameters` under UAC ("runas") and waits for it to end.
pub fn run_elevated(exe: &Path, parameters: &str) -> Elevated {
    let file = HSTRING::from(exe.as_os_str());
    let parameters = HSTRING::from(parameters);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised, its strings outlive the call, and cbSize names its real size.
    if let Err(error) = unsafe { ShellExecuteExW(&raw mut info) } {
        return if error.code() == ERROR_CANCELLED.to_hresult() {
            Elevated::Declined
        } else {
            Elevated::Failed(error.message())
        };
    }
    let process = info.hProcess;
    if process.is_invalid() {
        return Elevated::Failed("no process handle".to_owned());
    }
    // SAFETY: `process` is the valid handle SEE_MASK_NOCLOSEPROCESS returned; it is closed once below.
    let waited = unsafe { WaitForSingleObject(process, INFINITE) };
    let mut code = 1;
    // SAFETY: the handle is still open and `code` is a valid out pointer.
    let read = unsafe { GetExitCodeProcess(process, &raw mut code) };
    // SAFETY: the handle came from ShellExecuteExW and is closed exactly once.
    if let Err(error) = unsafe { CloseHandle(process) } {
        tracing::warn!("could not close the elevated process handle: {error}");
    }
    if waited != WAIT_OBJECT_0 || read.is_err() {
        return Elevated::Lost("the elevated action did not report back".to_owned());
    }
    Elevated::Exited(code)
}
