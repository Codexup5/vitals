//! The Windows side of startup: the Run keys, the `StartupApproved` marks, and the SID of this user from the profile
//! list, which names the trace files.
#![allow(
    unsafe_code,
    reason = "registry calls of the startup screen live in this file"
)]

use std::path::Path;

use vitals_ipc::{StartupOverview, StartupSetArgs, StartupSource};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_READ, KEY_SET_VALUE,
    KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_BINARY, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE,
    REG_SAM_FLAGS, REG_SZ, RRF_NOEXPAND, RRF_RT_REG_BINARY, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegEnumKeyExW, RegEnumValueW, RegGetValueW, RegOpenKeyExW,
    RegSetValueExW,
};
use windows::core::{HSTRING, PWSTR};

use super::{Facts, Listed, StartupError, Trace, impact};
use crate::trust;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";
const PROFILES: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList";

/// An open registry key, closed when dropped.
struct Key(HKEY);

impl Key {
    fn open(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Option<Self> {
        let mut key = HKEY::default();
        // SAFETY: the path is a live null-terminated string and `key` a valid out pointer.
        let opened =
            unsafe { RegOpenKeyExW(root, &HSTRING::from(path), None, access, &raw mut key) };
        (opened == ERROR_SUCCESS).then_some(Self(key))
    }

    fn create(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Result<Self, StartupError> {
        let mut key = HKEY::default();
        // SAFETY: the path is a live null-terminated string and `key` a valid out pointer; no class nor security.
        let created = unsafe {
            RegCreateKeyExW(
                root,
                &HSTRING::from(path),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                access,
                None,
                &raw mut key,
                None,
            )
        };
        if created == ERROR_SUCCESS {
            Ok(Self(key))
        } else {
            Err(StartupError::Registry(format!("{path}: {}", created.0)))
        }
    }

    /// The string values of the key, a `REG_EXPAND_SZ` one left unexpanded.
    fn strings(&self) -> Vec<(String, String)> {
        let mut values = Vec::new();
        let mut name = vec![0u16; 16_384];
        let mut data = vec![0u8; 65_536];
        for index in 0.. {
            let mut name_length = u32::try_from(name.len()).unwrap_or(0);
            let mut data_length = u32::try_from(data.len()).unwrap_or(0);
            let mut kind = 0u32;
            // SAFETY: both buffers hold the lengths passed and outlive the call.
            let read = unsafe {
                RegEnumValueW(
                    self.0,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &raw mut name_length,
                    None,
                    Some(&raw mut kind),
                    Some(data.as_mut_ptr()),
                    Some(&raw mut data_length),
                )
            };
            if read != ERROR_SUCCESS {
                break;
            }
            if kind != REG_SZ.0 && kind != REG_EXPAND_SZ.0 {
                continue;
            }
            let name_length = usize::try_from(name_length).unwrap_or(0).min(name.len());
            let units: Vec<u16> = data[..usize::try_from(data_length).unwrap_or(0).min(data.len())]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            let text = String::from_utf16_lossy(&units);
            values.push((
                String::from_utf16_lossy(&name[..name_length]),
                text.trim_end_matches('\0').trim().to_owned(),
            ));
        }
        values
    }

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

    /// A value of a subkey as bytes, of the types `flags` allows.
    fn raw(&self, subkey: &str, value: &str, flags: u32) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; 4096];
        let mut size = u32::try_from(buffer.len()).unwrap_or(0);
        // SAFETY: `buffer` holds `size` bytes and outlives the call.
        let read = unsafe {
            RegGetValueW(
                self.0,
                &HSTRING::from(subkey),
                &HSTRING::from(value),
                windows::Win32::System::Registry::REG_ROUTINE_FLAGS(flags),
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&raw mut size),
            )
        };
        if read != ERROR_SUCCESS {
            return None;
        }
        buffer.truncate(usize::try_from(size).unwrap_or(0));
        Some(buffer)
    }

    fn set_binary(&self, value: &str, bytes: &[u8]) -> Result<(), StartupError> {
        // SAFETY: the key is open for writing and `bytes` outlives the call.
        let written =
            unsafe { RegSetValueExW(self.0, &HSTRING::from(value), None, REG_BINARY, Some(bytes)) };
        if written == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(StartupError::Registry(format!("{value}: {}", written.0)))
        }
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the key was opened or created above and is closed once, here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn hive_of(source: StartupSource) -> HKEY {
    if source.is_machine() {
        HKEY_LOCAL_MACHINE
    } else {
        HKEY_CURRENT_USER
    }
}

/// The values of a Run key; empty for a folder source.
fn listed_in_registry(source: StartupSource) -> Vec<Listed> {
    let view = match source {
        StartupSource::UserRun | StartupSource::MachineRun => KEY_WOW64_64KEY,
        StartupSource::MachineRun32 => KEY_WOW64_32KEY,
        _ => return Vec::new(),
    };
    let Some(key) = Key::open(hive_of(source), RUN, KEY_READ | view) else {
        return Vec::new();
    };
    key.strings()
        .into_iter()
        .map(|(name, command)| Listed {
            source,
            name,
            command,
            image: None,
        })
        .collect()
}

