//! Windows event log entries the panel keeps: what each one means and which ones are worth a row.

#[cfg(windows)]
pub mod windows;
pub mod xml;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Log {
    System,
    Application,
}

impl Log {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Application => "Application",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// Kernel-Power 41: the PC restarted without a clean shutdown
    Crash,
    /// WER-SystemErrorReporting 1001: a bugcheck (BSOD) with its code
    Bsod,
    /// nvlddmkm 13 or 153: the GPU driver reset
    GpuReset,
    /// Service Control Manager 7031 about an NVIDIA service that crashed
    NvContainer,
    Whea,
    /// Microsoft-Windows-HAL: the IOMMU and interrupt remapping warnings
    Iommu,
    /// Application Error 1000: an application crashed
    AppCrash,
}

impl EventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crash => "crash",
            Self::Bsod => "bsod",
            Self::GpuReset => "gpuReset",
            Self::NvContainer => "nvContainer",
            Self::Whea => "whea",
            Self::Iommu => "iommu",
            Self::AppCrash => "appCrash",
        }
    }
}

/// One entry as the event log gives it, before the panel decides what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEvent {
    pub log: Log,
    pub record_id: u64,
    pub time: DateTime<Utc>,
    pub provider: String,
    pub event_id: u32,
    pub level: u8,
    /// `EventData/Data` in document order: the name (absent in classic events) and the text
    pub data: Vec<(Option<String>, String)>,
}

impl RawEvent {
    fn data_named(&self, name: &str) -> Option<&str> {
        self.data
            .iter()
            .find(|(key, _)| key.as_deref() == Some(name))
            .map(|(_, value)| value.as_str())
    }

    fn first_data(&self) -> Option<&str> {
        self.data.first().map(|(_, value)| value.as_str())
    }
}

/// A row of the `events` table: a known kind, or a plain System error, or both (an NVIDIA 7031 is also an error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub log: Log,
    pub record_id: u64,
    pub time: DateTime<Utc>,
    /// The local calendar day, the unit of every daily count
    pub day: NaiveDate,
    pub provider: String,
    pub event_id: u32,
    pub level: u8,
    pub kind: Option<EventKind>,
    /// What the screen shows next to the kind: the bugcheck code, the crashed application, the NVIDIA service
    pub detail: Option<String>,
}

const KERNEL_POWER: &str = "Microsoft-Windows-Kernel-Power";
const WER_SYSTEM: &str = "Microsoft-Windows-WER-SystemErrorReporting";
const NVIDIA_DRIVER: &str = "nvlddmkm";
const WHEA_LOGGER: &str = "Microsoft-Windows-WHEA-Logger";
const HAL: &str = "Microsoft-Windows-HAL";
const SERVICE_CRASHED: u32 = 7031;
const APP_CRASHED: u32 = 1000;

/// Critical (1) and Error (2). Level 0 is "log always", which providers also use for informational entries.
pub const fn is_error_level(level: u8) -> bool {
    matches!(level, 1 | 2)
}

pub fn classify(event: &RawEvent) -> Option<(EventKind, Option<String>)> {
    let provider = event.provider.as_str();
    match (event.log, event.event_id) {
        (Log::System, 41) if provider == KERNEL_POWER => {
            let code = event.data_named("BugcheckCode").filter(|code| *code != "0");
            Some((EventKind::Crash, code.map(str::to_owned)))
        }
        (Log::System, 1001) if provider == WER_SYSTEM => {
            let code = event
                .data_named("param1")
                .and_then(|text| text.split_whitespace().next());
            Some((EventKind::Bsod, code.map(str::to_owned)))
        }
        (Log::System, 13 | 153) if provider == NVIDIA_DRIVER => Some((EventKind::GpuReset, None)),
        (Log::System, SERVICE_CRASHED) => {
            let service = event.data_named("param1").or_else(|| event.first_data())?;
            service
                .contains("NVIDIA")
                .then(|| (EventKind::NvContainer, Some(service.to_owned())))
        }
        (Log::System, _) if provider == WHEA_LOGGER => Some((EventKind::Whea, None)),
        (Log::System, _) if provider == HAL => Some((EventKind::Iommu, None)),
        (Log::Application, APP_CRASHED) => {
            let app = event.data_named("AppName").or_else(|| event.first_data());
            Some((EventKind::AppCrash, app.map(str::to_owned)))
        }
        _ => None,
    }
}

