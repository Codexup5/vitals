//! The Windows side of the optimizer: registry values of both hives, `SPI_SETMOUSE`, the power scheme, the hardware
//! facts, the restore point and the Explorer restart.
#![allow(
    unsafe_code,
    reason = "the Windows calls of the optimizer live in this file"
)]

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use vitals_ipc::{Condition, CpuVendor, HardwareFacts, RestorePoint, TweakId};
use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_ADAPTERINFO, D3DKMT_CLOSEADAPTER, D3DKMT_ENUMADAPTERS2, D3DKMT_QUERYADAPTERINFO,
    D3DKMT_WDDM_2_7_CAPS, D3DKMTCloseAdapter, D3DKMTEnumAdapters2, D3DKMTQueryAdapterInfo,
    KMTQAITYPE_WDDM_2_7_CAPS,
};
use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SERVICE_DISABLED, ERROR_SUCCESS, HLOCAL,
    LocalFree, WIN32_ERROR,
};
use windows::Win32::System::Power::{
    GetPwrCapabilities, GetSystemPowerStatus, PowerGetActiveScheme, PowerReadFriendlyName,
    PowerSetActiveScheme, SYSTEM_POWER_CAPABILITIES, SYSTEM_POWER_STATUS,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_READ, KEY_SET_VALUE,
    REG_DWORD, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE,
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegEnumKeyExW, RegEnumValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::System::Restore::{
    BEGIN_SYSTEM_CHANGE, END_SYSTEM_CHANGE, MODIFY_SETTINGS, RESTOREPOINTINFO_EVENT_TYPE,
    RESTOREPOINTINFOW, SRSetRestorePointW, STATEMGRSTATUS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_SETMOUSE, SPIF_SENDCHANGE, SystemParametersInfoW,
};
use windows::core::{GUID, HSTRING, PWSTR};
use wmi::{COMLibrary, WMIConnection, WMIDateTime};

use super::apply::{Elevation, Machine};
use super::elevated::{AdminMachine, ElevatedReport, ElevatedRequest};
use super::plan::HIGH_PERFORMANCE;
use super::{Data, Hive, KeyAbsent, Location, Put, Reader, RegWrite};

const PROCESSOR: &str = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0";
const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const MOUSE: &str = r"Control Panel\Mouse";
const RESTORE_NAMESPACE: &str = r"root\default";
const RESTORE_DESCRIPTION: &str = "Vitals: optimization";

/// An open registry key, closed when dropped.
struct Key(HKEY);

impl Key {
    fn open(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Result<Self, WIN32_ERROR> {
        let mut key = HKEY::default();
        // SAFETY: the path is a live null-terminated string and `key` a valid out pointer.
        let opened =
            unsafe { RegOpenKeyExW(root, &HSTRING::from(path), None, access, &raw mut key) };
        if opened == ERROR_SUCCESS {
            Ok(Self(key))
        } else {
            Err(opened)
        }
    }

    fn create(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Result<Self, String> {
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
            Err(format!("{path}: error {}", created.0))
        }
    }

    /// The type and bytes of a value; `None` when it is not there.
    fn query(&self, name: &str) -> Result<Option<(REG_VALUE_TYPE, Vec<u8>)>, String> {
        let name = HSTRING::from(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0u32;
        // SAFETY: only the type and the size are asked for; both are valid out pointers.
        let sized = unsafe {
            RegQueryValueExW(
                self.0,
                &name,
                None,
                Some(&raw mut kind),
                None,
                Some(&raw mut size),
            )
        };
        if sized == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if sized != ERROR_SUCCESS {
            return Err(format!("error {}", sized.0));
        }
        let mut bytes = vec![0u8; usize::try_from(size).unwrap_or(0)];
        // SAFETY: `bytes` holds `size` bytes and outlives the call.
        let read = unsafe {
            RegQueryValueExW(
                self.0,
                &name,
                None,
                Some(&raw mut kind),
                Some(bytes.as_mut_ptr()),
                Some(&raw mut size),
            )
        };
        if read != ERROR_SUCCESS {
            return Err(format!("error {}", read.0));
        }
        bytes.truncate(usize::try_from(size).unwrap_or(0));
        Ok(Some((kind, bytes)))
    }

    fn set(&self, name: &str, data: &Data) -> Result<(), String> {
        let (kind, bytes): (REG_VALUE_TYPE, Vec<u8>) = match data {
            Data::Dword(value) => (REG_DWORD, value.to_le_bytes().to_vec()),
            Data::Text(text) => (
                REG_SZ,
                text.encode_utf16()
                    .chain(std::iter::once(0))
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            ),
        };
        // SAFETY: the key is open for writing and `bytes` outlives the call.
        let written =
            unsafe { RegSetValueExW(self.0, &HSTRING::from(name), None, kind, Some(&bytes)) };
        if written == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("{name}: error {}", written.0))
        }
    }

    /// Removes a value; one that is not there is already removed.
    fn delete_value(&self, name: &str) -> Result<(), String> {
        // SAFETY: the key is open for writing and the name is a live null-terminated string.
        let deleted = unsafe { RegDeleteValueW(self.0, &HSTRING::from(name)) };
        if deleted == ERROR_SUCCESS || deleted == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("{name}: error {}", deleted.0))
        }
    }

    /// The names of the key's values; the default value is the empty name. `Err` when the listing broke off.
    fn value_names(&self) -> Result<Vec<String>, WIN32_ERROR> {
        let mut names = Vec::new();
        let mut buffer = vec![0u16; 16_384];
        for index in 0.. {
            let mut length = u32::try_from(buffer.len()).unwrap_or(0);
            // SAFETY: `buffer` holds `length` u16 and outlives the call; the type and data are not asked for.
            let read = unsafe {
                RegEnumValueW(
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
            if read == ERROR_NO_MORE_ITEMS {
                break;
            }
            if read != ERROR_SUCCESS {
                return Err(read);
            }
            let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
            names.push(String::from_utf16_lossy(&buffer[..length]));
        }
        Ok(names)
    }

    /// `Err` when the listing broke off.
    fn subkeys(&self) -> Result<Vec<String>, WIN32_ERROR> {
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
            if read == ERROR_NO_MORE_ITEMS {
                break;
            }
            if read != ERROR_SUCCESS {
                return Err(read);
            }
            let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
            names.push(String::from_utf16_lossy(&buffer[..length]));
        }
        Ok(names)
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the key was opened or created above and is closed once, here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

const fn root_of(hive: Hive) -> HKEY {
    match hive {
        Hive::User => HKEY_CURRENT_USER,
        Hive::Machine => HKEY_LOCAL_MACHINE,
    }
}

/// The catalog writes `REG_DWORD` and `REG_SZ`; `REG_EXPAND_SZ` reads as text.
fn data_of(kind: REG_VALUE_TYPE, bytes: &[u8]) -> Option<Data> {
    if kind == REG_DWORD {
        let bytes: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
        return Some(Data::Dword(u32::from_le_bytes(bytes)));
    }
    if kind == REG_SZ || kind == REG_EXPAND_SZ {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        return Some(Data::Text(
            String::from_utf16_lossy(&units)
                .trim_end_matches('\0')
                .to_owned(),
        ));
    }
    None
}

/// The first key of `path` that does not exist: the one a write would create.
fn first_missing(root: HKEY, path: &str) -> String {
    let parts: Vec<&str> = path.split('\\').collect();
    (1..=parts.len())
        .map(|count| parts[..count].join("\\"))
        .find(|prefix| {
            matches!(Key::open(root, prefix, KEY_READ), Err(error) if error == ERROR_FILE_NOT_FOUND)
        })
        .unwrap_or_else(|| path.to_owned())
}

/// A value of a key from the root of a hive. A value of a type the catalog does not write reads as absent; a write
/// over it is refused, so it is never lost.
pub fn read(hive: Hive, key: &str, name: &str) -> Result<Option<Data>, KeyAbsent> {
    let root = root_of(hive);
    match Key::open(root, key, KEY_QUERY_VALUE) {
        Ok(open) => Ok(open
            .query(name)
            .ok()
            .flatten()
            .and_then(|(kind, bytes)| data_of(kind, &bytes))),
        Err(error) if error == ERROR_FILE_NOT_FOUND => Err(KeyAbsent {
            created: first_missing(root, key),
        }),
        Err(_) => Ok(None),
    }
}

fn is_on_path(key: &str, value_key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let value_key = value_key.to_ascii_lowercase();
    value_key == key || value_key.starts_with(&format!("{key}\\"))
}

/// The keys from `created` down to `value_key` hold nothing but that path, and the last one nothing but `name`.
fn holds_only_path(root: HKEY, created: &str, value_key: &str, name: &str) -> bool {
    let rest = value_key.get(created.len()..).unwrap_or_default();
    let mut below = rest.split('\\').filter(|part| !part.is_empty());
    let mut current = created.to_owned();
    loop {
        let Ok(key) = Key::open(root, &current, KEY_READ) else {
            return false;
        };
        // A listing that broke off says nothing: the key is kept
        let (Ok(subkeys), Ok(values)) = (key.subkeys(), key.value_names()) else {
            return false;
        };
        let Some(next) = below.next() else {
            return subkeys.is_empty()
                && values.iter().all(|value| value.eq_ignore_ascii_case(name));
        };
        let is_path_only =
            values.is_empty() && subkeys.len() == 1 && subkeys[0].eq_ignore_ascii_case(next);
        if !is_path_only {
            return false;
        }
        current = format!("{current}\\{next}");
    }
}

fn delete_value(root: HKEY, key: &str, name: &str) -> Result<(), String> {
    match Key::open(root, key, KEY_SET_VALUE) {
        Ok(open) => open.delete_value(name),
        Err(error) if error == ERROR_FILE_NOT_FOUND => Ok(()),
        Err(error) => Err(format!("{key}: error {}", error.0)),
    }
}

/// One registry write; a key is deleted only when it holds nothing but the path to the value the write names,
/// else only that value goes.
pub fn put(write: &RegWrite) -> Result<(), String> {
    let root = root_of(write.hive);
    match &write.put {
        Put::Data { data } => {
            let key = Key::create(root, &write.key, KEY_QUERY_VALUE | KEY_SET_VALUE)?;
            if let Ok(Some((kind, bytes))) = key.query(&write.name)
                && data_of(kind, &bytes).is_none()
            {
                return Err(format!(
                    "{}: a value of another type is there, it is left as it is",
                    write.name
                ));
            }
            key.set(&write.name, data)
        }
        Put::DeleteValue => delete_value(root, &write.key, &write.name),
        Put::DeleteKey { key } => {
            if !is_on_path(key, &write.key) {
                return Err(format!("{key} is not on the path of {}", write.key));
            }
            if !holds_only_path(root, key, &write.key, &write.name) {
                return delete_value(root, &write.key, &write.name);
            }
            // SAFETY: the path is a live null-terminated string; the key holds only what the change created.
            let deleted = unsafe { RegDeleteTreeW(root, &HSTRING::from(key.as_str())) };
            if deleted == ERROR_SUCCESS || deleted == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!("{key}: error {}", deleted.0))
            }
        }
    }
}

