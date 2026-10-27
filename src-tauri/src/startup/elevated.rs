//! The admin action of startup: the mark of an entry of every user, through `vitals.exe --elevated startup-approve`.
//! The elevated side takes only a machine source and checks the entry again before it writes.

use vitals_ipc::StartupSetArgs;

pub const ACTION: &str = "startup-approve";

/// The elevated process: the change in, `Ok` or the reason out, so the panel can say what went wrong.
pub fn perform(request: &str) -> Result<String, String> {
    let change: StartupSetArgs =
        serde_json::from_str(request).map_err(|error| error.to_string())?;
    let outcome = if change.source.is_machine() {
        super::apply(&change, chrono::Utc::now().timestamp()).map_err(|error| error.to_string())
    } else {
        Err(super::StartupError::NotMachine.to_string())
    };
    serde_json::to_string(&outcome).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use vitals_ipc::StartupSource;

    use super::*;

    #[test]
    fn the_elevated_side_refuses_an_entry_of_the_user() {
        let change = StartupSetArgs {
            source: StartupSource::UserRun,
            name: "Steam".to_owned(),
            enable: false,
        };
        let report = perform(&serde_json::to_string(&change).unwrap()).unwrap();
        let outcome: Result<(), String> = serde_json::from_str(&report).unwrap();
        assert!(outcome.is_err());
    }
}
