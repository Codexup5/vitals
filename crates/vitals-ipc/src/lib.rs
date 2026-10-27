//! The contract between the Vitals core and its window: the command names, their arguments and what they return.
//!
//! Both sides compile these types, so a field renamed on one side is a compile error on the other, not a broken
//! screen. The wire format is JSON with camelCase keys, as Tauri's `invoke` carries it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The names the core registers in `invoke_handler` and the window calls.
pub mod command {
    pub const HEALTH_OVERVIEW: &str = "health_overview";
    pub const HEALTH_EVENTS: &str = "health_events";
    pub const MINIDUMPS: &str = "minidumps";
    pub const REVEAL_MINIDUMP: &str = "reveal_minidump";
    pub const DISKS_OVERVIEW: &str = "disks_overview";
    pub const SCAN_START: &str = "scan_start";
    pub const SCAN_STATUS: &str = "scan_status";
    pub const REVEAL_PATH: &str = "reveal_path";
    pub const CLEANUP_PREVIEW: &str = "cleanup_preview";
    pub const CLEANUP_RUN: &str = "cleanup_run";
    pub const UPDATES_STATUS: &str = "updates_status";
    pub const UPDATES_CHECK: &str = "updates_check";
    pub const UPDATES_RUN: &str = "updates_run";
    pub const UPDATES_TOOLS: &str = "updates_tools";
    pub const OPEN_UPDATE_TOOL: &str = "open_update_tool";
    pub const RUNNERS_PRESENT: &str = "runners_present";
    pub const RUNNERS_OVERVIEW: &str = "runners_overview";
    pub const MODE_START: &str = "mode_start";
    pub const MODE_STATUS: &str = "mode_status";
    pub const LOG_UI_ERROR: &str = "log_ui_error";
}

/// The size of a page of the event feed.
pub const EVENT_PAGE_SIZE: u32 = 50;

/// The widest chart range a command accepts.
pub const MAX_RANGE_DAYS: u32 = 366;

