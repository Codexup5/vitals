//! The programs Windows starts at sign-in, from the Run keys and the Startup folders, as Task Manager lists them.
//! Turning one off is what Task Manager does: a value under `StartupApproved` marks it off, the program and its Run
//! value stay, and turning it on again is one click (the brief, §1 p.15). An entry of every user needs admin rights,
//! so it changes through `vitals.exe --elevated`; a part of Windows is shown and never turned off.

pub mod elevated;
mod impact;
mod shortcut;
#[cfg(windows)]
mod windows;

use std::path::{Path, PathBuf};

use vitals_ipc::{ChangeOutcome, StartupEntry, StartupSetArgs, StartupSource};

use crate::trust::{self, Verdict};
use impact::Trace;

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("no startup entry {0:?} in its place")]
    NotFound(String),
    #[error("{0:?} is a part of Windows: the panel does not turn it off")]
    Protected(String),
    #[error("the registry refused the change: {0}")]
    Registry(String),
    #[error("an entry of every user changes only with admin rights")]
    NotMachine,
    #[error("{0}")]
    Elevated(String),
}

/// A program as its place lists it, before anything is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub source: StartupSource,
    pub name: String,
    pub command: String,
    /// Known up front for a shortcut: its target
    pub image: Option<PathBuf>,
}

/// Seconds between 1601-01-01, where a FILETIME starts, and 1970-01-01
const FILETIME_UNIX_SECONDS: i64 = 11_644_473_600;
const FILETIME_TICKS: i64 = 10_000_000;

/// The subkey of `StartupApproved` that holds the on and off marks of a source.
pub const fn approved_key(source: StartupSource) -> &'static str {
    match source {
        StartupSource::UserRun | StartupSource::MachineRun => "Run",
        StartupSource::MachineRun32 => "Run32",
        StartupSource::UserFolder | StartupSource::MachineFolder => "StartupFolder",
    }
}

/// On or off by the mark Windows keeps, and when it went off. No mark is on; an odd first byte is off.
pub fn approved_state(mark: Option<&[u8]>) -> (bool, Option<i64>) {
    let Some(mark) = mark else {
        return (true, None);
    };
    if mark.first().is_none_or(|byte| byte & 1 == 0) {
        return (true, None);
    }
    let at = mark
        .get(4..12)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(i64::from_le_bytes)
        .filter(|ticks| *ticks > 0)
        .map(|ticks| ticks / FILETIME_TICKS - FILETIME_UNIX_SECONDS);
    (false, at)
}

/// The mark Task Manager writes: `02` and zeros for on, `03` and the time it went off for off.
pub fn approved_mark(enable: bool, now: i64) -> [u8; 12] {
    let mut mark = [0u8; 12];
    if enable {
        mark[0] = 2;
    } else {
        mark[0] = 3;
        let ticks = (now + FILETIME_UNIX_SECONDS) * FILETIME_TICKS;
        mark[4..].copy_from_slice(&ticks.to_le_bytes());
    }
    mark
}

/// `%NAME%` replaced by its value; a name without one stays as it is.
pub fn expand_env(text: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(length) = after.find('%') else {
            break;
        };
        if let Some(value) = lookup(&after[..length]).filter(|_| length > 0) {
            out.push_str(&rest[..start]);
            out.push_str(&value);
            rest = &after[length + 1..];
        } else {
            out.push_str(&rest[..=start]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// The program a command line starts, read as `CreateProcess` reads it: a quoted path, else the shortest prefix
/// ending at a space that names a file. `resolve` answers the existing file a candidate names.
pub fn image_of(command: &str, resolve: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    if let Some(rest) = command.strip_prefix('"') {
        let path = rest.split('"').next().unwrap_or(rest);
        return Some(resolve(path).unwrap_or_else(|| PathBuf::from(path)));
    }
    let mut ends: Vec<usize> = command.match_indices(' ').map(|(at, _)| at).collect();
    ends.push(command.len());
    for end in &ends {
        let candidate = &command[..*end];
        if let Some(path) = resolve(candidate).or_else(|| resolve(&format!("{candidate}.exe"))) {
            return Some(path);
        }
    }
    // Nothing of it exists: up to the first `.exe`, else the first word
    let end = command
        .to_ascii_lowercase()
        .find(".exe")
        .map_or(ends[0], |at| at + 4);
    Some(PathBuf::from(&command[..end]))
}

/// `%SystemRoot%`, the Windows folder.
pub fn windows_dir() -> PathBuf {
    std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
}

/// The file a candidate of a command line names: an absolute path as it is, a bare name in the Windows folders.
pub fn resolve_candidate(candidate: &str, windows_dir: &Path) -> Option<PathBuf> {
    let path = Path::new(candidate);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_path_buf());
    }
    [windows_dir.join("System32"), windows_dir.to_path_buf()]
        .into_iter()
        .map(|dir| dir.join(path))
        .find(|path| path.is_file())
}

pub fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// The Startup folder of a folder source.
pub fn folder_of(source: StartupSource) -> Option<PathBuf> {
    let (base, tail) = match source {
        StartupSource::UserFolder => ("APPDATA", "Startup"),
        StartupSource::MachineFolder => ("ProgramData", "StartUp"),
        _ => return None,
    };
    let base = std::env::var_os(base)?;
    Some(
        Path::new(&base)
            .join(r"Microsoft\Windows\Start Menu\Programs")
            .join(tail),
    )
}

/// The files of a Startup folder; a shortcut is read for its target and arguments.
pub fn listed_in_folder(source: StartupSource, folder: &Path) -> Vec<Listed> {
    let Ok(read) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut listed: Vec<Listed> = read
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case("desktop.ini") {
                return None;
            }
            let path = entry.path();
            let is_shortcut = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"));
            let link = is_shortcut
                .then(|| std::fs::read(&path).ok())
                .flatten()
                .and_then(|bytes| shortcut::parse(&bytes));
            Some(match link {
                Some(link) => {
                    let target = expand_env(&link.target, env_lookup);
                    let command = match &link.arguments {
                        Some(arguments) => format!("\"{target}\" {arguments}"),
                        None => format!("\"{target}\""),
                    };
                    Listed {
                        source,
                        name,
                        command,
                        image: Some(PathBuf::from(target)),
                    }
                }
                None => Listed {
                    source,
                    name,
                    command: path.display().to_string(),
                    image: (!is_shortcut).then(|| path.clone()),
                },
            })
        })
        .collect();
    listed.sort_by_key(|item| item.name.to_lowercase());
    listed
}

