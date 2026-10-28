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
    pub const NETWORK_OVERVIEW: &str = "network_overview";
    pub const NETWORK_PROBE: &str = "network_probe";
    pub const SPEED_TEST_START: &str = "speed_test_start";
    pub const SPEED_TEST_STATUS: &str = "speed_test_status";
    pub const DEVICES_OVERVIEW: &str = "devices_overview";
    pub const DEVICES_SCAN: &str = "devices_scan";
    pub const STARTUP_OVERVIEW: &str = "startup_overview";
    pub const STARTUP_SET: &str = "startup_set";
    pub const SERVICES_OVERVIEW: &str = "services_overview";
    pub const SERVICE_SET: &str = "service_set";
    pub const ALERTS_STATUS: &str = "alerts_status";
    pub const ALERTS_SAVE_BOT: &str = "alerts_save_bot";
    pub const ALERTS_IMPORT_LEGACY: &str = "alerts_import_legacy";
    pub const ALERTS_FORGET_BOT: &str = "alerts_forget_bot";
    pub const ALERTS_TEST: &str = "alerts_test";
    pub const ALERTS_SET_SENDING: &str = "alerts_set_sending";
    pub const CLAUDE_STATUS: &str = "claude_status";
    pub const CLAUDE_CONNECT: &str = "claude_connect";
    pub const SENSORS_LIVE: &str = "sensors_live";
    pub const SENSORS_HISTORY: &str = "sensors_history";
    pub const AUTOSTART_STATUS: &str = "autostart_status";
    pub const AUTOSTART_SET: &str = "autostart_set";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdapterKind {
    Ethernet,
    Wifi,
    Other,
}

/// A network adapter that is up and has a gateway: the way out of this PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub description: String,
    pub kind: AdapterKind,
    /// The link speed the adapter negotiated, bits per second
    pub link_bps: Option<u64>,
    pub address: Option<String>,
    pub gateway: Option<String>,
}

/// A burst of pings to one host: the mean round trip, the share lost, the jitter between consecutive replies.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub avg_ms: Option<f64>,
    /// 0 to 100
    pub loss_pct: f64,
    pub jitter_ms: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetSample {
    /// Unix seconds
    pub time: i64,
    /// None when the PC had no gateway
    pub gateway: Option<Probe>,
    pub internet: Probe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkOverview {
    pub adapters: Vec<Adapter>,
    /// The samples of the last day, oldest first
    pub samples: Vec<NetSample>,
    /// The host pinged for the internet
    pub internet_host: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    /// Bytes per second; none when the transfer failed
    pub down: Option<u64>,
    pub up: Option<u64>,
    /// Unix seconds
    pub finished_at: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedStatus {
    pub is_running: bool,
    pub last: Option<SpeedResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceRole {
    /// This PC
    This,
    /// The router: the gateway of the way out
    Gateway,
    Other,
}

/// A device of the local network, as the scans saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// `dc:51:93:a5:61:44`
    pub mac: String,
    pub ip: String,
    pub name: Option<String>,
    /// The maker of its network card by the IEEE registry
    pub maker: Option<String>,
    /// A MAC the device made up (phones and tablets do per network): it has no maker
    pub is_random_mac: bool,
    pub role: DeviceRole,
    /// Unix seconds
    pub first_seen: i64,
    pub last_seen: i64,
    /// Seen by the last scan
    pub is_online: bool,
    /// First seen within the last week, after the first scan of the panel
    pub is_new: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicesOverview {
    /// The online first, then by address
    pub devices: Vec<Device>,
    /// Unix seconds of the last scan
    pub last_scan: Option<i64>,
    pub is_scanning: bool,
}

/// Where Windows keeps a program that starts at sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartupSource {
    /// `HKCU\…\Run`: this user
    UserRun,
    /// `HKLM\…\Run`: every user
    MachineRun,
    /// `HKLM\…\WOW6432Node\…\Run`: every user, a 32-bit program
    MachineRun32,
    /// The Startup folder of this user
    UserFolder,
    /// The Startup folder of every user
    MachineFolder,
}

impl StartupSource {
    pub const ALL: [Self; 5] = [
        Self::UserRun,
        Self::MachineRun,
        Self::MachineRun32,
        Self::UserFolder,
        Self::MachineFolder,
    ];

    /// Turning an entry of every user on or off needs admin rights
    #[must_use]
    pub const fn is_machine(self) -> bool {
        matches!(
            self,
            Self::MachineRun | Self::MachineRun32 | Self::MachineFolder
        )
    }
}

/// Task Manager's scale of what a program costs the sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
}

/// What a program and the processes it started cost the first two minutes after sign-in, on average.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupImpact {
    pub level: ImpactLevel,
    pub cpu_ms: u64,
    pub disk_bytes: u64,
    /// The sign-ins it was seen in
    pub boots: u32,
}

