//! The tools of "pc-health": the same database and Windows calls the window reads, in a process of their own.

use std::path::PathBuf;

use chrono::{Days, Local};
use serde_json::{Value, json};
use vitals_ipc::{DayCounts, Group, ScanResult, UpdatesPhase};

use super::days_of;
use crate::storage::Storage;
use crate::storage::queries::EventFilter;
use crate::updates::job::Host;

/// The tools over the panel's database in `%APPDATA%\Vitals`.
#[derive(Debug)]
pub struct PanelTools {
    pub db_path: PathBuf,
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl PanelTools {
    fn open(&self) -> Result<Storage, String> {
        Storage::open(&self.db_path).map_err(text)
    }

    fn health_summary(&self, arguments: &Value) -> Result<Value, String> {
        let storage = self.open()?;
        let today = Local::now().date_naive();
        let days = days_of(arguments, 30);
        let from = today
            .checked_sub_days(Days::new(u64::from(days - 1)))
            .unwrap_or(today);
        let rows = storage.days_between(from, today).map_err(text)?;
        let mut total = DayCounts::default();
        for row in &rows {
            total.crashes += row.counts.crashes;
            total.bsod += row.counts.bsod;
            total.gpu_reset += row.counts.gpu_reset;
            total.nv_container += row.counts.nv_container;
            total.whea += row.counts.whea;
            total.iommu += row.counts.iommu;
            total.app_crashes += row.counts.app_crashes;
            total.errors += row.counts.errors;
        }
        let troubled = rows
            .iter()
            .filter(|row| {
                let counts = &row.counts;
                counts.crashes + counts.bsod + counts.whea + counts.gpu_reset + counts.app_crashes
                    > 0
            })
            .count();
        Ok(json!({
            "summary": crate::collector::summary_from(&storage, today).map_err(text)?,
            "from": from.format("%Y-%m-%d").to_string(),
            "to": today.format("%Y-%m-%d").to_string(),
            "totals": total,
            "daysWithTrouble": troubled,
            "days": rows,
        }))
    }

    fn events(&self, arguments: &Value) -> Result<Value, String> {
        let today = Local::now().date_naive();
        let days = days_of(arguments, 7);
        let filter = EventFilter {
            kinds: arguments
                .get("kind")
                .and_then(Value::as_str)
                .map(|kind| vec![kind.to_owned()])
                .unwrap_or_default(),
            from: today.checked_sub_days(Days::new(u64::from(days - 1))),
            to: Some(today),
        };
        let page = self
            .open()?
            .events_page(&filter, 0, vitals_ipc::EVENT_PAGE_SIZE)
            .map_err(text)?;
        serde_json::to_value(page).map_err(text)
    }

    fn latest_scan(&self) -> Result<Option<ScanResult>, String> {
        let body = self.open()?.latest_scan().map_err(text)?;
        Ok(body.and_then(|body| serde_json::from_str(&body).ok()))
    }

    fn disk_usage(&self) -> Result<Value, String> {
        let scan = self.latest_scan()?;
        Ok(json!({
            "volumes": crate::disks::windows::volumes(),
            "physicalDisks": crate::disks::windows::physical_disks(),
            "lastScan": scan.map(|scan| json!({
                "finishedAt": scan.finished_at,
                "home": scan.home,
                "totalBytes": scan.total_bytes,
                "files": scan.files,
                "linksNotFollowed": scan.links,
                "unreadable": scan.unreadable,
            })),
        }))
    }

    fn cleanup_preview(&self) -> Result<Value, String> {
        let Some(scan) = self.latest_scan()? else {
            return Ok(
                json!({ "scanned": false, "note": "No scan yet: run one on the Disks screen of Vitals." }),
            );
        };
        let sum = |group: Group| {
            scan.candidates
                .iter()
                .filter(|candidate| candidate.group == group)
                .map(|candidate| candidate.bytes)
                .sum::<u64>()
        };
        Ok(json!({
            "scanned": true,
            "scannedAt": scan.finished_at,
            "safeBytes": sum(Group::Safe),
            "reviewBytes": sum(Group::Review),
            "candidates": scan.candidates,
            "note": "Preview only. Deleting runs in the Vitals window, after its own preview, never through this server.",
        }))
    }

    fn updates_list() -> Result<Value, String> {
        let host = crate::updates::winget::WingetHost;
        match host.list() {
            Err(_) => Err("winget is not installed".to_owned()),
            Ok(None) => Err("winget did not answer".to_owned()),
            Ok(Some(mut packages)) => {
                for package in &mut packages {
                    package.is_open = host.is_open(package);
                }
                Ok(json!({ "phase": UpdatesPhase::Idle, "packages": packages }))
            }
        }
    }

    fn network_status(&self) -> Result<Value, String> {
        let since = chrono::Utc::now().timestamp() - 24 * 60 * 60;
        let samples = self.open()?.net_samples_since(since).map_err(text)?;
        Ok(json!({
            "adapters": crate::network::adapters(),
            "internetHost": crate::network::INTERNET_HOST.to_string(),
            "latest": samples.last(),
            "samplesToday": samples.len(),
            "samples": samples,
        }))
    }
}

impl super::Tools for PanelTools {
    fn call(&self, name: &str, arguments: &Value) -> Result<Value, String> {
        match name {
            "health_summary" => self.health_summary(arguments),
            "events" => self.events(arguments),
            "disk_usage" => self.disk_usage(),
            "cleanup_preview" => self.cleanup_preview(),
            "updates_list" => Self::updates_list(),
            "network_status" => self.network_status(),
            _ => Err(format!("no tool {name}")),
        }
    }
}

/// `vitals.exe --mcp`: serves stdin and stdout until the client closes them.
pub fn run(db_path: PathBuf) -> std::process::ExitCode {
    let tools = PanelTools { db_path };
    let stdin = std::io::stdin();
    match super::serve(stdin.lock(), std::io::stdout(), &tools) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::FAILURE,
    }
}
