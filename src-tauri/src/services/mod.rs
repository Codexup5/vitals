//! The Windows services, with who made each: the signature of its exe, or of its DLL when `svchost.exe` hosts it.
//! A service of Microsoft or a part of Windows is shown and never changed (the brief, §1 p.15: turning those off is
//! debloat). Another maker's automatic service can be turned to "start when asked" (`Manual`), never disabled; the
//! start type it had is kept in the database, so "Restore" brings it back exactly.

pub mod elevated;
#[cfg(windows)]
mod windows;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vitals_ipc::{ChangeOutcome, Service, ServiceSetArgs, ServiceState, SignatureState, StartType};

use crate::startup::{env_lookup, expand_env, image_of as command_image, resolve_candidate};
use crate::storage::{Storage, StorageError};
use crate::trust::{self, Verdict};

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("no service {0:?}")]
    NotFound(String),
    #[error("{0:?} is a service of Microsoft or of Windows: the panel does not change it")]
    Protected(String),
    #[error("the panel did not change {0:?}: there is nothing to restore")]
    NothingToRestore(String),
    #[error("{0:?} does not start with Windows: there is nothing to turn off")]
    NotAutomatic(String),
    #[error("the panel sets only manual or automatic starts")]
    Forbidden,
    #[error("the service manager refused: {0}")]
    Manager(String),
    #[error("{0}")]
    Elevated(String),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// A service as the service manager describes it, before anything is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawService {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub start: StartType,
    pub state: ServiceState,
    /// The command line of `ImagePath`
    pub binary: String,
    /// `Parameters\ServiceDll` of a service `svchost.exe` hosts
    pub service_dll: Option<String>,
}

/// What the elevated process is asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartChange {
    pub name: String,
    pub start: StartType,
}

