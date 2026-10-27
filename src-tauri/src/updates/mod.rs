//! Updates of programs through winget: its table read into packages, an upgrade run one package at a time, the
//! version read back after it. The panel never reboots the PC and never passes `--allow-reboot` (the brief, §1 p.9).

pub mod job;
pub mod table;
#[cfg(windows)]
pub mod windows;
pub mod winget;

use std::path::{Path, PathBuf};

pub use vitals_ipc::{Package, UpgradeOutcome, UpgradeResult};

/// The columns of `winget upgrade` and `winget list`, by position: their titles follow the language of Windows
const NAME: usize = 0;
const ID: usize = 1;
const VERSION: usize = 2;
const AVAILABLE: usize = 3;
const SOURCE: usize = 4;

/// winget cut the text to fit its column
const ELLIPSIS: char = '…';

/// An id winget can be told with `--id … --exact`: whole, and nothing a command line would read as more.
pub fn is_whole_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || "._-+".contains(letter))
}

/// The packages of `winget upgrade`. A footer line is not one: "7 upgrades available." does not fill the columns,
/// and a long one ("1 package(s) have version numbers…") fills them with words, where an id and a source have no space.
pub fn packages_of(output: &str) -> Vec<Package> {
    table::parse(output)
        .into_iter()
        .filter_map(|row| {
            let field = |index: usize| row.get(index).cloned().unwrap_or_default();
            let (id, installed, available, source) =
                (field(ID), field(VERSION), field(AVAILABLE), field(SOURCE));
            let is_word = |text: &str| !text.is_empty() && !text.contains(char::is_whitespace);
            if !is_word(&id) || !is_word(&source) || installed.is_empty() || available.is_empty() {
                return None;
            }
            Some(Package {
                can_upgrade: is_whole_id(&id) && is_whole_id(&source),
                name: field(NAME),
                id,
                installed,
                available,
                source,
                is_open: false,
            })
        })
        .collect()
}

/// The installed version `winget list --id <id> --exact` shows for exactly this id.
pub fn installed_version(output: &str, id: &str) -> Option<String> {
    table::parse(output).into_iter().find_map(|row| {
        (row.get(ID).map(String::as_str) == Some(id))
            .then(|| row.get(VERSION).cloned())
            .flatten()
            .filter(|version| !version.is_empty())
    })
}

/// winget's own exit codes (`returnCodes.md` of winget-cli), read as what the user should do next. winget exits
/// with an HRESULT, which the process code carries as a negative i32.
pub fn outcome_of(code: i32) -> UpgradeOutcome {
    const PACKAGE_IN_USE: i32 = 0x8A15_0101_u32.cast_signed();
    const FILE_IN_USE: i32 = 0x8A15_0103_u32.cast_signed();
    const IN_USE_BY_APPLICATION: i32 = 0x8A15_0111_u32.cast_signed();
    const REBOOT_TO_FINISH: i32 = 0x8A15_0109_u32.cast_signed();
    const REBOOT_FOR_INSTALL: i32 = 0x8A15_010A_u32.cast_signed();
    const REBOOT_INITIATED: i32 = 0x8A15_010B_u32.cast_signed();
    const CANCELLED_BY_USER: i32 = 0x8A15_010C_u32.cast_signed();
    /// `ERROR_CANCELLED`: "No" at the UAC prompt of an installer
    const UAC_DECLINED: i32 = 1223;
    match code {
        0 => UpgradeOutcome::Done,
        PACKAGE_IN_USE | FILE_IN_USE | IN_USE_BY_APPLICATION => UpgradeOutcome::Open,
        REBOOT_TO_FINISH | REBOOT_FOR_INSTALL | REBOOT_INITIATED => UpgradeOutcome::NeedsRestart,
        CANCELLED_BY_USER | UAC_DECLINED => UpgradeOutcome::Declined,
        _ => UpgradeOutcome::Failed,
    }
}

/// Folder names that hold many programs: a program "installed" there would make every process look like it.
const SHARED_FOLDERS: &[&str] = &[
    "program files",
    "program files (x86)",
    "programdata",
    "windows",
    "system32",
    "users",
    "appdata",
    "local",
    "roaming",
    "programs",
    "common files",
    "windowsapps",
];

/// The folder of one program: at least two levels below the drive, not a folder shared by many programs.
pub fn is_program_folder(path: &Path) -> bool {
    let normal = path
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .count();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase());
    normal >= 2 && name.is_some_and(|name| !SHARED_FOLDERS.contains(&name.as_str()))
}

