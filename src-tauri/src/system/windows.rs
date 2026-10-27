//! Disk space, memory and uptime from Win32; BIOS, memory speed, the NVIDIA driver and stability from WMI.
#![allow(
    unsafe_code,
    reason = "Win32 calls of the machine snapshot live in this file"
)]

use std::collections::HashMap;

use serde::Deserialize;
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};
use windows::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::core::HSTRING;
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

fn ram_free_bytes() -> Option<u64> {
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: `status` is a live MEMORYSTATUSEX with dwLength set, as the call requires.
    unsafe { GlobalMemoryStatusEx(&raw mut status) }.ok()?;
    Some(status.ullAvailPhys)
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