fn guid_text(guid: &GUID) -> String {
    format!("{guid:?}").to_ascii_lowercase()
}

fn active_scheme() -> Option<String> {
    let mut active: *mut GUID = std::ptr::null_mut();
    // SAFETY: `active` is a valid out pointer; Windows allocates the GUID, freed below.
    let read = unsafe { PowerGetActiveScheme(None, &raw mut active) };
    if read != ERROR_SUCCESS || active.is_null() {
        return None;
    }
    // SAFETY: the call succeeded, so `active` points to a GUID Windows allocated with LocalAlloc.
    let guid = unsafe { *active };
    // SAFETY: freed once, as PowerGetActiveScheme asks.
    let _ = unsafe { LocalFree(Some(HLOCAL(active.cast()))) };
    Some(guid_text(&guid))
}

fn friendly_name(guid: &str) -> Option<String> {
    let guid = GUID::try_from(guid).ok()?;
    let mut size = 0u32;
    // SAFETY: the first call only asks the size of the name.
    let sized = unsafe {
        PowerReadFriendlyName(None, Some(&raw const guid), None, None, None, &raw mut size)
    };
    if sized != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut bytes = vec![0u8; usize::try_from(size).ok()?];
    // SAFETY: `bytes` holds `size` bytes and outlives the call.
    let read = unsafe {
        PowerReadFriendlyName(
            None,
            Some(&raw const guid),
            None,
            None,
            Some(bytes.as_mut_ptr()),
            &raw mut size,
        )
    };
    if read != ERROR_SUCCESS {
        return None;
    }
    match data_of(REG_SZ, &bytes) {
        Some(Data::Text(name)) if !name.is_empty() => Some(name),
        _ => None,
    }
}

