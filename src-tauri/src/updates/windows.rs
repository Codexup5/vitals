//! The Windows side of updates: the programs of "Apps & features" and the firmware, from the registry.
#![allow(
    unsafe_code,
    reason = "registry calls of the updates screen live in this file"
)]

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_SAM_FLAGS, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
};
use windows::core::{HSTRING, PWSTR};

use super::{InstalledProgram, program_folder};

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const BIOS: &str = r"HARDWARE\DESCRIPTION\System\BIOS";

/// An open registry key, closed when dropped.
struct Key(HKEY);

impl Key {
    fn open(root: HKEY, path: &str, view: REG_SAM_FLAGS) -> Option<Self> {
        let mut key = HKEY::default();
        // SAFETY: the path is a live null-terminated string and `key` a valid out pointer.
        let opened = unsafe {
            RegOpenKeyExW(
                root,
                &HSTRING::from(path),
                None,
                KEY_READ | view,
                &raw mut key,
            )
        };
        (opened == ERROR_SUCCESS).then_some(Self(key))
    }

    /// The names of the subkeys.
    fn subkeys(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut buffer = [0u16; 256];
        for index in 0.. {
            let mut length = u32::try_from(buffer.len()).unwrap_or(0);
            // SAFETY: `buffer` holds `length` u16 and outlives the call; the optional outputs are not asked for.
            let read = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    Some(PWSTR(buffer.as_mut_ptr())),
                    &raw mut length,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if read != ERROR_SUCCESS {
                break;
            }
            let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
            names.push(String::from_utf16_lossy(&buffer[..length]));
        }
        names
    }

    /// A string value of a subkey (`""` for the key itself).
    fn text(&self, subkey: &str, value: &str) -> Option<String> {
        let mut buffer = [0u16; 1024];
        let mut size = u32::try_from(size_of_val(&buffer)).unwrap_or(0);
        // SAFETY: `buffer` holds `size` bytes and outlives the call; RRF_RT_REG_SZ makes the value a terminated string.
        let read = unsafe {
            RegGetValueW(
                self.0,
                &HSTRING::from(subkey),
                &HSTRING::from(value),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&raw mut size),
            )
        };
        if read != ERROR_SUCCESS {
            return None;
        }
        let units = (usize::try_from(size).unwrap_or(0) / 2).min(buffer.len());
        let text = String::from_utf16_lossy(&buffer[..units]);
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_owned())
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the key was opened by RegOpenKeyExW and is closed once, here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

/// The programs of "Apps & features" with a folder of their own: machine-wide 64- and 32-bit, and the user's.
pub fn installed_programs() -> Vec<InstalledProgram> {
    let views = [
        (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, REG_SAM_FLAGS(0)),
    ];
    views
        .into_iter()
        .filter_map(|(root, view)| Key::open(root, UNINSTALL, view))
        .flat_map(|key| {
            key.subkeys()
                .into_iter()
                .filter_map(|subkey| {
                    let name = key.text(&subkey, "DisplayName")?;
                    let folder = program_folder(
                        key.text(&subkey, "InstallLocation").as_deref(),
                        key.text(&subkey, "DisplayIcon").as_deref(),
                    )?;
                    Some(InstalledProgram { name, folder })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The board and the version and date of its firmware, as Windows read them at start.
pub fn firmware() -> vitals_ipc::Firmware {
    let key = Key::open(HKEY_LOCAL_MACHINE, BIOS, REG_SAM_FLAGS(0));
    let text = |value: &str| key.as_ref().and_then(|key| key.text("", value));
    let board = match (text("BaseBoardManufacturer"), text("BaseBoardProduct")) {
        (Some(maker), Some(product)) => Some(format!("{maker} {product}")),
        (maker, product) => maker.or(product),
    };
    vitals_ipc::Firmware {
        board,
        version: text("BIOSVersion"),
        released: text("BIOSReleaseDate").and_then(|date| super::iso_date(&date)),
    }
}
