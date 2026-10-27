//! The admin action of the cleanup: `C:\Windows\Temp` through `vitals.exe --elevated clean-windows-temp`.
//!
//! The elevated side trusts nothing in the plan file: the root must be `%SystemRoot%\Temp` as it computes it itself,
//! and every entry passes the same checks as in a normal run.

use std::path::{Path, PathBuf};

use super::execute::{self, Tally, Target};
pub use crate::elevated::ElevatedError;

pub const ACTION: &str = "clean-windows-temp";

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

/// The elevated process: the plan in, the tally out.
pub fn perform(request: &str) -> Result<String, String> {
    let target: Target = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let expected = windows_temp().ok_or("no Windows temp folder")?;
    serde_json::to_string(&execute_plan(&target, &expected)).map_err(|error| error.to_string())
}

/// Hands a Windows temp target to the elevated exe and reads back what it did.
pub fn run(target: &Target, work_dir: &Path) -> Result<Tally, ElevatedError> {
    crate::elevated::run(ACTION, target, work_dir)
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
}
