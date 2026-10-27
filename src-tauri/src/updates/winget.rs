//! winget itself: the listing, an upgrade of one package, the version read back. Ids reach its command line only
//! when they are whole (`is_whole_id`), each as one argument.

use std::time::Duration;

use super::job::{Host, NoWinget};
use super::{InstalledProgram, Package, installed_version, is_open, packages_of};
use crate::process::{RunError, output};

const PROGRAM: &str = "winget";
/// The listing refreshes winget's sources first; past this the check is reported as failed
const LIST_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// A big installer (Docker Desktop) takes minutes; past this the upgrade is stopped and reported as failed
const UPGRADE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const QUERY_TIMEOUT: Duration = Duration::from_secs(2 * 60);
/// No prompt may wait for an answer nobody sees: the panel runs winget without a window
const QUIET: [&str; 2] = ["--disable-interactivity", "--accept-source-agreements"];

/// The real host: winget, the registry and the process list.
#[derive(Debug, Default)]
pub struct WingetHost;

#[cfg(windows)]
fn programs() -> Vec<InstalledProgram> {
    super::windows::installed_programs()
}

#[cfg(not(windows))]
fn programs() -> Vec<InstalledProgram> {
    Vec::new()
}

#[cfg(windows)]
fn running() -> Vec<std::path::PathBuf> {
    crate::system::windows::running_exes()
}

#[cfg(not(windows))]
fn running() -> Vec<std::path::PathBuf> {
    Vec::new()
}

impl Host for WingetHost {
    fn list(&self) -> Result<Option<Vec<Package>>, NoWinget> {
        match output(PROGRAM, &[&["upgrade"], &QUIET[..]].concat(), LIST_TIMEOUT) {
            // winget exits with an error code when nothing is upgradable; the table, or its absence, tells
            Ok(done) => Ok(Some(packages_of(&done.stdout))),
            Err(RunError::Missing) => Err(NoWinget),
            Err(error) => {
                tracing::warn!("winget upgrade did not list: {error}");
                Ok(None)
            }
        }
    }

    fn is_open(&self, package: &Package) -> bool {
        is_open(package, &programs(), &running())
    }

    fn upgrade(&self, package: &Package) -> Option<i32> {
        if !package.can_upgrade {
            return None;
        }
        let args = [
            "upgrade",
            "--id",
            &package.id,
            "--exact",
            "--source",
            &package.source,
            "--silent",
            "--accept-package-agreements",
            QUIET[0],
            QUIET[1],
        ];
        output(PROGRAM, &args, UPGRADE_TIMEOUT)
            .map_err(|error| tracing::warn!("winget upgrade {} failed: {error}", package.id))
            .ok()
            .map(|done| done.code)
    }

    fn installed_version(&self, id: &str) -> Option<String> {
        let args = [&["list", "--id", id, "--exact"], &QUIET[..]].concat();
        let done = output(PROGRAM, &args, QUERY_TIMEOUT).ok()?;
        installed_version(&done.stdout, id)
    }
}