/// What the Authenticode signature of a file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignatureState {
    /// Signed, by the file itself or by a Windows catalog, and the chain is trusted
    Valid,
    /// A signature is there but does not hold: the file changed, or its certificate is not trusted
    Invalid,
    Unsigned,
    /// An alias of an app of the Microsoft Store: Windows checked its package when it was installed
    StoreApp,
    /// The file the entry starts does not exist
    NoFile,
}

/// A program Windows starts at sign-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntry {
    pub source: StartupSource,
    /// The value name of a Run key, or the file name in a Startup folder
    pub name: String,
    /// What Windows runs: the command of the Run value or the target and arguments of the shortcut
    pub command: String,
    pub image: Option<PathBuf>,
    pub is_enabled: bool,
    /// Unix seconds of when it was turned off, as Windows recorded it
    pub disabled_at: Option<i64>,
    pub signature: SignatureState,
    /// The subject of the signing certificate
    pub signer: Option<String>,
    /// The company of the file's version resource: what the file says of itself, not verified
    pub company: Option<String>,
    /// A part of Windows: shown, never turned off
    pub is_protected: bool,
    /// None when no recent sign-in saw it run
    pub impact: Option<StartupImpact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupOverview {
    pub entries: Vec<StartupEntry>,
    /// The sign-ins Windows kept a trace of
    pub measured_boots: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSetArgs {
    pub source: StartupSource,
    pub name: String,
    pub enable: bool,
}

/// How a change that may need admin rights ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeOutcome {
    Done,
    /// The user said no at the UAC prompt: nothing changed
    Declined,
}

/// When Windows starts a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartType {
    Automatic,
    /// Automatic, a couple of minutes after the sign-in
    AutomaticDelayed,
    /// When a program or a trigger asks for it
    Manual,
    Disabled,
}

