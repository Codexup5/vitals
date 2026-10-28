//! The panel's own start at sign-in (the brief, §1 p.12): a `Vitals` value under this user's Run key that starts this
//! exe with `--autostart`, which keeps the window in the tray. Task Manager's mark for the value counts too, so a
//! "Disable" there reads as off here.

use std::path::{Path, PathBuf};

use vitals_ipc::AutostartStatus;

use super::{approved_state, image_of};

/// The name of the value under the Run key
pub const VALUE: &str = "Vitals";
/// The start at sign-in: the panel in the tray, without its window
pub const FLAG: &str = "--autostart";
/// The settings key that says the first start of a release decided the default
pub const DECIDED_KEY: &str = "autostart.decided";

/// What the Run value holds for this exe: the quoted path and the flag.
pub fn command_of(exe: &Path) -> String {
    format!("\"{}\" {FLAG}", exe.display())
}

/// The exe a Run command starts, compared as Windows compares paths: without regard to case.
fn starts(command: &str, exe: &Path) -> bool {
    image_of(command, |candidate| Some(PathBuf::from(candidate))).is_some_and(|image| {
        image
            .to_string_lossy()
            .eq_ignore_ascii_case(&exe.to_string_lossy())
    })
}

/// On when the value is there and Task Manager has not marked it off; the copy it starts when it is not this one.
pub fn status_of(command: Option<&str>, approved: Option<&[u8]>, exe: &Path) -> AutostartStatus {
    let Some(command) = command else {
        return AutostartStatus {
            is_enabled: false,
            other_copy: None,
        };
    };
    AutostartStatus {
        is_enabled: approved_state(approved).0,
        other_copy: (!starts(command, exe)).then(|| {
            image_of(command, |candidate| Some(PathBuf::from(candidate)))
                .map_or_else(|| command.to_owned(), |image| image.display().to_string())
        }),
    }
}

/// The brief, §1 p.12: on where the old OLEGPC collector lived, off on any other PC. The first start of a release
/// decides once; after that only the switch in Settings changes it.
pub const fn turns_on_at_first_start(is_decided: bool, has_legacy: bool) -> bool {
    !is_decided && has_legacy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::startup::approved_mark;

    const EXE: &str = r"C:\Users\me\AppData\Local\Programs\Vitals\vitals.exe";

    #[test]
    fn the_value_starts_this_exe_in_the_tray() {
        let command = command_of(Path::new(EXE));
        assert_eq!(command, format!("\"{EXE}\" --autostart"));
        let status = status_of(Some(&command), None, Path::new(EXE));
        assert_eq!(
            status,
            AutostartStatus {
                is_enabled: true,
                other_copy: None
            }
        );
        // Windows paths do not differ by case
        let status = status_of(Some(&command.to_uppercase()), None, Path::new(EXE));
        assert_eq!(status.other_copy, None);
    }

    #[test]
    fn no_value_or_a_mark_of_task_manager_is_off() {
        assert!(!status_of(None, None, Path::new(EXE)).is_enabled);
        let command = command_of(Path::new(EXE));
        let off = approved_mark(false, 1_761_600_000);
        assert!(!status_of(Some(&command), Some(&off), Path::new(EXE)).is_enabled);
        let on = approved_mark(true, 1_761_600_000);
        assert!(status_of(Some(&command), Some(&on), Path::new(EXE)).is_enabled);
    }

    #[test]
    fn a_value_of_another_copy_names_it() {
        let status = status_of(
            Some(r#""C:\Old\vitals.exe" --autostart"#),
            None,
            Path::new(EXE),
        );
        assert!(status.is_enabled);
        assert_eq!(status.other_copy.as_deref(), Some(r"C:\Old\vitals.exe"));
    }

    #[test]
    fn only_the_first_start_on_the_old_pc_turns_it_on() {
        assert!(turns_on_at_first_start(false, true));
        assert!(!turns_on_at_first_start(false, false));
        assert!(!turns_on_at_first_start(true, true));
    }
}
