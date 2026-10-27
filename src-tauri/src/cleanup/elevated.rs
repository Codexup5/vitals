//! The admin action of the brief, §1 p.10: `vitals.exe --elevated clean-windows-temp <plan> <report>`. The same exe
//! runs under UAC for this one action and exits; a refusal at the prompt means the action does not run.
//!
//! The elevated side trusts nothing in the plan file: the root must be `%SystemRoot%\Temp` as it computes it itself,
//! and every entry passes the same checks as in a normal run.

use std::path::{Path, PathBuf};

use super::execute::{self, Tally, Target};

pub const FLAG: &str = "--elevated";
pub const CLEAN_WINDOWS_TEMP: &str = "clean-windows-temp";

#[derive(Debug, thiserror::Error)]
pub enum ElevatedError {
    #[error("the user declined the admin rights")]
    Declined,
    #[error("the admin action failed: {0}")]
    Failed(String),
}

/// `C:\Windows\Temp`, canonical, from the environment of this process.
pub fn windows_temp() -> Option<PathBuf> {
    let root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    std::fs::canonicalize(root.join("Temp")).ok()
}

/// What the elevated side does with a plan: refuse a root other than the one it expects, else run it.
pub fn execute_plan(target: &Target, expected_root: &Path) -> Tally {
    if !super::guard::is_within(&target.root, expected_root)
        || !super::guard::is_within(expected_root, &target.root)
    {
        return Tally {
            refused: u64::try_from(target.entries.len()).unwrap_or(u64::MAX),
            ..Tally::default()
        };
    }
    execute::run(target, &[])
}

/// Runs in the elevated process when its arguments ask for an action; `None` for a normal start.
pub fn handle(args: &[String]) -> Option<std::process::ExitCode> {
    if args.get(1).map(String::as_str) != Some(FLAG) {
        return None;
    }
    let outcome = (|| -> Result<(), String> {
        if args.get(2).map(String::as_str) != Some(CLEAN_WINDOWS_TEMP) {
            return Err("unknown elevated action".to_owned());
        }
        let (plan, report) = (
            args.get(3).ok_or("no plan")?,
            args.get(4).ok_or("no report")?,
        );
        let text = std::fs::read_to_string(plan).map_err(|error| error.to_string())?;
        let target: Target = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let expected = windows_temp().ok_or("no Windows temp folder")?;
        let tally = execute_plan(&target, &expected);
        let body = serde_json::to_string(&tally).map_err(|error| error.to_string())?;
        std::fs::write(report, body).map_err(|error| error.to_string())
    })();
    Some(if outcome.is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    })
}

/// Hands a Windows temp target to the elevated exe and reads back what it did.
#[cfg(windows)]
pub fn run(target: &Target, work_dir: &Path) -> Result<Tally, ElevatedError> {
    use super::windows::{Elevated, run_elevated};

    let failed = |message: String| ElevatedError::Failed(message);
    let plan = work_dir.join("elevated-plan.json");
    let report = work_dir.join("elevated-report.json");
    let _ = std::fs::remove_file(&report);
    let body = serde_json::to_string(target).map_err(|error| failed(error.to_string()))?;
    std::fs::write(&plan, body).map_err(|error| failed(error.to_string()))?;
    let exe = std::env::current_exe().map_err(|error| failed(error.to_string()))?;
    let parameters = format!(
        "{FLAG} {CLEAN_WINDOWS_TEMP} \"{}\" \"{}\"",
        plan.display(),
        report.display()
    );
    let outcome = run_elevated(&exe, &parameters);
    let _ = std::fs::remove_file(&plan);
    match outcome {
        Elevated::Declined => Err(ElevatedError::Declined),
        Elevated::Failed(message) => Err(failed(message)),
        Elevated::Exited(code) if code != 0 => Err(failed(format!("exit code {code}"))),
        Elevated::Exited(_) => {
            let text =
                std::fs::read_to_string(&report).map_err(|error| failed(error.to_string()))?;
            let _ = std::fs::remove_file(&report);
            serde_json::from_str(&text).map_err(|error| failed(error.to_string()))
        }
    }
}

#[cfg(not(windows))]
pub fn run(_: &Target, _: &Path) -> Result<Tally, ElevatedError> {
    Err(ElevatedError::Failed(
        "admin actions exist only on Windows".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::snapshot::{Filter, take};
    use super::*;

    #[test]
    fn the_elevated_side_refuses_a_root_it_did_not_expect() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("file.txt"), b"keep").unwrap();
        let root = fs::canonicalize(&other).unwrap();
        let target = Target {
            entries: take(&root, Filter::All),
            root,
            removes_root: false,
        };
        let expected = fs::canonicalize(dir.path()).unwrap().join("windows-temp");
        let tally = execute_plan(&target, &expected);
        assert_eq!((tally.deleted, tally.refused), (0, 1));
        assert!(other.join("file.txt").exists());
    }

    #[test]
    fn a_normal_start_is_not_an_elevated_action() {
        assert!(handle(&["vitals.exe".to_owned()]).is_none());
        assert!(
            handle(&[
                "vitals.exe".to_owned(),
                FLAG.to_owned(),
                "format-c".to_owned()
            ])
            .is_some()
        );
    }
}
