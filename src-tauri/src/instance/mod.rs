//! One panel at a time. The first start holds a named mutex; a second start finds it, asks the first to show its
//! window through a named event, and exits, so a click on the shortcut never makes a second tray icon. A debug build
//! has names of its own, so it runs next to the installed panel.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::claim;

/// The mutex the first start holds and the event a later start sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Names {
    pub mutex: &'static str,
    pub show: &'static str,
}

pub const NAMES: Names = if cfg!(debug_assertions) {
    Names {
        mutex: r"Local\Vitals.Panel.Debug",
        show: r"Local\Vitals.Panel.Debug.Show",
    }
} else {
    Names {
        mutex: r"Local\Vitals.Panel",
        show: r"Local\Vitals.Panel.Show",
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_live_in_the_session_and_differ() {
        assert!(NAMES.mutex.starts_with(r"Local\") && NAMES.show.starts_with(r"Local\"));
        assert_ne!(NAMES.mutex, NAMES.show);
    }
}
