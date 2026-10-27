//! Volume space from Win32, drive health from the Storage WMI namespace (readable by a normal user).
#![allow(
    unsafe_code,
    reason = "Win32 calls of the volume list live in this file"
)]

use serde::Deserialize;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};
use windows::core::HSTRING;
use wmi::{COMLibrary, WMIConnection};

use super::{PhysicalDisk, Volume, health_from_code, media_from_code};

const DRIVE_FIXED: u32 = 3;
const STORAGE_NAMESPACE: &str = r"root\Microsoft\Windows\Storage";

pub fn volumes() -> Vec<Volume> {
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
            let (mut free, mut total) = (0u64, 0u64);
            // SAFETY: `root` is valid; `free` and `total` are live u64 values the call writes into.
            unsafe { GetDiskFreeSpaceExW(&root, Some(&raw mut free), Some(&raw mut total), None) }
                .ok()?;
            let mut label = [0u16; 261];
            // SAFETY: the label buffer is a live slice; the call writes at most its length.
            let label = match unsafe {
                GetVolumeInformationW(&root, Some(&mut label), None, None, None, None)
            } {
                Ok(()) => {
                    let end = label
                        .iter()
                        .position(|&unit| unit == 0)
                        .unwrap_or(label.len());
                    String::from_utf16_lossy(&label[..end])
                }
                Err(_) => String::new(),
            };
            Some(Volume {
                letter: letter.to_string(),
                label,
                total_bytes: total,
                free_bytes: free,
            })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename = "MSFT_PhysicalDisk", rename_all = "PascalCase")]
struct MsftPhysicalDisk {
    #[serde(rename = "__Path")]
    path: String,
    friendly_name: Option<String>,
    media_type: Option<u16>,
    health_status: Option<u16>,
    size: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename = "MSFT_PhysicalDiskToStorageReliabilityCounter")]
struct DiskToReliability {}

#[derive(Deserialize)]
#[serde(rename = "MSFT_StorageReliabilityCounter", rename_all = "PascalCase")]
struct MsftReliability {
    temperature: Option<u32>,
    wear: Option<u32>,
    power_on_hours: Option<u64>,
}

/// Every drive Windows sees, with its counters when the drive reports them; empty when WMI fails.
pub fn physical_disks() -> Vec<PhysicalDisk> {
    let connection = match COMLibrary::new()
        .and_then(|com| WMIConnection::with_namespace_path(STORAGE_NAMESPACE, com))
    {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!("storage WMI is not available: {error}");
            return Vec::new();
        }
    };
    let disks: Vec<MsftPhysicalDisk> = connection.query().unwrap_or_else(|error| {
        tracing::warn!("MSFT_PhysicalDisk: {error}");
        Vec::new()
    });
    disks
        .into_iter()
        .map(|disk| {
            // The counters are reachable only through the association: the class alone enumerates nothing
            let counters = connection
                .associators::<MsftReliability, DiskToReliability>(&disk.path)
                .unwrap_or_else(|error| {
                    tracing::warn!("reliability counters of {}: {error}", disk.path);
                    Vec::new()
                });
            let counter = counters.first();
            PhysicalDisk {
                name: disk.friendly_name.unwrap_or_default(),
                media: media_from_code(disk.media_type.unwrap_or(0)),
                size_bytes: disk.size.unwrap_or(0),
                health: health_from_code(disk.health_status.unwrap_or(5)),
                temperature_c: counter
                    .and_then(|counter| counter.temperature)
                    .filter(|value| *value > 0),
                wear_percent: counter.and_then(|counter| counter.wear),
                power_on_hours: counter.and_then(|counter| counter.power_on_hours),
            }
        })
        .collect()
}
