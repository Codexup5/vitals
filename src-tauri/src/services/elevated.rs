//! The admin action of services: the start type of one service, through `vitals.exe --elevated service-start`.
//! The elevated side reads the service again, refuses Microsoft's and Windows' own and never disables one.

use super::StartChange;

pub const ACTION: &str = "service-start";

/// The elevated process: the change in, `Ok` or the reason out.
pub fn perform(request: &str) -> Result<String, String> {
    let change: StartChange = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let outcome = check_and_set(&change).map_err(|error| error.to_string());
    serde_json::to_string(&outcome).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn check_and_set(change: &StartChange) -> Result<(), super::ServiceError> {
    if !super::is_allowed(change.start) {
        return Err(super::ServiceError::Forbidden);
    }
    let service = super::windows::read_unsaved(&change.name)?;
    if service.is_microsoft {
        return Err(super::ServiceError::Protected(change.name.clone()));
    }
    super::set_start(&change.name, change.start)
}

#[cfg(not(windows))]
fn check_and_set(_: &StartChange) -> Result<(), super::ServiceError> {
    Err(super::ServiceError::Manager("only on Windows".to_owned()))
}

#[cfg(test)]
mod tests {
    use vitals_ipc::StartType;

    use super::*;

    #[test]
    fn the_elevated_side_never_disables() {
        let change = StartChange {
            name: "Docker Desktop Service".to_owned(),
            start: StartType::Disabled,
        };
        let report = perform(&serde_json::to_string(&change).unwrap()).unwrap();
        let outcome: Result<(), String> = serde_json::from_str(&report).unwrap();
        assert!(outcome.is_err());
    }
}