/// `\SystemRoot\…` and `\??\C:\…` are kernel spellings of ordinary paths.
fn plain_path(text: &str, windows_dir: &Path) -> String {
    let text = text.trim();
    if let Some(rest) = text
        .get(..12)
        .filter(|head| head.eq_ignore_ascii_case(r"\SystemRoot\"))
        .and(text.get(12..))
    {
        return windows_dir.join(rest).display().to_string();
    }
    text.strip_prefix(r"\??\").unwrap_or(text).to_owned()
}

/// The file that is the service: the DLL of a hosted one, else the exe of its command line.
pub fn image_of(raw: &RawService, windows_dir: &Path) -> Option<PathBuf> {
    if let Some(dll) = &raw.service_dll {
        return Some(PathBuf::from(plain_path(
            &expand_env(dll, env_lookup),
            windows_dir,
        )));
    }
    let command = plain_path(&expand_env(&raw.binary, env_lookup), windows_dir);
    command_image(&command, |candidate| {
        resolve_candidate(candidate, windows_dir)
    })
}

/// Microsoft's or Windows' own; a service whose file is unknown counts as one, on the safe side.
pub fn is_microsoft(image: Option<&Path>, verdict: &Verdict, windows_dir: &Path) -> bool {
    let is_signed_by_microsoft = verdict.state == SignatureState::Valid
        && verdict.signer.as_deref().is_some_and(trust::is_microsoft);
    is_signed_by_microsoft
        || image.is_none_or(|image| trust::is_part_of_windows(image, windows_dir, verdict))
}

/// The start type a change asks for: `Manual` for off, the saved one for restore.
pub fn target_of(service: &Service, restore: bool) -> Result<StartType, ServiceError> {
    if service.is_microsoft {
        return Err(ServiceError::Protected(service.name.clone()));
    }
    if restore {
        return service
            .changed_from
            .ok_or_else(|| ServiceError::NothingToRestore(service.name.clone()));
    }
    if !service.start.is_automatic() {
        return Err(ServiceError::NotAutomatic(service.name.clone()));
    }
    Ok(StartType::Manual)
}

/// The elevated side sets nothing but these: never `Disabled`.
pub const fn is_allowed(start: StartType) -> bool {
    !matches!(start, StartType::Disabled)
}

pub fn service_of(
    raw: RawService,
    image: Option<PathBuf>,
    verdict: Verdict,
    company: Option<String>,
    changed_from: Option<StartType>,
    windows_dir: &Path,
) -> Service {
    let is_microsoft = is_microsoft(image.as_deref(), &verdict, windows_dir);
    Service {
        name: raw.name,
        display_name: raw.display_name,
        description: raw.description,
        start: raw.start,
        state: raw.state,
        image,
        signature: verdict.state,
        signer: verdict.signer,
        company,
        is_microsoft,
        changed_from,
    }
}

/// Other makers first, among them the ones that start with Windows, then by the name people see.
pub fn sort_services(services: &mut [Service]) {
    services.sort_by_key(|service| {
        (
            service.is_microsoft,
            !service.start.is_automatic() && service.changed_from.is_none(),
            service.display_name.to_lowercase(),
        )
    });
}

#[cfg(windows)]
pub use windows::{overview, read_one, set_start};

#[cfg(not(windows))]
pub fn read_one(_: &str, _: &Storage) -> Result<Service, ServiceError> {
    Err(ServiceError::Manager("only on Windows".to_owned()))
}

/// Turns a service to manual start or back; the change goes through UAC, where a "No" changes nothing. The start
/// type it had is saved before the change and dropped after a restore.
pub fn change(
    args: &ServiceSetArgs,
    storage: &Storage,
    work_dir: &Path,
) -> Result<ChangeOutcome, ServiceError> {
    let service = read_one(&args.name, storage)?;
    let start = target_of(&service, args.restore)?;
    if !args.restore {
        storage.put_service_change(&service.name, service.start, chrono::Utc::now().timestamp())?;
    }
    let request = StartChange {
        name: service.name.clone(),
        start,
    };
    let outcome =
        match crate::elevated::run::<_, Result<(), String>>(elevated::ACTION, &request, work_dir) {
            Ok(Ok(())) => Ok(ChangeOutcome::Done),
            Ok(Err(message)) => Err(ServiceError::Elevated(message)),
            Err(crate::elevated::ElevatedError::Declined) => Ok(ChangeOutcome::Declined),
            Err(error) => Err(ServiceError::Elevated(error.to_string())),
        };
    // Kept only while the service runs with the type the panel set
    let is_restored = args.restore && matches!(outcome, Ok(ChangeOutcome::Done));
    let is_unchanged = !args.restore && !matches!(outcome, Ok(ChangeOutcome::Done));
    if is_restored || is_unchanged {
        storage.remove_service_change(&service.name)?;
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(binary: &str, service_dll: Option<&str>) -> RawService {
        RawService {
            name: "svc".to_owned(),
            display_name: "Svc".to_owned(),
            description: None,
            start: StartType::Automatic,
            state: ServiceState::Running,
            binary: binary.to_owned(),
            service_dll: service_dll.map(str::to_owned),
        }
    }

    fn signed(signer: &str) -> Verdict {
        Verdict {
            state: SignatureState::Valid,
            signer: Some(signer.to_owned()),
        }
    }

    #[test]
    fn a_hosted_service_is_its_dll_and_kernel_paths_read_plainly() {
        let windows = Path::new(r"C:\WINDOWS");
        assert_eq!(
            image_of(
                &raw(
                    r"C:\WINDOWS\system32\svchost.exe -k netsvcs -p",
                    Some(r"C:\WINDOWS\system32\wuaueng.dll")
                ),
                windows
            ),
            Some(PathBuf::from(r"C:\WINDOWS\system32\wuaueng.dll"))
        );
        assert_eq!(
            plain_path(r"\SystemRoot\System32\x.exe", windows),
            r"C:\WINDOWS\System32\x.exe"
        );
        assert_eq!(
            plain_path(r"\??\C:\Tools\x.exe", windows),
            r"C:\Tools\x.exe"
        );
        assert_eq!(
            image_of(
                &raw(r#""C:\Gone\Docker\com.docker.service""#, None),
                windows
            ),
            Some(PathBuf::from(r"C:\Gone\Docker\com.docker.service"))
        );
    }

    #[test]
    fn microsoft_is_told_by_the_signer_or_the_windows_folder() {
        let windows = Path::new(r"C:\WINDOWS");
        let edge =
            Path::new(r"C:\Program Files (x86)\Microsoft\EdgeUpdate\MicrosoftEdgeUpdate.exe");
        assert!(is_microsoft(
            Some(edge),
            &signed("Microsoft Corporation"),
            windows
        ));
        let docker = Path::new(r"C:\Program Files\Docker\Docker\com.docker.service");
        assert!(!is_microsoft(Some(docker), &signed("Docker Inc"), windows));
        let nvidia =
            Path::new(r"C:\WINDOWS\System32\DriverStore\FileRepository\nv\NVDisplay.Container.exe");
        assert!(!is_microsoft(
            Some(nvidia),
            &signed("NVIDIA Corporation"),
            windows
        ));
        let unsigned = Verdict {
            state: SignatureState::Unsigned,
            signer: None,
        };
        assert!(is_microsoft(
            Some(Path::new(r"C:\WINDOWS\system32\x.dll")),
            &unsigned,
            windows
        ));
        assert!(!is_microsoft(
            Some(Path::new(r"C:\Tools\x.exe")),
            &unsigned,
            windows
        ));
        assert!(is_microsoft(None, &unsigned, windows));
    }

    fn service(start: StartType, is_microsoft: bool, changed_from: Option<StartType>) -> Service {
        Service {
            name: "svc".to_owned(),
            display_name: "Svc".to_owned(),
            description: None,
            start,
            state: ServiceState::Running,
            image: None,
            signature: SignatureState::Valid,
            signer: None,
            company: None,
            is_microsoft,
            changed_from,
        }
    }

    #[test]
    fn off_is_manual_and_restore_is_what_was_saved() {
        assert_eq!(
            target_of(&service(StartType::AutomaticDelayed, false, None), false).unwrap(),
            StartType::Manual
        );
        assert_eq!(
            target_of(
                &service(StartType::Manual, false, Some(StartType::AutomaticDelayed)),
                true
            )
            .unwrap(),
            StartType::AutomaticDelayed
        );
        assert!(matches!(
            target_of(&service(StartType::Automatic, true, None), false),
            Err(ServiceError::Protected(_))
        ));
        assert!(matches!(
            target_of(&service(StartType::Manual, false, None), false),
            Err(ServiceError::NotAutomatic(_))
        ));
        assert!(matches!(
            target_of(&service(StartType::Manual, false, None), true),
            Err(ServiceError::NothingToRestore(_))
        ));
        assert!(!is_allowed(StartType::Disabled));
    }

    #[test]
    fn other_makers_that_start_come_first() {
        let named = |name: &str, mut one: Service| {
            name.clone_into(&mut one.display_name);
            one
        };
        let mut services = vec![
            named("a-ms", service(StartType::Automatic, true, None)),
            named("b-manual", service(StartType::Manual, false, None)),
            named("c-auto", service(StartType::Automatic, false, None)),
            named(
                "d-changed",
                service(StartType::Manual, false, Some(StartType::Automatic)),
            ),
        ];
        sort_services(&mut services);
        let order: Vec<_> = services
            .iter()
            .map(|one| one.display_name.as_str())
            .collect();
        assert_eq!(order, ["c-auto", "d-changed", "b-manual", "a-ms"]);
    }
}