/// Where a program from "Apps & features" lives: its `InstallLocation`, else the folder of its `DisplayIcon` exe.
pub fn program_folder(location: Option<&str>, icon: Option<&str>) -> Option<PathBuf> {
    let location = location
        .map(|location| location.trim().trim_matches('"'))
        .filter(|location| !location.is_empty())
        .map(PathBuf::from);
    let from_icon = || {
        let icon = icon?.trim();
        // `"C:\…\app.exe",0`: the path, its quotes and the icon index
        let path = icon
            .rsplit_once(',')
            .filter(|(_, index)| index.trim().parse::<i32>().is_ok())
            .map_or(icon, |(path, _)| path)
            .trim()
            .trim_matches('"');
        let path = Path::new(path);
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            .then(|| path.parent().map(Path::to_path_buf))
            .flatten()
    };
    location
        .or_else(from_icon)
        .filter(|folder| is_program_folder(folder))
}

/// A program of "Apps & features": its display name and folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProgram {
    pub name: String,
    pub folder: PathBuf,
}

/// The folders of the programs named like the package; a name winget cut matches by its start.
pub fn folders_of<'a>(
    package: &Package,
    programs: &'a [InstalledProgram],
) -> impl Iterator<Item = &'a Path> {
    let name = package.name.to_lowercase();
    let (wanted, is_cut) = name
        .strip_suffix(ELLIPSIS)
        .map_or((name.as_str(), false), |start| (start, true));
    let wanted = wanted.to_owned();
    programs
        .iter()
        .filter(move |program| {
            let display = program.name.to_lowercase();
            !wanted.is_empty() && (display == wanted || (is_cut && display.starts_with(&wanted)))
        })
        .map(|program| program.folder.as_path())
}

/// One of the running exes lives in a folder of the package.
pub fn is_open(package: &Package, programs: &[InstalledProgram], running: &[PathBuf]) -> bool {
    let running: Vec<String> = running
        .iter()
        .map(|exe| exe.to_string_lossy().to_lowercase())
        .collect();
    folders_of(package, programs).any(|folder| {
        let folder = folder.to_string_lossy().to_lowercase();
        let folder = folder.trim_end_matches('\\');
        running.iter().any(|exe| {
            exe.strip_prefix(folder)
                .is_some_and(|rest| rest.starts_with('\\'))
        })
    })
}

/// `09/17/2026`, as the firmware writes its date, to `2026-09-17`.
pub fn iso_date(date: &str) -> Option<String> {
    let mut parts = date.trim().split('/');
    let (month, day, year) = (parts.next()?, parts.next()?, parts.next()?);
    let is_number = |text: &str, digits: usize| {
        text.len() == digits && text.bytes().all(|byte| byte.is_ascii_digit())
    };
    (is_number(month, 2) && is_number(day, 2) && is_number(year, 4) && parts.next().is_none())
        .then(|| format!("{year}-{month}-{day}"))
}