fn set_active_scheme(guid: &str) -> Result<(), String> {
    let parsed = GUID::try_from(guid).map_err(|error| error.to_string())?;
    // SAFETY: `parsed` is a live GUID for the duration of the call.
    let set = unsafe { PowerSetActiveScheme(None, Some(&raw const parsed)) };
    if set == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("power scheme {guid}: error {}", set.0))
    }
}

/// The three `Control Panel\Mouse` values in force now, so the pointer follows the registry without a sign-in.
fn refresh_mouse() {
    let number = |name| match read(Hive::User, MOUSE, name) {
        Ok(Some(Data::Text(text))) => text.trim().parse::<i32>().unwrap_or(0),
        _ => 0,
    };
    let mut values = [
        number("MouseThreshold1"),
        number("MouseThreshold2"),
        number("MouseSpeed"),
    ];
    // SAFETY: SPI_SETMOUSE reads an array of three ints, which `values` is and outlives the call.
    let set = unsafe {
        SystemParametersInfoW(
            SPI_SETMOUSE,
            0,
            Some(values.as_mut_ptr().cast()),
            SPIF_SENDCHANGE,
        )
    };
    if let Err(error) = set {
        tracing::warn!("SPI_SETMOUSE: {error}");
    }
}

/// A graphics adapter reports hardware-accelerated GPU scheduling: the check Settings → Graphics makes.
fn hags_supported() -> bool {
    let mut enumerate = D3DKMT_ENUMADAPTERS2::default();
    // SAFETY: with no array the call only counts the adapters.
    if unsafe { D3DKMTEnumAdapters2(&raw mut enumerate) }.is_err() {
        return false;
    }
    let mut adapters =
        vec![D3DKMT_ADAPTERINFO::default(); usize::try_from(enumerate.NumAdapters).unwrap_or(0)];
    enumerate.pAdapters = adapters.as_mut_ptr();
    // SAFETY: `adapters` holds `NumAdapters` entries and outlives the call.
    if unsafe { D3DKMTEnumAdapters2(&raw mut enumerate) }.is_err() {
        return false;
    }
    adapters.truncate(usize::try_from(enumerate.NumAdapters).unwrap_or(0));
    let is_supported = adapters.iter().any(|adapter| {
        let mut caps = D3DKMT_WDDM_2_7_CAPS::default();
        let mut query = D3DKMT_QUERYADAPTERINFO {
            hAdapter: adapter.hAdapter,
            Type: KMTQAITYPE_WDDM_2_7_CAPS,
            pPrivateDriverData: (&raw mut caps).cast(),
            PrivateDriverDataSize: u32::try_from(size_of::<D3DKMT_WDDM_2_7_CAPS>()).unwrap_or(0),
        };
        // SAFETY: `caps` is the buffer the query type names, of the size passed.
        let is_queried = unsafe { D3DKMTQueryAdapterInfo(&raw mut query) }.is_ok();
        // SAFETY: every bit pattern of the union is a valid u32; bit 0 is HwSchSupported.
        is_queried && unsafe { caps.Anonymous.Value } & 1 == 1
    });
    for adapter in &adapters {
        let close = D3DKMT_CLOSEADAPTER {
            hAdapter: adapter.hAdapter,
        };
        // SAFETY: each handle was opened by D3DKMTEnumAdapters2 and is closed once.
        let _ = unsafe { D3DKMTCloseAdapter(&raw const close) };
    }
    is_supported
}

