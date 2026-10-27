//! The named mutex and event of [`super`]: Win32 objects of this user's session, gone when the process ends.
#![allow(
    unsafe_code,
    reason = "the named mutex and event of the single panel live in this file"
)]

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent,
    WaitForSingleObject,
};
use windows::core::HSTRING;

use super::Names;

/// The first panel: it keeps the mutex for as long as it runs and waits on the event.
#[derive(Debug)]
pub struct Instance {
    /// Never closed: the mutex is held until the process ends
    _mutex: isize,
    event: isize,
}

/// The handles are plain kernel handles, valid in any thread of the process.
const fn handle(raw: isize) -> HANDLE {
    HANDLE(raw as *mut core::ffi::c_void)
}

/// Asks the first panel to show its window.
fn signal_first(names: Names) {
    // SAFETY: the name is a live null-terminated string; only the right to set the event is asked for.
    let Ok(event) = (unsafe { OpenEventW(EVENT_MODIFY_STATE, false, &HSTRING::from(names.show)) })
    else {
        return;
    };
    // SAFETY: `event` was just opened with the right to set it, and is closed once.
    unsafe {
        let _ = SetEvent(event);
        let _ = CloseHandle(event);
    }
}

/// The first start of a session gets the instance; a later one shows the first panel and gets none.
pub fn claim() -> Option<Instance> {
    claim_named(super::NAMES)
}

fn claim_named(names: Names) -> Option<Instance> {
    // SAFETY: the name is a live null-terminated string; the mutex is not owned on creation.
    let mutex = unsafe { CreateMutexW(None, false, &HSTRING::from(names.mutex)) }.ok()?;
    // SAFETY: read right after the call that set it.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // SAFETY: the handle of the existing mutex is not needed by a start that exits.
        let _ = unsafe { CloseHandle(mutex) };
        signal_first(names);
        return None;
    }
    // SAFETY: an unnamed security, an auto-reset event that starts unset, its name a live string.
    let event = unsafe { CreateEventW(None, false, false, &HSTRING::from(names.show)) }.ok()?;
    Some(Instance {
        _mutex: mutex.0 as isize,
        event: event.0 as isize,
    })
}

impl Instance {
    /// Calls `show` every time a later start asks, on a thread of its own, for the life of the panel.
    pub fn on_show(self, show: impl Fn() + Send + 'static) {
        let spawned = std::thread::Builder::new()
            .name("instance".to_owned())
            .spawn(move || {
                let instance = self;
                // SAFETY: the event stays open: `instance` lives in this thread, which never ends.
                while unsafe { WaitForSingleObject(handle(instance.event), INFINITE) }
                    == WAIT_OBJECT_0
                {
                    show();
                }
            });
        if let Err(error) = spawned {
            tracing::error!("the instance thread did not start: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_claim_in_the_session_gets_nothing() {
        let names = Names {
            mutex: r"Local\Vitals.Test.Instance",
            show: r"Local\Vitals.Test.Instance.Show",
        };
        let first = claim_named(names).unwrap();
        assert!(claim_named(names).is_none());
        // The second claim set the event: the first one is told
        // SAFETY: the event of `first` is open for the whole test.
        let waited = unsafe { WaitForSingleObject(handle(first.event), 1000) };
        assert_eq!(waited, WAIT_OBJECT_0);
    }
}