/// The GPU vendor's app, where its installer puts it: NVIDIA App, then AMD Software.
pub fn gpu_tool(program_files: &Path) -> Option<PathBuf> {
    [
        program_files.join(r"NVIDIA Corporation\NVIDIA App\CEF\NVIDIA App.exe"),
        program_files.join(r"AMD\CNext\CNext\RadeonSoftware.exe"),
    ]
    .into_iter()
    .find(|exe| exe.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_firmware_date_reads_as_iso() {
        assert_eq!(iso_date("09/17/2026").as_deref(), Some("2026-09-17"));
        assert_eq!(iso_date("2026-09-17"), None);
        assert_eq!(iso_date("9/1/2026"), None);
    }

    const UPGRADE: &str = "\r   - \r   \\ \r\
Name            Id                      Version         Available     Source\r
----------------------------------------------------------------------------\r
Battle.net      Blizzard.BattleNet      Unknown         2.0.0.4232    winget\r
Git             Git.Git                 2.51.2          2.56.0.3      winget\r
Ubisoft Connect Ubisoft.Connect         < 174.2.0.13389 174.2.0.13389 winget\r
Some Long Na…   Vendor.SomeLongNam…     1.0             2.0           winget\r
7 upgrades available.\r
1 package(s) have version numbers that cannot be determined. Use --include-unknown to see all results.\r
";

    #[test]
    fn reads_the_upgrades_and_not_the_footer() {
        let packages = packages_of(UPGRADE);
        let ids: Vec<&str> = packages.iter().map(|package| package.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "Blizzard.BattleNet",
                "Git.Git",
                "Ubisoft.Connect",
                "Vendor.SomeLongNam…"
            ]
        );
        assert_eq!(packages[1].installed, "2.51.2");
        assert_eq!(packages[1].available, "2.56.0.3");
        assert_eq!(packages[2].installed, "< 174.2.0.13389");
        assert!(packages[1].can_upgrade);
        // A cut id cannot be told to winget exactly: it is listed, not upgraded
        assert!(!packages[3].can_upgrade);
    }

    #[test]
    fn nothing_to_upgrade_is_an_empty_list() {
        assert_eq!(
            packages_of("No installed package found matching input criteria.\r\n"),
            Vec::<Package>::new()
        );
        assert_eq!(packages_of(""), Vec::<Package>::new());
    }

    #[test]
    fn an_id_with_a_space_or_a_quote_is_not_whole() {
        assert!(is_whole_id("Microsoft.VisualStudioCode"));
        assert!(is_whole_id("9NBLGGH4NNS1"));
        assert!(!is_whole_id("Git.Git --force"));
        assert!(!is_whole_id("a\"b"));
        assert!(!is_whole_id(""));
    }

    #[test]
    fn reads_the_version_back_for_exactly_the_id() {
        let output = "Name Id      Version  Source\r\n----------------------------\r\nGit  Git.Git 2.56.0.3 winget\r\n";
        assert_eq!(
            installed_version(output, "Git.Git").as_deref(),
            Some("2.56.0.3")
        );
        assert_eq!(installed_version(output, "Git"), None);
    }

    #[test]
    fn exit_codes_say_what_to_do() {
        assert_eq!(outcome_of(0), UpgradeOutcome::Done);
        assert_eq!(
            outcome_of(0x8A15_0101_u32.cast_signed()),
            UpgradeOutcome::Open
        );
        assert_eq!(
            outcome_of(0x8A15_0109_u32.cast_signed()),
            UpgradeOutcome::NeedsRestart
        );
        assert_eq!(outcome_of(1223), UpgradeOutcome::Declined);
        assert_eq!(outcome_of(1), UpgradeOutcome::Failed);
    }

    #[test]
    fn a_shared_folder_is_not_a_program_folder() {
        assert!(is_program_folder(Path::new(r"C:\Program Files\Git")));
        assert!(is_program_folder(Path::new(
            r"C:\Users\someone\AppData\Local\Programs\Microsoft VS Code"
        )));
        assert!(!is_program_folder(Path::new(r"C:\Program Files")));
        assert!(!is_program_folder(Path::new(r"C:\Git")));
        assert!(!is_program_folder(Path::new(
            r"C:\Users\someone\AppData\Local"
        )));
    }

    #[test]
    fn the_folder_comes_from_the_location_or_the_icon() {
        assert_eq!(
            program_folder(Some(r"C:\Program Files\Git\"), None),
            Some(PathBuf::from(r"C:\Program Files\Git\"))
        );
        assert_eq!(
            program_folder(
                None,
                Some(r#""C:\Program Files\Docker\Docker\Docker Desktop.exe",0"#)
            ),
            Some(PathBuf::from(r"C:\Program Files\Docker\Docker"))
        );
        assert_eq!(program_folder(Some(""), Some(r"C:\Windows\icon.ico")), None);
        assert_eq!(program_folder(Some(r"C:\Program Files"), None), None);
    }

    fn package(name: &str) -> Package {
        Package {
            id: "Vendor.App".to_owned(),
            name: name.to_owned(),
            installed: "1".to_owned(),
            available: "2".to_owned(),
            source: "winget".to_owned(),
            is_open: false,
            can_upgrade: true,
        }
    }

    #[test]
    fn a_package_is_open_when_an_exe_of_its_folder_runs() {
        let programs = [
            InstalledProgram {
                name: "Docker Desktop".to_owned(),
                folder: PathBuf::from(r"C:\Program Files\Docker\Docker"),
            },
            InstalledProgram {
                name: "Git".to_owned(),
                folder: PathBuf::from(r"C:\Program Files\Git"),
            },
        ];
        let running = [PathBuf::from(
            r"C:\Program Files\Docker\Docker\Docker Desktop.exe",
        )];
        assert!(is_open(&package("Docker Desktop"), &programs, &running));
        assert!(is_open(&package("Docker Desk…"), &programs, &running));
        assert!(!is_open(&package("Git"), &programs, &running));
        // A sibling folder that starts the same is another program
        let sibling = [PathBuf::from(r"C:\Program Files\Git2\git.exe")];
        assert!(!is_open(&package("Git"), &programs, &sibling));
        assert!(!is_open(&package(""), &programs, &running));
    }
}