fn is_laptop() -> bool {
    const NO_SYSTEM_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid out structure.
    if unsafe { GetSystemPowerStatus(&raw mut status) }.is_err() {
        return false;
    }
    status.BatteryFlag & NO_SYSTEM_BATTERY == 0 && status.BatteryFlag != UNKNOWN
}

fn has_hibernation() -> bool {
    let mut capabilities = SYSTEM_POWER_CAPABILITIES::default();
    // SAFETY: `capabilities` is a valid out structure.
    let is_read = unsafe { GetPwrCapabilities(&raw mut capabilities) };
    is_read && capabilities.HiberFilePresent
}

/// What the optimizer knows of the machine, read without admin rights.
pub fn facts() -> HardwareFacts {
    let text = |key, name| match read(Hive::Machine, key, name) {
        Ok(Some(Data::Text(text))) => text,
        _ => String::new(),
    };
    let vendor = text(PROCESSOR, "VendorIdentifier");
    let cpu_vendor = if vendor.contains("AMD") {
        CpuVendor::Amd
    } else if vendor.contains("Intel") {
        CpuVendor::Intel
    } else {
        CpuVendor::Other
    };
    let mut met = Vec::new();
    if hags_supported() {
        met.push(Condition::HagsSupported);
    }
    if has_hibernation() {
        met.push(Condition::Hibernation);
    }
    if friendly_name(HIGH_PERFORMANCE).is_some() {
        met.push(Condition::HighPerformanceScheme);
    }
    HardwareFacts {
        cpu_vendor,
        is_laptop: is_laptop(),
        met,
        edition: text(CURRENT_VERSION, "EditionID"),
        build: text(CURRENT_VERSION, "CurrentBuildNumber")
            .parse()
            .unwrap_or(0),
    }
}

