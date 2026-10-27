//! Who made a file: its Authenticode signature, checked offline, and what its version resource says of itself.
//! Startup entries use it now; the services of stage 7b tell Microsoft's own from the rest by it.

use std::path::Path;

use vitals_ipc::SignatureState;

#[cfg(windows)]
pub mod windows;

/// The signature of a file and its signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub state: SignatureState,
    /// The subject of the signing certificate, even when the chain does not hold
    pub signer: Option<String>,
}

impl Verdict {
    pub const NO_FILE: Self = Self {
        state: SignatureState::NoFile,
        signer: None,
    };
}

/// `Microsoft Windows`, `Microsoft Corporation`, `Microsoft Windows Publisher`…
pub fn is_microsoft(signer: &str) -> bool {
    signer == "Microsoft" || signer.starts_with("Microsoft ")
}

/// `path` lies inside `root`, compared as Windows compares paths: by component, case aside.
pub fn is_inside(path: &Path, root: &Path) -> bool {
    let fold = |path: &Path| {
        path.components()
            .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
            .collect::<Vec<_>>()
    };
    let (path, root) = (fold(path), fold(root));
    !root.is_empty() && path.len() > root.len() && path.starts_with(&root)
}

/// An app execution alias of the Store (`%LOCALAPPDATA%\Microsoft\WindowsApps`): a link `WinVerifyTrust` cannot read.
pub fn is_app_alias(path: &Path, local_app_data: &Path) -> bool {
    is_inside(path, &local_app_data.join(r"Microsoft\WindowsApps"))
}

/// A part of Windows itself: a file of the Windows folder that no other maker signed. The panel shows such a thing
/// and never turns it off (the brief, §1 p.15 and §7).
pub fn is_part_of_windows(image: &Path, windows_dir: &Path, verdict: &Verdict) -> bool {
    let is_other_maker = verdict.state == SignatureState::Valid
        && verdict
            .signer
            .as_deref()
            .is_some_and(|signer| !is_microsoft(signer));
    is_inside(image, windows_dir) && !is_other_maker
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed(signer: &str) -> Verdict {
        Verdict {
            state: SignatureState::Valid,
            signer: Some(signer.to_owned()),
        }
    }

    #[test]
    fn microsoft_is_told_by_the_signer_name() {
        assert!(is_microsoft("Microsoft Windows"));
        assert!(is_microsoft("Microsoft Corporation"));
        assert!(!is_microsoft("Microsoftish Software Ltd"));
        assert!(!is_microsoft("Realtek Semiconductor Corp."));
    }

    #[test]
    fn inside_compares_components_case_aside() {
        let windows = Path::new(r"C:\Windows");
        assert!(is_inside(Path::new(r"c:\WINDOWS\System32\a.exe"), windows));
        assert!(!is_inside(Path::new(r"C:\WindowsApps\a.exe"), windows));
        assert!(!is_inside(windows, windows));
    }

    #[test]
    fn a_store_alias_is_told_apart() {
        let local = Path::new(r"C:\Users\someone\AppData\Local");
        let teams = Path::new(
            r"C:\Users\someone\AppData\Local\Microsoft\WindowsApps\MSTeams_8wekyb3d8bbwe\ms-teams.exe",
        );
        assert!(is_app_alias(teams, local));
        let discord = Path::new(r"C:\Users\someone\AppData\Local\Discord\Update.exe");
        assert!(!is_app_alias(discord, local));
    }

    #[test]
    fn a_windows_file_is_protected_unless_another_maker_signed_it() {
        let windows = Path::new(r"C:\Windows");
        let tray = Path::new(r"C:\Windows\System32\SecurityHealthSystray.exe");
        assert!(is_part_of_windows(
            tray,
            windows,
            &signed("Microsoft Windows")
        ));
        let unsigned = Verdict {
            state: SignatureState::Unsigned,
            signer: None,
        };
        assert!(is_part_of_windows(tray, windows, &unsigned));
        let realtek =
            Path::new(r"C:\Windows\System32\DriverStore\FileRepository\x\RtkAudUService64.exe");
        assert!(!is_part_of_windows(
            realtek,
            windows,
            &signed("Realtek Semiconductor Corp.")
        ));
        let edge = Path::new(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");
        assert!(!is_part_of_windows(
            edge,
            windows,
            &signed("Microsoft Corporation")
        ));
    }
}
