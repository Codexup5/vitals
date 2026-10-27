//! A snapshot of the machine: free disk space, memory and uptime every run, the slow WMI facts once an hour.

#[cfg(windows)]
pub mod windows;

use crate::storage::{DiskFree, Snapshot};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// The facts that change rarely and cost a WMI round trip: the collector refreshes them once an hour.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlowFacts {
    pub stability: Option<f64>,
    pub bios: Option<String>,
    pub ram_mts: Option<u32>,
    pub gpu_driver: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FastFacts {
    /// Drive letter and free bytes available to the user
    pub disks: Vec<(char, u64)>,
    pub ram_free_bytes: Option<u64>,
    pub uptime_ms: u64,
}

pub fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[allow(
    clippy::cast_precision_loss,
    reason = "gigabytes to one decimal: bytes beyond 2^53 lose nothing visible"
)]
fn gib(bytes: u64) -> f64 {
    round1(bytes as f64 / GIB)
}

/// The NVIDIA driver as the user knows it: Windows reports `32.0.16.2101`, the release is its last five digits, 621.01.
pub fn nvidia_release(windows_version: &str) -> Option<String> {
    let digits: String = windows_version
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let tail = digits.get(digits.len().checked_sub(5)?..)?;
    Some(format!("{}.{}", &tail[..3], &tail[3..]))
}

pub fn to_snapshot(fast: &FastFacts, slow: &SlowFacts) -> Snapshot {
    Snapshot {
        disks: fast
            .disks
            .iter()
            .map(|(letter, free)| DiskFree {
                drive: letter.to_string(),
                free_gb: gib(*free),
            })
            .collect(),
        ram_free_gb: fast.ram_free_bytes.map(gib),
        #[allow(clippy::cast_precision_loss, reason = "hours to one decimal")]
        uptime_h: Some(round1(fast.uptime_ms as f64 / 3_600_000.0)),
        stability: slow.stability.map(|value| (value * 100.0).round() / 100.0),
        bios: slow.bios.clone(),
        ram_mts: slow.ram_mts,
        gpu_driver: slow.gpu_driver.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_release_from_the_windows_version() {
        assert_eq!(nvidia_release("32.0.16.2101").as_deref(), Some("621.01"));
        assert_eq!(nvidia_release("32.0.15.6094").as_deref(), Some("560.94"));
        assert_eq!(nvidia_release("1.2"), None);
    }

    #[test]
    fn snapshot_rounds_like_the_powershell_collector() {
        let fast = FastFacts {
            disks: vec![('C', 222_906_920_960), ('F', 813_686_964_224)],
            ram_free_bytes: Some(53_794_629_632),
            uptime_ms: 360_000,
        };
        let slow = SlowFacts {
            stability: Some(4.861_234),
            bios: Some("4.60".to_owned()),
            ram_mts: Some(4800),
            gpu_driver: None,
        };
        let snapshot = to_snapshot(&fast, &slow);
        assert_eq!(
            snapshot.disks[0],
            DiskFree {
                drive: "C".to_owned(),
                free_gb: 207.6
            }
        );
        assert_eq!(snapshot.disks[1].free_gb, 757.8);
        assert_eq!(snapshot.ram_free_gb, Some(50.1));
        assert_eq!(snapshot.uptime_h, Some(0.1));
        assert_eq!(snapshot.stability, Some(4.86));
    }
}