/// The optimizer of the normal process: HKCU and the power scheme here, HKLM through `--elevated`.
pub struct Windows {
    pub work_dir: PathBuf,
}

impl Reader for Windows {
    fn value(&self, at: &Location) -> Result<Option<Data>, KeyAbsent> {
        read(at.hive, at.key, at.name)
    }

    fn scheme(&self) -> Option<String> {
        active_scheme()
    }

    fn scheme_name(&self, guid: &str) -> Option<String> {
        friendly_name(guid)
    }
}

impl Machine for Windows {
    fn write(&mut self, write: &RegWrite) -> Result<(), String> {
        if write.hive != Hive::User {
            return Err("the normal process writes HKCU only".to_owned());
        }
        put(write)
    }

    fn set_scheme(&mut self, guid: &str) -> Result<(), String> {
        set_active_scheme(guid)
    }

    fn elevate(&mut self, request: &ElevatedRequest) -> Result<ElevatedReport, Elevation> {
        use crate::elevated::{ElevatedError, run};
        match run::<_, Result<ElevatedReport, String>>(
            super::elevated::ACTION,
            request,
            &self.work_dir,
        ) {
            Ok(Ok(report)) => Ok(report),
            Ok(Err(message)) => Err(Elevation::Failed(message)),
            Err(ElevatedError::Declined) => Err(Elevation::Declined),
            Err(ElevatedError::Unknown(message)) => Err(Elevation::Unknown(message)),
            Err(error) => Err(Elevation::Failed(error.to_string())),
        }
    }

    fn refresh(&mut self, id: TweakId) {
        if id == TweakId::PointerPrecisionOff {
            refresh_mouse();
        }
    }
}