/// What the panel knows of one program, put together.
pub struct Facts<'a> {
    pub approved: Option<&'a [u8]>,
    pub image: Option<PathBuf>,
    pub verdict: Verdict,
    pub company: Option<String>,
}

pub fn entry_of(
    listed: Listed,
    facts: Facts<'_>,
    windows_dir: &Path,
    traces: &[Trace],
) -> StartupEntry {
    let (is_enabled, disabled_at) = approved_state(facts.approved);
    let is_protected = facts
        .image
        .as_deref()
        .is_some_and(|image| trust::is_part_of_windows(image, windows_dir, &facts.verdict));
    // A program that is off did not start by itself: a trace of it is the user opening it
    let impact = facts
        .image
        .as_deref()
        .filter(|_| is_enabled)
        .and_then(|image| impact::impact_of(traces, image));
    StartupEntry {
        source: listed.source,
        name: listed.name,
        command: listed.command,
        image: facts.image,
        is_enabled,
        disabled_at,
        signature: facts.verdict.state,
        signer: facts.verdict.signer,
        company: facts.company,
        is_protected,
        impact,
    }
}

/// The ones that start first, the costly first among them, then by name.
pub fn sort_entries(entries: &mut [StartupEntry]) {
    entries.sort_by_key(|entry| {
        (
            !entry.is_enabled,
            std::cmp::Reverse(entry.impact.map(|impact| impact.level)),
            entry.name.to_lowercase(),
        )
    });
}

#[cfg(windows)]
pub use windows::overview;

/// Checks an entry again and writes its mark: the same steps in the panel and in the elevated process.
#[cfg(windows)]
pub fn apply(change: &StartupSetArgs, now: i64) -> Result<(), StartupError> {
    windows::apply(change, now)
}

#[cfg(not(windows))]
pub fn apply(_: &StartupSetArgs, _: i64) -> Result<(), StartupError> {
    Err(StartupError::Registry("only on Windows".to_owned()))
}

