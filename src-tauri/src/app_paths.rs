use std::ffi::OsString;
use std::path::PathBuf;

const DATA_FOLDER: &str = "Vitals";

#[derive(Debug, thiserror::Error)]
pub enum AppPathsError {
    #[error("APPDATA is not set: the panel has nowhere to keep its data")]
    MissingAppData,
    #[error("could not create the data folder {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// `%APPDATA%\Vitals`: the only place the panel writes personal data; never the repository or the exe folder.
pub fn data_dir_from(app_data: Option<OsString>) -> Result<PathBuf, AppPathsError> {
    let app_data = app_data
        .filter(|value| !value.is_empty())
        .ok_or(AppPathsError::MissingAppData)?;
    Ok(PathBuf::from(app_data).join(DATA_FOLDER))
}

/// `VITALS_E2E_ROOT` in a debug build: every folder the panel reads or writes lives under it, so the e2e scenario
/// scans and cleans a fixture, never the PC. A release build has no such switch.
#[cfg(debug_assertions)]
pub fn e2e_root() -> Option<PathBuf> {
    std::env::var_os("VITALS_E2E_ROOT")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(not(debug_assertions))]
pub const fn e2e_root() -> Option<PathBuf> {
    None
}

pub fn ensure_data_dir() -> Result<PathBuf, AppPathsError> {
    let app_data = e2e_root().map_or_else(
        || std::env::var_os("APPDATA"),
        |root| Some(root.join("appdata").into_os_string()),
    );
    let path = data_dir_from(app_data)?;
    std::fs::create_dir_all(&path).map_err(|source| AppPathsError::CreateDir {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_is_vitals_inside_app_data() {
        let path = data_dir_from(Some(OsString::from(r"C:\Users\someone\AppData\Roaming")));
        assert_eq!(
            path.ok(),
            Some(PathBuf::from(r"C:\Users\someone\AppData\Roaming\Vitals"))
        );
    }

    #[test]
    fn missing_app_data_is_an_error() {
        assert!(matches!(
            data_dir_from(None),
            Err(AppPathsError::MissingAppData)
        ));
    }

    #[test]
    fn empty_app_data_is_an_error() {
        assert!(matches!(
            data_dir_from(Some(OsString::new())),
            Err(AppPathsError::MissingAppData)
        ));
    }
}