/// A restore point as `root\default` lists it; reading the list needs admin rights.
#[derive(Debug, Deserialize)]
#[serde(rename = "SystemRestore", rename_all = "PascalCase")]
struct SystemRestore {
    sequence_number: u32,
    creation_time: WMIDateTime,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct EnableIn {
    drive: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EnableOut {
    return_value: u32,
}

fn restore_connection() -> Option<WMIConnection> {
    COMLibrary::new()
        .and_then(|com| WMIConnection::with_namespace_path(RESTORE_NAMESPACE, com))
        .map_err(|error| tracing::warn!("restore point WMI: {error}"))
        .ok()
}

/// The newest point: its sequence number and Unix time.
fn newest_point(connection: &WMIConnection) -> Option<(u32, i64)> {
    let points: Vec<SystemRestore> = connection.query().ok()?;
    points
        .into_iter()
        .max_by_key(|point| point.sequence_number)
        .map(|point| (point.sequence_number, point.creation_time.0.timestamp()))
}

fn set_point(event: RESTOREPOINTINFO_EVENT_TYPE, sequence: i64) -> Result<i64, WIN32_ERROR> {
    let mut info = RESTOREPOINTINFOW {
        dwEventType: event,
        dwRestorePtType: MODIFY_SETTINGS,
        llSequenceNumber: sequence,
        ..RESTOREPOINTINFOW::default()
    };
    let mut description = info.szDescription;
    for (slot, unit) in description
        .iter_mut()
        .zip(RESTORE_DESCRIPTION.encode_utf16())
    {
        *slot = unit;
    }
    info.szDescription = description;
    let mut status = STATEMGRSTATUS::default();
    // SAFETY: `info` is a filled RESTOREPOINTINFOW with a terminated description, `status` a valid out structure.
    let is_made = unsafe { SRSetRestorePointW(&raw const info, &raw mut status) }.as_bool();
    // The structure is packed: copy the fields out before reading them
    let (code, number) = (status.nStatus, status.llSequenceNumber);
    if is_made { Ok(number) } else { Err(code) }
}

/// Turns System Protection on for C: through WMI.
fn enable_protection(connection: Option<&WMIConnection>) -> Result<(), String> {
    let connection = connection.ok_or("System Protection: WMI is not available")?;
    let out: EnableOut = connection
        .exec_class_method::<SystemRestore, _>(
            "Enable",
            EnableIn {
                drive: r"C:\".to_owned(),
            },
        )
        .map_err(|error| error.to_string())?;
    if out.return_value == 0 {
        Ok(())
    } else {
        Err(format!("System Protection: error {}", out.return_value))
    }
}

/// The admin side: HKLM writes and the restore point.
pub struct Admin;

impl AdminMachine for Admin {
    fn write(&mut self, write: &RegWrite) -> Result<(), String> {
        if write.hive != Hive::Machine {
            return Err("the elevated side never writes HKCU".to_owned());
        }
        put(write)
    }

    fn restore_point(&mut self, should_enable: bool) -> RestorePoint {
        let connection = restore_connection();
        let before = connection.as_ref().and_then(newest_point);
        let mut made = set_point(BEGIN_SYSTEM_CHANGE, 0);
        if matches!(made, Err(code) if code == ERROR_SERVICE_DISABLED) {
            if !should_enable {
                return RestorePoint::ProtectionOff;
            }
            if let Err(message) = enable_protection(connection.as_ref()) {
                return RestorePoint::Failed { message };
            }
            made = set_point(BEGIN_SYSTEM_CHANGE, 0);
        }
        let sequence = match made {
            Ok(sequence) => sequence,
            Err(code) => {
                return RestorePoint::Failed {
                    message: format!("restore point: error {}", code.0),
                };
            }
        };
        let _ = set_point(END_SYSTEM_CHANGE, sequence);
        // Windows makes at most one point a day: the list before and after tells which happened
        let after = connection.as_ref().and_then(newest_point);
        match (before, after) {
            (before, Some((number, time))) if before.is_none_or(|(old, _)| number > old) => {
                RestorePoint::Created { time }
            }
            (_, Some((_, time))) => RestorePoint::Kept { time },
            (_, None) => RestorePoint::Created {
                time: chrono::Utc::now().timestamp(),
            },
        }
    }
}

/// Restarts the shell so Explorer's settings show: `taskkill` ends it and Windows starts it again; if it does not
/// come back, it is started here.
pub fn restart_explorer() -> Result<(), String> {
    let windows_dir =
        std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let explorer = windows_dir.join("explorer.exe");
    crate::process::output(
        "taskkill.exe",
        &["/f", "/im", "explorer.exe"],
        Duration::from_secs(10),
    )
    .map_err(|error| error.to_string())?;
    std::thread::sleep(Duration::from_secs(2));
    let is_back = crate::system::windows::running_exes().iter().any(|exe| {
        exe.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("explorer.exe"))
    });
    if !is_back {
        std::process::Command::new(&explorer)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use vitals_ipc::TweakId;

    use super::super::Put;
    use super::super::plan::BALANCED;
    use super::*;

    #[test]
    fn a_guid_reads_as_powercfg_writes_it() {
        let guid = GUID::try_from(BALANCED).unwrap();
        assert_eq!(guid_text(&guid), BALANCED);
    }

    /// Deletes the scratch key even when an assertion fails.
    struct Scratch(String);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = put(&RegWrite {
                id: TweakId::GameMode,
                hive: Hive::User,
                key: self.0.clone(),
                name: String::new(),
                put: Put::DeleteKey {
                    key: self.0.clone(),
                },
            });
        }
    }