/// Turns an entry on or off; one of every user goes through UAC, where a "No" changes nothing.
pub fn set_enabled(
    change: &StartupSetArgs,
    work_dir: &Path,
) -> Result<ChangeOutcome, StartupError> {
    if !change.source.is_machine() {
        apply(change, chrono::Utc::now().timestamp())?;
        return Ok(ChangeOutcome::Done);
    }
    match crate::elevated::run::<_, Result<(), String>>(elevated::ACTION, change, work_dir) {
        Ok(Ok(())) => Ok(ChangeOutcome::Done),
        Ok(Err(message)) => Err(StartupError::Elevated(message)),
        Err(crate::elevated::ElevatedError::Declined) => Ok(ChangeOutcome::Declined),
        Err(error) => Err(StartupError::Elevated(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use vitals_ipc::{ImpactLevel, SignatureState, StartupImpact};

    use super::*;

    #[test]
    fn the_marks_task_manager_writes_read_back() {
        assert_eq!(approved_state(None), (true, None));
        assert_eq!(approved_state(Some(&[2, 0, 0, 0])), (true, None));
        assert_eq!(approved_state(Some(&[6; 12])), (true, None));
        let off = approved_mark(false, 1_761_600_000);
        assert_eq!(off[0], 3);
        assert_eq!(approved_state(Some(&off)), (false, Some(1_761_600_000)));
        assert_eq!(approved_state(Some(&approved_mark(true, 0))), (true, None));
        // An off mark of an entry turned off before Windows kept the time
        assert_eq!(
            approved_state(Some(&[3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])),
            (false, None)
        );
    }

    #[test]
    fn a_real_off_mark_gives_its_date() {
        // OneDrive on OLEGPC: 03000000AE3ED1930D53DC01
        let mark = [
            0x03, 0, 0, 0, 0xAE, 0x3E, 0xD1, 0x93, 0x0D, 0x53, 0xDC, 0x01,
        ];
        let (is_on, at) = approved_state(Some(&mark));
        assert!(!is_on);
        let year = chrono::DateTime::from_timestamp(at.unwrap(), 0).unwrap();
        assert_eq!(year.format("%Y").to_string(), "2025");
    }

    #[test]
    fn variables_expand_and_unknown_ones_stay() {
        let lookup = |name: &str| (name == "windir").then(|| r"C:\WINDOWS".to_owned());
        assert_eq!(
            expand_env(r"%windir%\system32\SecurityHealthSystray.exe", lookup),
            r"C:\WINDOWS\system32\SecurityHealthSystray.exe"
        );
        assert_eq!(expand_env("50% %nope% 100%", lookup), "50% %nope% 100%");
    }

    fn resolver(existing: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
        move |candidate| {
            existing
                .iter()
                .find(|path| path.eq_ignore_ascii_case(candidate))
                .map(PathBuf::from)
        }
    }

    #[test]
    fn a_command_line_gives_its_program() {
        let resolve = resolver(&[
            r"C:\Program Files (x86)\Overwolf\OverwolfLauncher.exe",
            r"C:\Program Files\Everything\Everything.exe",
            r"C:\WINDOWS\system32\rundll32.exe",
        ]);
        assert_eq!(
            image_of(
                r#""C:\Program Files\Everything\Everything.exe" -startup"#,
                &resolve
            ),
            Some(PathBuf::from(r"C:\Program Files\Everything\Everything.exe"))
        );
        assert_eq!(
            image_of(
                r"C:\Program Files (x86)\Overwolf\OverwolfLauncher.exe -overwolfsilent",
                &resolve
            ),
            Some(PathBuf::from(
                r"C:\Program Files (x86)\Overwolf\OverwolfLauncher.exe"
            ))
        );
        assert_eq!(
            image_of(
                r"C:\WINDOWS\system32\rundll32 shell32.dll,Control",
                &resolve
            ),
            Some(PathBuf::from(r"C:\WINDOWS\system32\rundll32.exe"))
        );
        assert_eq!(
            image_of(r"C:\Gone Away\tool.exe --tray", &resolve),
            Some(PathBuf::from(r"C:\Gone Away\tool.exe"))
        );
        assert_eq!(image_of("   ", &resolve), None);
    }

    fn listed(name: &str) -> Listed {
        Listed {
            source: StartupSource::MachineRun,
            name: name.to_owned(),
            command: String::new(),
            image: None,
        }
    }

    #[test]
    fn a_windows_tray_is_protected_and_an_off_entry_has_no_impact() {
        let windows = Path::new(r"C:\WINDOWS");
        let facts = Facts {
            approved: Some(&[6; 12]),
            image: Some(PathBuf::from(
                r"C:\WINDOWS\system32\SecurityHealthSystray.exe",
            )),
            verdict: Verdict {
                state: SignatureState::Valid,
                signer: Some("Microsoft Windows".to_owned()),
            },
            company: None,
        };
        let entry = entry_of(listed("SecurityHealth"), facts, windows, &[]);
        assert!(entry.is_protected && entry.is_enabled);
        let off = approved_mark(false, 0);
        let facts = Facts {
            approved: Some(&off),
            image: None,
            verdict: Verdict::NO_FILE,
            company: None,
        };
        let entry = entry_of(listed("Gone"), facts, windows, &[]);
        assert!(!entry.is_enabled && !entry.is_protected && entry.impact.is_none());
    }

    #[test]
    fn the_costly_ones_that_start_come_first() {
        let entry = |name: &str, is_enabled: bool, level: Option<ImpactLevel>| StartupEntry {
            source: StartupSource::UserRun,
            name: name.to_owned(),
            command: String::new(),
            image: None,
            is_enabled,
            disabled_at: None,
            signature: SignatureState::Unsigned,
            signer: None,
            company: None,
            is_protected: false,
            impact: level.map(|level| StartupImpact {
                level,
                cpu_ms: 0,
                disk_bytes: 0,
                boots: 1,
            }),
        };
        let mut entries = vec![
            entry("off", false, None),
            entry("b-low", true, Some(ImpactLevel::Low)),
            entry("unmeasured", true, None),
            entry("a-high", true, Some(ImpactLevel::High)),
        ];
        sort_entries(&mut entries);
        let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["a-high", "b-low", "unmeasured", "off"]);
    }

    #[test]
    fn a_startup_folder_lists_its_files_without_desktop_ini() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("desktop.ini"), b"[.ShellClassInfo]").unwrap();
        std::fs::write(dir.path().join("tool.bat"), b"@echo off").unwrap();
        let listed = listed_in_folder(StartupSource::UserFolder, dir.path());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "tool.bat");
        assert_eq!(listed[0].image, Some(dir.path().join("tool.bat")));
    }
}
