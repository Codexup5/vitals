//! The volumes (C:, D:, F:) with their space, and the physical drives with their health.

#[cfg(windows)]
pub mod windows;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub letter: String,
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// What Windows reports for a physical drive: its own health verdict and the reliability counters it exposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalDisk {
    pub name: String,
    pub media: String,
    pub size_bytes: u64,
    pub health: DiskHealth,
    pub temperature_c: Option<u32>,
    /// Percent of the rated endurance used (`NVMe` "percentage used"); 0 for a new drive
    pub wear_percent: Option<u32>,
    pub power_on_hours: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiskHealth {
    Healthy,
    Warning,
    Unhealthy,
    Unknown,
}

/// `MSFT_PhysicalDisk.HealthStatus`: 0 healthy, 1 warning, 2 unhealthy, 5 unknown.
pub const fn health_from_code(code: u16) -> DiskHealth {
    match code {
        0 => DiskHealth::Healthy,
        1 => DiskHealth::Warning,
        2 => DiskHealth::Unhealthy,
        _ => DiskHealth::Unknown,
    }
}

/// `MSFT_PhysicalDisk.MediaType`: 3 HDD, 4 SSD, 5 SCM.
pub fn media_from_code(code: u16) -> String {
    match code {
        3 => "HDD",
        4 => "SSD",
        5 => "SCM",
        _ => "—",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_and_media_codes() {
        assert_eq!(health_from_code(0), DiskHealth::Healthy);
        assert_eq!(health_from_code(1), DiskHealth::Warning);
        assert_eq!(health_from_code(2), DiskHealth::Unhealthy);
        assert_eq!(health_from_code(5), DiskHealth::Unknown);
        assert_eq!(media_from_code(4), "SSD");
        assert_eq!(media_from_code(0), "—");
    }
}