    fn write(key: &str, name: &str, put: Put) -> RegWrite {
        RegWrite {
            id: TweakId::GameMode,
            hive: Hive::User,
            key: key.to_owned(),
            name: name.to_owned(),
            put,
        }
    }

    #[test]
    fn a_dword_a_string_and_an_absent_value_round_trip_in_a_scratch_key() {
        let root = format!(r"Software\Vitals\test-{}", std::process::id());
        let key = format!(r"{root}\inner");
        // Software\Vitals may not exist yet: the first missing key is the one a write creates and a revert deletes
        let absent = read(Hive::User, &key, "Mode");
        let Err(KeyAbsent { created }) = absent else {
            panic!("{absent:?}: the scratch key exists");
        };
        assert!(
            root.to_ascii_lowercase()
                .starts_with(&created.to_ascii_lowercase()),
            "{created} is not on the path of {root}"
        );
        let scratch = Scratch(created.clone());

        put(&write(
            &key,
            "Mode",
            Put::Data {
                data: Data::Dword(2),
            },
        ))
        .unwrap();
        assert_eq!(read(Hive::User, &key, "Mode"), Ok(Some(Data::Dword(2))));
        put(&write(
            &key,
            "Speed",
            Put::Data {
                data: Data::Text("0".to_owned()),
            },
        ))
        .unwrap();
        assert_eq!(
            read(Hive::User, &key, "Speed"),
            Ok(Some(Data::Text("0".to_owned())))
        );
        put(&write(
            &key,
            "",
            Put::Data {
                data: Data::Text(String::new()),
            },
        ))
        .unwrap();
        assert_eq!(
            read(Hive::User, &key, ""),
            Ok(Some(Data::Text(String::new())))
        );
        assert_eq!(read(Hive::User, &key, "Gone"), Ok(None));

        put(&write(&key, "Mode", Put::DeleteValue)).unwrap();
        assert_eq!(read(Hive::User, &key, "Mode"), Ok(None));
        put(&write(&key, "Mode", Put::DeleteValue)).unwrap();

        // The key holds a value the change did not write: it is kept, the named value goes
        put(&write(
            &key,
            "Speed",
            Put::DeleteKey {
                key: created.clone(),
            },
        ))
        .unwrap();
        assert_eq!(read(Hive::User, &key, "Speed"), Ok(None));
        assert_eq!(
            read(Hive::User, &key, ""),
            Ok(Some(Data::Text(String::new())))
        );

        put(&write(
            &key,
            "",
            Put::DeleteKey {
                key: created.clone(),
            },
        ))
        .unwrap();
        assert!(matches!(read(Hive::User, &key, ""), Err(KeyAbsent { .. })));
        assert!(matches!(
            read(Hive::User, &created, ""),
            Err(KeyAbsent { .. })
        ));
        drop(scratch);
    }
}