fn listed_of(source: StartupSource) -> Vec<Listed> {
    match source {
        StartupSource::UserFolder | StartupSource::MachineFolder => super::folder_of(source)
            .map(|folder| super::listed_in_folder(source, &folder))
            .unwrap_or_default(),
        _ => listed_in_registry(source),
    }
}

fn approved(source: StartupSource, name: &str) -> Option<Vec<u8>> {
    let key = Key::open(hive_of(source), APPROVED, KEY_READ | KEY_WOW64_64KEY)?;
    key.raw(super::approved_key(source), name, RRF_RT_REG_BINARY.0)
}

/// The SID of this user: the profile whose folder is `%USERPROFILE%`.
fn user_sid() -> Option<String> {
    let profile = std::env::var("USERPROFILE").ok()?;
    let list = Key::open(HKEY_LOCAL_MACHINE, PROFILES, KEY_READ | KEY_WOW64_64KEY)?;
    list.subkeys().into_iter().find(|sid| {
        list.raw(
            sid,
            "ProfileImagePath",
            RRF_RT_REG_SZ.0 | RRF_RT_REG_EXPAND_SZ.0 | RRF_NOEXPAND.0,
        )
        .map(|bytes| {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            String::from_utf16_lossy(&units)
        })
        .is_some_and(|path| {
            let path = super::expand_env(path.trim_end_matches('\0'), super::env_lookup);
            path.eq_ignore_ascii_case(&profile)
        })
    })
}

fn read_traces(windows_dir: &Path) -> Vec<Trace> {
    let Some(sid) = user_sid() else {
        return Vec::new();
    };
    let dir = windows_dir.join(r"System32\wdi\LogFiles\StartupInfo");
    impact::trace_files(&dir, &sid)
        .into_iter()
        .filter_map(|path| {
            let bytes = std::fs::read(&path).ok()?;
            impact::parse(&impact::decode(&bytes))
                .map_err(|error| tracing::warn!("{} does not parse: {error}", path.display()))
                .ok()
        })
        .collect()
}

fn image_of(listed: &Listed, windows_dir: &Path) -> Option<std::path::PathBuf> {
    listed.image.clone().or_else(|| {
        let command = super::expand_env(&listed.command, super::env_lookup);
        super::image_of(&command, |candidate| {
            super::resolve_candidate(candidate, windows_dir)
        })
    })
}

/// The signature of a program; an alias of a Store app is said to be one, as `WinVerifyTrust` cannot read it.
fn verify(image: &Path) -> trust::Verdict {
    let local = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    if local.is_some_and(|local| trust::is_app_alias(image, &local)) {
        return trust::Verdict {
            state: vitals_ipc::SignatureState::StoreApp,
            signer: None,
        };
    }
    trust::windows::verify(image)
}

/// Every program Windows starts at sign-in, checked: its mark, signature, maker and cost.
pub fn overview() -> StartupOverview {
    let windows_dir = super::windows_dir();
    let traces = read_traces(&windows_dir);
    let mut entries: Vec<_> = StartupSource::ALL
        .into_iter()
        .flat_map(listed_of)
        .map(|listed| {
            let mark = approved(listed.source, &listed.name);
            let image = image_of(&listed, &windows_dir);
            let verdict = image.as_deref().map_or(trust::Verdict::NO_FILE, verify);
            let company = image.as_deref().and_then(trust::windows::company);
            let facts = Facts {
                approved: mark.as_deref(),
                image,
                verdict,
                company,
            };
            super::entry_of(listed, facts, &windows_dir, &traces)
        })
        .collect();
    super::sort_entries(&mut entries);
    StartupOverview {
        entries,
        measured_boots: u32::try_from(traces.len()).unwrap_or(u32::MAX),
    }
}

/// The entry must be in its place now and not a part of Windows; then its mark is written.
pub fn apply(change: &StartupSetArgs, now: i64) -> Result<(), StartupError> {
    let windows_dir = super::windows_dir();
    let listed = listed_of(change.source)
        .into_iter()
        .find(|listed| listed.name == change.name)
        .ok_or_else(|| StartupError::NotFound(change.name.clone()))?;
    if let Some(image) = image_of(&listed, &windows_dir) {
        let verdict = verify(&image);
        if trust::is_part_of_windows(&image, &windows_dir, &verdict) {
            return Err(StartupError::Protected(change.name.clone()));
        }
    }
    let key = Key::create(
        hive_of(change.source),
        &format!(r"{APPROVED}\{}", super::approved_key(change.source)),
        KEY_SET_VALUE | KEY_QUERY_VALUE | KEY_WOW64_64KEY,
    )?;
    key.set_binary(&change.name, &super::approved_mark(change.enable, now))
}