impl StartType {
    /// Starts by itself with Windows
    #[must_use]
    pub const fn is_automatic(self) -> bool {
        matches!(self, Self::Automatic | Self::AutomaticDelayed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceState {
    Running,
    Stopped,
    /// Starting, stopping, pausing or paused
    Changing,
}

/// A Windows service, with who made it and what the panel may do with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    /// The key name: `Docker Desktop Service`, `wuauserv`
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub start: StartType,
    pub state: ServiceState,
    /// The exe, or the DLL of a service hosted by `svchost.exe`
    pub image: Option<PathBuf>,
    pub signature: SignatureState,
    pub signer: Option<String>,
    pub company: Option<String>,
    /// Microsoft's or a part of Windows: shown, never changed (the brief, §1 p.15)
    pub is_microsoft: bool,
    /// The start type before the panel changed it: "Restore" brings it back
    pub changed_from: Option<StartType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicesOverview {
    pub services: Vec<Service>,
}

/// Off is "start when asked" (`Manual`); restore is the start type the panel saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSetArgs {
    pub name: String,
    pub restore: bool,
}

/// One check of the alerts: what it found, and whether it went to Telegram.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRecord {
    /// Unix seconds
    pub time: i64,
    /// The lines of the message, as Telegram would show them
    pub lines: Vec<String>,
    pub was_sent: bool,
    /// Why it did not go: sending is off, no bot, Telegram refused
    pub reason: Option<AlertSkip>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlertSkip {
    /// Sending is off: the panel only writes what it would send
    SendingOff,
    NoBot,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertsStatus {
    /// A bot token and a chat are saved
    pub has_bot: bool,
    /// The last digits of the chat, to tell which one it is
    pub chat_hint: Option<String>,
    pub is_sending: bool,
    /// `~/.olegpc/telegram.env` of the old alerts is there to take the bot from
    pub has_legacy_bot: bool,
    pub disk_min_gb: u32,
    pub runner_grace_minutes: u32,
    /// Unix seconds of the last check
    pub last_check: Option<i64>,
    /// The checks that found something, newest first
    pub recent: Vec<AlertRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertsSaveBotArgs {
    pub token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertsSetSendingArgs {
    pub is_sending: bool,
}

/// How the test message went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestOutcome {
    Sent,
    NoBot,
    /// Telegram answered no: a wrong token or chat
    Refused,
    /// Telegram did not answer: no network, or curl is missing
    Unreachable,
}

/// Whether Claude Code is here and reads this panel's MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatus {
    /// `claude` was found on this PC
    pub has_cli: bool,
    /// `pc-health` is in the user's MCP servers of Claude Code
    pub is_connected: bool,
    /// What a status line script runs for the line of Vitals
    pub status_command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectOutcome {
    Done,
    NoCli,
    Failed,
}

/// Whether the panel starts at sign-in: its value under this user's Run key, and the mark Task Manager keeps for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    pub is_enabled: bool,
    /// The exe the value starts when it is not this one: an older copy, moved or installed elsewhere
    pub other_copy: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutostartSetArgs {
    pub enable: bool,
}

/// The widest history of the sensors a command accepts: the core keeps a month.
pub const MAX_SENSOR_HOURS: u32 = 30 * 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorsHistoryArgs {
    pub hours: u32,
}

/// The processor as Windows' performance counters see it, over the interval since the reading before.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuReading {
    pub name: Option<String>,
    /// 0 to 100, the utilization Task Manager shows
    pub load_pct: Option<f64>,
    /// The effective clock: the base one times the performance the counters measured
    pub mhz: Option<f64>,
    pub base_mhz: Option<f64>,
}

/// A graphics card as `nvidia-smi` reads it; a value the card does not report is absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuReading {
    pub name: String,
    pub temp_c: Option<f64>,
    pub load_pct: Option<f64>,
    pub power_w: Option<f64>,
    pub power_limit_w: Option<f64>,
    pub fan_pct: Option<f64>,
    pub core_mhz: Option<f64>,
    pub memory_mhz: Option<f64>,
    /// Mebibytes, as `nvidia-smi` counts them
    pub vram_used_mb: Option<f64>,
    pub vram_total_mb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorReading {
    /// Unix seconds
    pub time: i64,
    pub cpu: CpuReading,
    pub ram_used_bytes: Option<u64>,
    pub ram_total_bytes: Option<u64>,
    /// Empty without an NVIDIA card; the cards of other makers need `LibreHardwareMonitor`
    pub gpus: Vec<GpuReading>,
}

/// A point of a sensors chart: one live reading, or one five-minute sample of the history. The card is the first one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorPoint {
    /// Unix seconds
    pub time: i64,
    pub cpu_load_pct: Option<f64>,
    pub cpu_mhz: Option<f64>,
    pub ram_used_pct: Option<f64>,
    pub gpu_temp_c: Option<f64>,
    pub gpu_load_pct: Option<f64>,
    pub gpu_power_w: Option<f64>,
}

/// The live sensors: the latest reading and the points of the last ten minutes, oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorsLive {
    pub reading: Option<SensorReading>,
    pub points: Vec<SensorPoint>,
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
