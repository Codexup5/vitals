//! The admin actions of the brief, §1 p.10: `vitals.exe --elevated <action> <request> <report>`. The same exe runs
//! under UAC for one action and exits; a refusal at the prompt means the action does not run. The request and the
//! report are JSON files in `%APPDATA%\Vitals`, so no argument of the command line needs quoting.
//!
//! The elevated side trusts nothing in a request: every action checks it again as if a stranger wrote it.

use std::path::Path;
use std::process::ExitCode;

use serde::Serialize;
use serde::de::DeserializeOwned;

#[cfg(windows)]
mod windows;

pub const FLAG: &str = "--elevated";

#[derive(Debug, thiserror::Error)]
pub enum ElevatedError {
    #[error("the user declined the admin rights")]
    Declined,
    #[error("the admin action failed: {0}")]
    Failed(String),
}

/// What an action does in the elevated process: the request in, the report out, as JSON.
type Perform = fn(&str) -> Result<String, String>;

fn action_of(name: &str) -> Option<Perform> {
    match name {
        crate::cleanup::elevated::ACTION => Some(crate::cleanup::elevated::perform),
        crate::startup::elevated::ACTION => Some(crate::startup::elevated::perform),
        crate::services::elevated::ACTION => Some(crate::services::elevated::perform),
        _ => None,
    }
}

/// Runs in the elevated process when its arguments ask for an action; `None` for a normal start.
pub fn handle(args: &[String]) -> Option<ExitCode> {
    if args.get(1).map(String::as_str) != Some(FLAG) {
        return None;
    }
    let outcome = (|| -> Result<(), String> {
        let perform = args
            .get(2)
            .and_then(|name| action_of(name))
            .ok_or("unknown elevated action")?;
        let (request, report) = (
            args.get(3).ok_or("no request")?,
            args.get(4).ok_or("no report")?,
        );
        let text = std::fs::read_to_string(request).map_err(|error| error.to_string())?;
        let body = perform(&text)?;
        std::fs::write(report, body).map_err(|error| error.to_string())
    })();
    Some(if outcome.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Hands `request` to the elevated exe for `action` and reads back its report.
#[cfg(windows)]
pub fn run<Q: Serialize, R: DeserializeOwned>(
    action: &str,
    request: &Q,
    work_dir: &Path,
) -> Result<R, ElevatedError> {
    use windows::{Elevated, run_elevated};

    let failed = |message: String| ElevatedError::Failed(message);
    let request_path = work_dir.join(format!("elevated-{action}.json"));
    let report = work_dir.join(format!("elevated-{action}-report.json"));
    let _ = std::fs::remove_file(&report);
    let body = serde_json::to_string(request).map_err(|error| failed(error.to_string()))?;
    std::fs::write(&request_path, body).map_err(|error| failed(error.to_string()))?;
    let exe = std::env::current_exe().map_err(|error| failed(error.to_string()))?;
    let parameters = format!(
        "{FLAG} {action} \"{}\" \"{}\"",
        request_path.display(),
        report.display()
    );
    let outcome = run_elevated(&exe, &parameters);
    let _ = std::fs::remove_file(&request_path);
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
pub fn run<Q: Serialize, R: DeserializeOwned>(
    _: &str,
    _: &Q,
    _: &Path,
) -> Result<R, ElevatedError> {
    Err(ElevatedError::Failed(
        "admin actions exist only on Windows".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