/// The row to keep, or `None` for an entry the panel does not store (a System warning, any other Application event).
pub fn to_stored<Tz: TimeZone>(event: RawEvent, zone: &Tz) -> Option<StoredEvent> {
    let classified = classify(&event);
    let is_kept = classified.is_some() || (event.log == Log::System && is_error_level(event.level));
    if !is_kept {
        return None;
    }
    let (kind, detail) = classified.map_or((None, None), |(kind, detail)| (Some(kind), detail));
    Some(StoredEvent {
        day: event.time.with_timezone(zone).date_naive(),
        log: event.log,
        record_id: event.record_id,
        time: event.time,
        provider: event.provider,
        event_id: event.event_id,
        level: event.level,
        kind,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    fn event(
        log: Log,
        provider: &str,
        event_id: u32,
        level: u8,
        data: &[(&str, &str)],
    ) -> RawEvent {
        RawEvent {
            log,
            record_id: 1,
            time: DateTime::parse_from_rfc3339("2026-10-27T21:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            provider: provider.to_owned(),
            event_id,
            level,
            data: data
                .iter()
                .map(|(name, value)| (Some((*name).to_owned()), (*value).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn kernel_power_41_is_a_crash_with_a_nonzero_bugcheck_only() {
        let clean = event(Log::System, KERNEL_POWER, 41, 1, &[("BugcheckCode", "0")]);
        let coded = event(Log::System, KERNEL_POWER, 41, 1, &[("BugcheckCode", "78")]);
        assert_eq!(classify(&clean), Some((EventKind::Crash, None)));
        assert_eq!(
            classify(&coded),
            Some((EventKind::Crash, Some("78".to_owned())))
        );
    }

    #[test]
    fn wer_1001_is_a_bsod_with_its_code() {
        let bsod = event(
            Log::System,
            WER_SYSTEM,
            1001,
            2,
            &[("param1", "0x0000004e (0x06, 0x6d93f2, 0xffffec7e, 0x0d9e)")],
        );
        assert_eq!(
            classify(&bsod),
            Some((EventKind::Bsod, Some("0x0000004e".to_owned())))
        );
    }

    #[test]
    fn service_crash_counts_only_for_nvidia() {
        let nvidia = event(
            Log::System,
            "Service Control Manager",
            7031,
            2,
            &[("param1", "NVIDIA LocalSystem Container")],
        );
        let other = event(
            Log::System,
            "Service Control Manager",
            7031,
            2,
            &[("param1", "Print Spooler")],
        );
        assert_eq!(
            classify(&nvidia),
            Some((
                EventKind::NvContainer,
                Some("NVIDIA LocalSystem Container".to_owned())
            ))
        );
        assert_eq!(classify(&other), None);
    }

    #[test]
    fn gpu_reset_whea_iommu_and_app_crash() {
        assert_eq!(
            classify(&event(Log::System, NVIDIA_DRIVER, 153, 2, &[])),
            Some((EventKind::GpuReset, None))
        );
        assert_eq!(
            classify(&event(Log::System, NVIDIA_DRIVER, 14, 2, &[])),
            None
        );
        assert_eq!(
            classify(&event(Log::System, WHEA_LOGGER, 17, 3, &[])),
            Some((EventKind::Whea, None))
        );
        assert_eq!(
            classify(&event(Log::System, HAL, 16, 3, &[])),
            Some((EventKind::Iommu, None))
        );
        assert_eq!(
            classify(&event(
                Log::Application,
                "Application Error",
                1000,
                2,
                &[("AppName", "Steam.exe")]
            )),
            Some((EventKind::AppCrash, Some("Steam.exe".to_owned())))
        );
    }

    #[test]
    fn keeps_known_kinds_and_system_errors_only() {
        let zone = FixedOffset::east_opt(3 * 3600).unwrap();
        let warning = event(Log::System, "Disk", 153, 3, &[]);
        let error = event(Log::System, "Disk", 7, 2, &[]);
        let app_info = event(Log::Application, "MsiInstaller", 1033, 4, &[]);
        assert_eq!(to_stored(warning, &zone), None);
        assert_eq!(to_stored(app_info, &zone), None);
        let stored = to_stored(error, &zone);
        assert_eq!(stored.as_ref().map(|row| row.kind), Some(None));
        // 21:30 UTC is 00:30 the next day at UTC+3: the day is local
        assert_eq!(
            stored.map(|row| row.day),
            NaiveDate::from_ymd_opt(2026, 10, 28)
        );
    }

    #[test]
    fn every_stored_kind_is_a_kind_of_the_feed() {
        let kinds = [
            EventKind::Crash,
            EventKind::Bsod,
            EventKind::GpuReset,
            EventKind::NvContainer,
            EventKind::Whea,
            EventKind::Iommu,
            EventKind::AppCrash,
        ];
        for kind in kinds {
            assert!(
                vitals_ipc::FeedKind::parse(kind.as_str()).is_some(),
                "{kind:?}"
            );
        }
        assert_eq!(
            crate::storage::queries::PLAIN_ERROR,
            vitals_ipc::FeedKind::Error.as_str()
        );
    }
}
