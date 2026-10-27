//! The volumes (C:, D:, F:) with their space, and the physical drives with their health.

#[cfg(windows)]
pub mod windows;

pub use vitals_ipc::{DiskHealth, PhysicalDisk, Volume};

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