/// What went wrong inside a command, by the kind the window shows a message for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandErrorKind {
    Storage,
    Invalid,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandErrorBody {
    pub kind: CommandErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthOverviewArgs {
    pub days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthEventsArgs {
    /// Empty means every kind
    pub kinds: Vec<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// 0-based
    pub page: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevealMinidumpArgs {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogUiErrorArgs {
    pub message: String,
    pub context: String,
}

/// The counters of one day, or of one month in the monthly series.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCounts {
    pub crashes: u32,
    pub bsod: u32,
    pub gpu_reset: u32,
    pub nv_container: u32,
    pub whea: u32,
    pub iommu: u32,
    pub app_crashes: u32,
    pub errors: u32,
    /// "Steam.exe ×2", most frequent first, at most three
    pub top_apps: Vec<String>,
}

/// A calendar day `2026-10-27`, or a month `2026-10`, with its counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayRow {
    pub day: String,
    #[serde(flatten)]
    pub counts: DayCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSummary {
    /// Whole days since the last day with a crash or a BSOD; since the first recorded day when there was none
    pub days_without_crash: Option<i64>,
    pub system_drive_free_gb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthOverview {
    pub summary: HealthSummary,
    pub first_day: Option<String>,
    pub days: Vec<DayRow>,
    pub months: Vec<DayRow>,
}

/// The kinds of the event feed and its filter, in filter order: the kinds the core recognizes, then `error`, a System
/// error of no known kind. On the wire it is the `kind` string of an [`EventRow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedKind {
    Crash,
    Bsod,
    Whea,
    GpuReset,
    AppCrash,
    NvContainer,
    Iommu,
    Error,
}

impl FeedKind {
    pub const ALL: [Self; 8] = [
        Self::Crash,
        Self::Bsod,
        Self::Whea,
        Self::GpuReset,
        Self::AppCrash,
        Self::NvContainer,
        Self::Iommu,
        Self::Error,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crash => "crash",
            Self::Bsod => "bsod",
            Self::Whea => "whea",
            Self::GpuReset => "gpuReset",
            Self::AppCrash => "appCrash",
            Self::NvContainer => "nvContainer",
            Self::Iommu => "iommu",
            Self::Error => "error",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    /// Unix seconds
    pub time: i64,
    pub log: String,
    pub provider: String,
    pub event_id: u32,
    pub level: u8,
    /// A known kind, or `error`
    pub kind: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    pub items: Vec<EventRow>,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dump {
    pub name: String,
    pub size: u64,
    /// Unix seconds
    pub time: i64,
    /// The bugcheck code of the matching BSOD event, absent when the log no longer holds it
    pub code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevealPathArgs {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub letter: String,
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiskHealth {
    Healthy,
    Warning,
    Unhealthy,
    Unknown,
}

/// What Windows reports for a physical drive: its own health verdict and the reliability counters it exposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// The treemap categories of the brief, §4, in legend order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    DocumentsMedia,
    DevTools,
    CodeProjects,
    Sdks,
    AiModels,
    PackageCaches,
    System,
    Other,
}

impl Category {
    pub const ALL: [Self; 8] = [
        Self::DocumentsMedia,
        Self::DevTools,
        Self::CodeProjects,
        Self::Sdks,
        Self::AiModels,
        Self::PackageCaches,
        Self::System,
        Self::Other,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Safe,
    Review,
}

/// The cleanup candidates of the brief, §3: the whitelist first, then the ones to review one by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateKind {
    /// `%TEMP%` files older than seven days, `%TEMP%\claude` excluded
    UserTemp,
    /// `C:\Windows\Temp` older than seven days: an admin action
    WindowsTemp,
    PnpmStore,
    NpmCache,
    DockerDangling,
    DockerBuildCache,
    NextCache,
    Turbo,
    ShaderCache,
    StaleNodeModules,
    DockerUnusedImages,
    Vhdx,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub group: Group,
    pub kind: CandidateKind,
    pub path: Option<PathBuf>,
    pub bytes: u64,
    /// The size is what the folder holds; the command frees only part of it (pnpm prune, npm verify, hard links)
    pub is_upper_bound: bool,
    /// Unix seconds
    pub last_modified: Option<i64>,
    pub needs_admin: bool,
}

impl Candidate {
    #[must_use]
    pub fn new(group: Group, kind: CandidateKind, path: Option<PathBuf>, bytes: u64) -> Self {
        let id = match &path {
            Some(path) => format!("{kind:?}:{}", path.display()),
            None => format!("{kind:?}"),
        };
        Self {
            id,
            group,
            kind,
            path,
            bytes,
            is_upper_bound: false,
            last_modified: None,
            needs_admin: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub path: PathBuf,
    /// The path from the home, `AppData\Local\pnpm`
    pub label: String,
    pub bytes: u64,
    pub category: Category,
    /// Several small folders of one category shown as one tile
    pub is_rest: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// Unix seconds
    pub finished_at: i64,
    pub home: PathBuf,
    pub total_bytes: u64,
    pub files: u64,
    pub links: u64,
    pub unreadable: u64,
    pub tiles: Vec<Tile>,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisksOverview {
    pub volumes: Vec<Volume>,
    pub physical_disks: Vec<PhysicalDisk>,
    pub scan: Option<ScanResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    pub is_running: bool,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPreviewArgs {
    /// The candidate ids of the latest scan the user checked
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRunArgs {
    pub plan_id: u64,
}

/// How a candidate gets cleaned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupHow {
    /// The files listed by the preview are deleted
    Files,
    /// The tool that owns the cache cleans it (`pnpm store prune`, `docker image prune`)
    Command,
    /// The panel does not do it: compacting a disk image needs Docker and WSL stopped
    Manual,
    /// The candidate is not where its kind may be cleaned, or in a place the panel never touches
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub candidate_id: String,
    pub kind: CandidateKind,
    pub how: CleanupHow,
    /// The files the run will delete; 0 for a command
    pub files: u64,
    /// What the files hold, or the size the scan saw for a command
    pub bytes: u64,
    pub needs_admin: bool,
}

/// The snapshot a run executes: the same list, whatever appears or changes after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    pub plan_id: u64,
    pub items: Vec<PreviewItem>,
}

/// What became of the files of a cleanup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupTally {
    pub deleted: u64,
    /// Open in a program, or locked by Windows
    pub busy: u64,
    /// Written, replaced or gone since the preview
    pub changed: u64,
    /// Outside the root, or in a place the panel never touches
    pub refused: u64,
    pub freed: u64,
}

impl CleanupTally {
    pub fn add(&mut self, other: Self) {
        self.deleted += other.deleted;
        self.busy += other.busy;
        self.changed += other.changed;
        self.refused += other.refused;
        self.freed += other.freed;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupOutcome {
    Done,
    /// Windows asked for admin rights and the user said no: nothing ran
    Declined,
    /// The tool failed or is not installed
    Failed,
    /// Manual or refused: nothing to run
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItemReport {
    pub candidate_id: String,
    pub kind: CandidateKind,
    pub outcome: CleanupOutcome,
    pub tally: CleanupTally,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    /// Unix seconds
    pub finished_at: i64,
    pub items: Vec<CleanupItemReport>,
    pub total: CleanupTally,
}

/// A program winget can update, as `winget upgrade` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: String,
    pub name: String,
    pub installed: String,
    pub available: String,
    pub source: String,
    /// One of its programs runs now: the installer would fail or close it
    pub is_open: bool,
    /// The id came out whole: winget can be told exactly this package
    pub can_upgrade: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpgradeOutcome {
    /// Installed; the version after is read back from winget
    Done,
    /// The program was open: nothing ran, close it and update again
    Open,
    /// winget reports the update finishes after a restart, which the panel never does itself
    NeedsRestart,
    /// The downloaded installer is not the one winget's manifest names: the maker replaced it, winget refuses
    HashMismatch,
    /// The installer asked for admin rights or a choice and the user said no
    Declined,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeResult {
    pub id: String,
    pub name: String,
    pub before: String,
    /// The installed version read back after the run; none when winget no longer lists it
    pub after: Option<String>,
    pub outcome: UpgradeOutcome,
    /// winget's exit code when it failed
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdatesPhase {
    Idle,
    Checking,
    Upgrading,
}

/// The background job of the updates screen: a check or a run of upgrades, and what the last one found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesStatus {
    pub phase: UpdatesPhase,
    /// Unix seconds of the last finished check; none before the first
    pub checked_at: Option<i64>,
    /// The last check could not run winget or read its answer
    pub check_failed: bool,
    /// winget is not installed
    pub no_winget: bool,
    pub packages: Vec<Package>,
    /// The id being upgraded now
    pub current: Option<String>,
    /// The results of the last run, in the order it went
    pub results: Vec<UpgradeResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatesRunArgs {
    pub ids: Vec<String>,
}

/// A native tool the panel opens rather than doing the update itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateTool {
    WindowsUpdate,
    /// The GPU vendor's app (NVIDIA App, AMD Software)
    GpuDriver,
    Store,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenUpdateToolArgs {
    pub tool: UpdateTool,
}

/// The firmware of the board: shown for a manual update from the board maker's site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Firmware {
    pub board: Option<String>,
    pub version: Option<String>,
    /// `YYYY-MM-DD`
    pub released: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTools {
    /// The GPU vendor's app was found
    pub has_gpu_tool: bool,
    pub firmware: Firmware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunnerStatus {
    Online,
    Offline,
}

/// A self-hosted runner: what GitHub says of it, and what this PC says when its folder is here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Runner {
    pub name: String,
    /// `owner/name`
    pub repo: String,
    /// GitHub's view; none when `gh` could not ask
    pub status: Option<RunnerStatus>,
    pub is_busy: bool,
    pub labels: Vec<String>,
    /// The folder of a runner installed on this PC; none for a runner in a container or on another machine
    pub folder: Option<PathBuf>,
    /// Its `Runner.Listener` runs now; none when the runner is not in a folder of this PC
    pub is_listening: Option<bool>,
}

/// How the panel could ask GitHub about the runners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GithubAccess {
    Ok,
    /// `gh` is not installed or not signed in
    NoGh,
    /// `gh` answered with an error for at least one repository
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DockerState {
    Missing,
    Stopped,
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub name: String,
    pub image: String,
    /// `running`, `exited`, `restarting`…
    pub state: String,
    /// Docker's own words: `Up 11 hours (healthy)`
    pub status: String,
    /// The compose project the container belongs to
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnersOverview {
    pub runners: Vec<Runner>,
    pub github: GithubAccess,
    pub docker: DockerState,
    pub containers: Vec<Container>,
}

/// The two modes of the PC: games free memory and CPU from the dev stack, coding brings it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Docker Desktop and WSL stop: their containers and the runners in them go offline
    Gaming,
    /// Docker Desktop starts, and the containers that restart by themselves come up
    Coding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeStartArgs {
    pub mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeResult {
    pub mode: Mode,
    /// Every step ended well
    pub is_done: bool,
    /// The containers running after the switch
    pub running: u32,
    /// Unix seconds
    pub finished_at: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeStatus {
    /// The mode being switched to now
    pub switching: Option<Mode>,
    pub last: Option<ModeResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_row_flattens_its_counters_in_camel_case() {
        let row = DayRow {
            day: "2026-10-27".to_owned(),
            counts: DayCounts {
                gpu_reset: 2,
                ..DayCounts::default()
            },
        };
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(json["day"], "2026-10-27");
        assert_eq!(json["gpuReset"], 2);
        assert_eq!(serde_json::from_value::<DayRow>(json).unwrap(), row);
    }

    #[test]
    fn a_feed_kind_reads_back() {
        for kind in FeedKind::ALL {
            assert_eq!(FeedKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(FeedKind::parse("reboot"), None);
    }

    #[test]
    fn a_command_error_reads_as_kind_and_message() {
        let body: CommandErrorBody =
            serde_json::from_str(r#"{"kind":"storage","message":"locked"}"#).unwrap();
        assert_eq!(body.kind, CommandErrorKind::Storage);
    }
}
