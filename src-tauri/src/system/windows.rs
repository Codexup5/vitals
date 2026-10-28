//! Disk space, memory, uptime and the running exes from Win32; BIOS, memory speed, the NVIDIA driver and stability from WMI.
#![allow(
    unsafe_code,
    reason = "Win32 calls of the machine snapshot live in this file"
)]

use std::collections::HashMap;

use serde::Deserialize;
use std::path::PathBuf;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::HSTRING;
use windows::core::PWSTR;
use wmi::{COMLibrary, Variant, WMIConnection};

use super::{FastFacts, SlowFacts, nvidia_release};

const DRIVE_FIXED: u32 = 3;

pub fn fast_facts() -> FastFacts {
    FastFacts {
        disks: fixed_disks(),
        ram_free_bytes: ram_free_bytes(),
        uptime_ms: uptime_ms(),
    }
}

fn fixed_disks() -> Vec<(char, u64)> {
    // SAFETY: GetLogicalDrives takes no arguments and returns a bit mask.
    let mask = unsafe { GetLogicalDrives() };
    ('A'..='Z')
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .filter_map(|(_, letter)| {
            let root = HSTRING::from(format!("{letter}:\\"));
            // SAFETY: `root` is a valid null-terminated path for the duration of the call.
            if unsafe { GetDriveTypeW(&root) } != DRIVE_FIXED {
                return None;
            }
            let mut free = 0u64;
            // SAFETY: `root` is valid and `free` is a live u64 the call writes into.
            unsafe { GetDiskFreeSpaceExW(&root, Some(&raw mut free), None, None) }.ok()?;
            Some((letter, free))
        })
        .collect()
}

/// The physical memory and the part of it available, in bytes.
pub fn memory_status() -> Option<(u64, u64)> {
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: `status` is a live MEMORYSTATUSEX with dwLength set, as the call requires.
    unsafe { GlobalMemoryStatusEx(&raw mut status) }.ok()?;
    Some((status.ullTotalPhys, status.ullAvailPhys))
}

fn ram_free_bytes() -> Option<u64> {
    memory_status().map(|(_, available)| available)
}

fn uptime_ms() -> u64 {
    // SAFETY: GetTickCount64 takes no arguments.
    unsafe { GetTickCount64() }
}

#[derive(Deserialize)]
#[serde(rename = "Win32_BIOS", rename_all = "PascalCase")]
struct Bios {
    #[serde(rename = "SMBIOSBIOSVersion")]
    smbios_bios_version: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_PhysicalMemory", rename_all = "PascalCase")]
struct Memory {
    configured_clock_speed: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_VideoController", rename_all = "PascalCase")]
struct Video {
    name: Option<String>,
    driver_version: Option<String>,
}

/// Every fact is optional: one WMI class failing leaves the others in place.
pub fn slow_facts() -> SlowFacts {
    let connection = match COMLibrary::new().and_then(WMIConnection::new) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!("WMI is not available: {error}");
            return SlowFacts::default();
        }
    };
    let bios = connection
        .query::<Bios>()
        .map_err(|error| tracing::warn!("Win32_BIOS: {error}"))
        .ok()
        .and_then(|rows| rows.into_iter().find_map(|row| row.smbios_bios_version));
    let ram_mts = connection
        .query::<Memory>()
        .map_err(|error| tracing::warn!("Win32_PhysicalMemory: {error}"))
        .ok()
        .and_then(|rows| rows.into_iter().find_map(|row| row.configured_clock_speed));
    let gpu_driver = connection
        .query::<Video>()
        .map_err(|error| tracing::warn!("Win32_VideoController: {error}"))
        .ok()
        .and_then(|rows| {
            rows.into_iter()
                .find(|row| {
                    row.name
                        .as_deref()
                        .is_some_and(|name| name.contains("NVIDIA"))
                })
                .and_then(|row| row.driver_version)
        })
        .and_then(|version| nvidia_release(&version).or(Some(version)));
    SlowFacts {
        stability: stability(&connection),
        bios,
        ram_mts,
        gpu_driver,
    }
}

/// The newest hourly Reliability Monitor index (1 to 10); absent when the reliability service has no data yet.
fn stability(connection: &WMIConnection) -> Option<f64> {
    let rows: Vec<HashMap<String, Variant>> = connection
        .raw_query(
            "SELECT SystemStabilityIndex, TimeGenerated FROM Win32_ReliabilityStabilityMetrics",
        )
        .map_err(|error| tracing::warn!("Win32_ReliabilityStabilityMetrics: {error}"))
        .ok()?;
    rows.into_iter()
        .filter_map(|row| {
            let index = match row.get("SystemStabilityIndex")? {
                Variant::R8(value) => *value,
                Variant::R4(value) => f64::from(*value),
                _ => return None,
            };
            let Variant::String(time) = row.get("TimeGenerated")? else {
                return None;
            };
            Some((time.clone(), index))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, index)| index)
}

/// The Windows display language decides the tray texts, as it decides the window's language on first start.
pub fn ui_language_is_russian() -> bool {
    const LANG_RUSSIAN: u16 = 0x19;
    // SAFETY: GetUserDefaultUILanguage takes no arguments and returns a LANGID.
    let language = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    language & 0x3ff == LANG_RUSSIAN
}

/// The exes of the processes the user can see; a process of another account or an elevated one is left out.
pub fn running_exes() -> Vec<PathBuf> {
    // SAFETY: a snapshot of the process list, closed below.
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let mut exes = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..PROCESSENTRY32W::default()
    };
    // SAFETY: `snapshot` is open and `entry` a live PROCESSENTRY32W with dwSize set.
    let mut more = unsafe { Process32FirstW(snapshot, &raw mut entry) }.is_ok();
    while more {
        if let Some(exe) = exe_of(entry.th32ProcessID) {
            exes.push(exe);
        }
        // SAFETY: as for Process32FirstW.
        more = unsafe { Process32NextW(snapshot, &raw mut entry) }.is_ok();
    }
    close(snapshot);
    exes
}

fn exe_of(process_id: u32) -> Option<PathBuf> {
    // SAFETY: OpenProcess takes plain values; the handle is closed below.
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let mut buffer = [0u16; 1024];
    let mut length = u32::try_from(buffer.len()).unwrap_or(0);
    // SAFETY: `process` is open and `buffer` holds `length` u16 for the call.
    let read = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut length,
        )
    };
    close(process);
    read.ok()?;
    let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}

fn close(handle: HANDLE) {
    // SAFETY: the handle came from a call that opened it and is closed once, here.
    if let Err(error) = unsafe { CloseHandle(handle) } {
        tracing::warn!("could not close a handle: {error}");
    }
}
