//! One-off import of `~\.olegpc\health\health.json`, the history of the old PowerShell collector.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::storage::{DayCounts, DaySource, DiskFree, Snapshot, Storage, StorageError};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("could not read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("health.json is not the collector's format: {0}")]
    Format(#[from] serde_json::Error),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// One day as `health-collect.ps1` writes it; today's row also carries the machine snapshot.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthDay {
    #[serde(default)]
    crashes: u32,
    #[serde(default)]
    bsod: u32,
    #[serde(default)]
    gpu_reset: u32,
    #[serde(default)]
    nv_container: u32,
    #[serde(default)]
    whea: u32,
    #[serde(default)]
    iommu: u32,
    #[serde(default)]
    app_crashes: u32,
    #[serde(default)]
    errors: u32,
    #[serde(default, deserialize_with = "one_or_many")]
    top_apps: Vec<String>,
    free_c: Option<f64>,
    free_d: Option<f64>,
    free_f: Option<f64>,
    ram_free_gb: Option<f64>,
    uptime_h: Option<f64>,
    stability: Option<f64>,
    bios: Option<String>,
    ram_mts: Option<u32>,
    gpu_driver: Option<String>,
}

/// PowerShell's `ConvertTo-Json` writes a one-item array as a bare string.
fn one_or_many<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
        Nothing(()),
    }
    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(item) => vec![item],
        OneOrMany::Many(items) => items,
        OneOrMany::Nothing(()) => Vec::new(),
    })
}

impl HealthDay {
    fn counts(&self) -> DayCounts {
        DayCounts {
            crashes: self.crashes,
            bsod: self.bsod,
            gpu_reset: self.gpu_reset,
            nv_container: self.nv_container,
            whea: self.whea,
            iommu: self.iommu,
            app_crashes: self.app_crashes,
            errors: self.errors,
            top_apps: self.top_apps.clone(),
        }
    }

    fn snapshot(&self) -> Option<Snapshot> {
        let disks: Vec<DiskFree> = [("C", self.free_c), ("D", self.free_d), ("F", self.free_f)]
            .into_iter()
            .filter_map(|(drive, free)| {
                free.map(|free_gb| DiskFree {
                    drive: drive.to_owned(),
                    free_gb,
                })
            })
            .collect();
        let has_any = !disks.is_empty() || self.ram_free_gb.is_some() || self.bios.is_some();
        has_any.then(|| Snapshot {
            disks,
            ram_free_gb: self.ram_free_gb,
            uptime_h: self.uptime_h,
            stability: self.stability,
            bios: self.bios.clone(),
            ram_mts: self.ram_mts,
            gpu_driver: self.gpu_driver.as_deref().map(|version| {
                crate::system::nvidia_release(version).unwrap_or_else(|| version.to_owned())
            }),
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: usize,
}

/// Adds the days the database lacks; a day already there (counted or imported before) is left as it is.
pub fn import_text(storage: &Storage, text: &str) -> Result<ImportReport, ImportError> {
    let days: BTreeMap<String, HealthDay> =
        serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    let mut report = ImportReport::default();
    for (key, day) in &days {
        let Ok(date) = NaiveDate::parse_from_str(key, "%Y-%m-%d") else {
            report.skipped += 1;
            continue;
        };
        if storage.has_day(date)? {
            report.skipped += 1;
            continue;
        }
        storage.put_day(date, &day.counts(), DaySource::Import)?;
        if let Some(snapshot) = day.snapshot() {
            storage.put_snapshot_if_absent(date, &snapshot)?;
        }
        report.imported += 1;
    }
    Ok(report)
}

pub fn import_file(storage: &Storage, path: &Path) -> Result<ImportReport, ImportError> {
    let text = std::fs::read_to_string(path).map_err(|source| ImportError::Read {
        path: path.display().to_string(),
        source,
    })?;
    import_text(storage, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}{
      \"2026-07-13\": {\"date\":\"2026-07-13\",\"crashes\":0,\"bsod\":0,\"gpuReset\":0,\"nvContainer\":0,\"whea\":0,\"iommu\":0,\"appCrashes\":0,\"topApps\":[],\"errors\":0},
      \"2026-10-13\": {\"date\":\"2026-10-13\",\"crashes\":1,\"bsod\":1,\"gpuReset\":0,\"nvContainer\":1170,\"whea\":0,\"iommu\":5,\"appCrashes\":1,\"topApps\":\"mkdir.exe ×1\",\"errors\":2100},
      \"2026-10-27\": {\"date\":\"2026-10-27\",\"crashes\":0,\"bsod\":0,\"gpuReset\":0,\"nvContainer\":940,\"whea\":0,\"iommu\":5,\"appCrashes\":0,\"topApps\":[],\"errors\":1904,
                      \"freeC\":207.6,\"freeD\":193,\"freeF\":757.8,\"ramFreeGB\":50.1,\"uptimeH\":0.1,\"stability\":4.86,\"bios\":\"4.60\",\"ramMTs\":4800,\"gpuDriver\":\"32.0.16.2101\"}
    }";

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn imports_every_day_with_its_counts_and_snapshot() {
        let storage = Storage::in_memory().unwrap();
        assert_eq!(
            import_text(&storage, SAMPLE).unwrap(),
            ImportReport {
                imported: 3,
                skipped: 0
            }
        );
        let bsod_day = storage.day(day("2026-10-13")).unwrap().unwrap();
        assert_eq!(bsod_day.bsod, 1);
        assert_eq!(bsod_day.top_apps, vec!["mkdir.exe ×1".to_owned()]);
        let snapshot = storage.latest_snapshot().unwrap().unwrap();
        assert_eq!(snapshot.disks.len(), 3);
        assert_eq!(snapshot.bios.as_deref(), Some("4.60"));
        // Imported days are not the collector's: the first run still counts from the event log
        assert_eq!(storage.last_collected_day().unwrap(), None);
    }

    #[test]
    fn a_second_import_changes_nothing() {
        let storage = Storage::in_memory().unwrap();
        import_text(&storage, SAMPLE).unwrap();
        assert_eq!(
            import_text(&storage, SAMPLE).unwrap(),
            ImportReport {
                imported: 0,
                skipped: 3
            }
        );
    }

    #[test]
    fn a_counted_day_is_not_overwritten() {
        let storage = Storage::in_memory().unwrap();
        let counted = DayCounts {
            crashes: 1,
            ..DayCounts::default()
        };
        storage
            .put_day(day("2026-10-27"), &counted, DaySource::Collector)
            .unwrap();
        import_text(&storage, SAMPLE).unwrap();
        assert_eq!(storage.day(day("2026-10-27")).unwrap(), Some(counted));
    }

    #[test]
    fn rejects_another_format() {
        let storage = Storage::in_memory().unwrap();
        assert!(matches!(
            import_text(&storage, "[1, 2]"),
            Err(ImportError::Format(_))
        ));
    }
}
