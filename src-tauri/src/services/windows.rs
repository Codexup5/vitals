//! The Windows side of services: the service manager lists them and their configuration, the registry names the DLL
//! of a hosted one, and the start type changes through `ChangeServiceConfigW` (in the elevated process).
#![allow(
    unsafe_code,
    reason = "service manager and registry calls of the services screen live in this file"
)]

use std::collections::HashMap;
use std::path::PathBuf;

use vitals_ipc::{Service, ServiceState, ServicesOverview, StartType};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::System::Services::{
    ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ENUM_SERVICE_STATUS_PROCESSW,
    ENUM_SERVICE_TYPE, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW, QUERY_SERVICE_CONFIGW,
    QueryServiceConfig2W, QueryServiceConfigW, SC_ENUM_PROCESS_INFO, SC_HANDLE, SC_MANAGER_CONNECT,
    SC_MANAGER_ENUMERATE_SERVICE, SERVICE_AUTO_START, SERVICE_CHANGE_CONFIG,
    SERVICE_CONFIG_DELAYED_AUTO_START_INFO, SERVICE_CONFIG_DESCRIPTION,
    SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DEMAND_START, SERVICE_DESCRIPTIONW, SERVICE_DISABLED,
    SERVICE_ERROR, SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_RUNNING, SERVICE_START_TYPE,
    SERVICE_STATE_ALL, SERVICE_STOPPED, SERVICE_WIN32,
};
use windows::core::{BOOL, HSTRING, PCWSTR, PWSTR};

use super::{RawService, ServiceError};
use crate::storage::Storage;
use crate::trust::{self, Verdict};

/// A handle of the service manager or of one service, closed when dropped.
struct Handle(SC_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenSCManagerW or OpenServiceW and is closed once, here.
        let _ = unsafe { CloseServiceHandle(self.0) };
    }
}

fn manager(access: u32) -> Result<Handle, ServiceError> {
    // SAFETY: no machine or database name: the local active database.
    unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), access) }
        .map(Handle)
        .map_err(|error| ServiceError::Manager(error.message()))
}

fn open_service(manager: &Handle, name: &str, access: u32) -> Result<Handle, ServiceError> {
    // SAFETY: the manager handle is open and the name a live null-terminated string.
    unsafe { OpenServiceW(manager.0, &HSTRING::from(name), access) }
        .map(Handle)
        .map_err(|_| ServiceError::NotFound(name.to_owned()))
}

/// A string a pointer of the service manager's buffer points at.
fn text_at(pointer: PWSTR) -> String {
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: the pointer is a null-terminated string inside a buffer that outlives this call.
    unsafe { pointer.to_string() }.unwrap_or_default()
}

/// Asks twice: once for the size, once into a buffer of that size, 8-byte aligned for the structures in it.
fn sized_buffer(mut query: impl FnMut(&mut [u8], &mut u32) -> bool) -> Option<Vec<u64>> {
    let mut needed = 0u32;
    let mut probe = [0u8; 0];
    if query(&mut probe, &mut needed) {
        return Some(Vec::new());
    }
    let mut buffer = vec![0u64; usize::try_from(needed).ok()?.div_ceil(8)];
    // SAFETY: a u64 buffer viewed as bytes: the same memory, its length in bytes.
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(buffer.as_mut_ptr().cast::<u8>(), buffer.len() * 8)
    };
    query(bytes, &mut needed).then_some(buffer)
}

/// The start type, with the delayed flag read from the second configuration.
fn start_of(raw: SERVICE_START_TYPE, is_delayed: bool) -> StartType {
    match raw {
        SERVICE_AUTO_START if is_delayed => StartType::AutomaticDelayed,
        SERVICE_AUTO_START => StartType::Automatic,
        SERVICE_DISABLED => StartType::Disabled,
        _ => StartType::Manual,
    }
}

fn config2(
    service: &Handle,
    level: windows::Win32::System::Services::SERVICE_CONFIG,
) -> Option<Vec<u64>> {
    sized_buffer(|bytes, needed| {
        let slot = (!bytes.is_empty()).then_some(bytes);
        // SAFETY: the service handle is open; the buffer is `bytes.len()` long or absent for the size query.
        unsafe { QueryServiceConfig2W(service.0, level, slot, needed) }.is_ok()
    })
}

fn service_dll(name: &str) -> Option<String> {
    let mut buffer = vec![0u16; 1024];
    let mut size = u32::try_from(buffer.len() * 2).unwrap_or(0);
    let subkey = HSTRING::from(format!(
        r"SYSTEM\CurrentControlSet\Services\{name}\Parameters"
    ));
    // SAFETY: `buffer` holds `size` bytes and outlives the call; the value is a terminated string.
    let read = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &subkey,
            &HSTRING::from("ServiceDll"),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&raw mut size),
        )
    };
    if read != ERROR_SUCCESS {
        return None;
    }
    let units = (usize::try_from(size).unwrap_or(0) / 2).min(buffer.len());
    let end = buffer[..units]
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units);
    let text = String::from_utf16_lossy(&buffer[..end]);
    (!text.trim().is_empty()).then(|| text.trim().to_owned())
}

/// The configuration of one open service.
fn raw_of(
    service: &Handle,
    name: String,
    display_name: String,
    state: ServiceState,
) -> Option<RawService> {
    #[allow(
        clippy::cast_ptr_alignment,
        reason = "sized_buffer hands out the bytes of a u64 buffer: 8-byte aligned"
    )]
    let config = sized_buffer(|bytes, needed| {
        let slot = (!bytes.is_empty()).then(|| bytes.as_mut_ptr().cast::<QUERY_SERVICE_CONFIGW>());
        // SAFETY: the buffer is `bytes.len()` long, 8-byte aligned, or absent for the size query.
        unsafe {
            QueryServiceConfigW(
                service.0,
                slot,
                u32::try_from(bytes.len()).unwrap_or(0),
                needed,
            )
        }
        .is_ok()
    })?;
    // SAFETY: the service manager wrote a QUERY_SERVICE_CONFIGW at the start of the aligned buffer.
    let config = unsafe { &*config.as_ptr().cast::<QUERY_SERVICE_CONFIGW>() };
    let is_delayed = config2(service, SERVICE_CONFIG_DELAYED_AUTO_START_INFO).is_some_and(|info| {
        // SAFETY: the buffer holds the delayed start structure the query asked for.
        unsafe { (*info.as_ptr().cast::<SERVICE_DELAYED_AUTO_START_INFO>()).fDelayedAutostart }
            .as_bool()
    });
    let description = config2(service, SERVICE_CONFIG_DESCRIPTION).and_then(|info| {
        // SAFETY: the buffer holds the description structure; its string points inside the same buffer.
        let text =
            text_at(unsafe { (*info.as_ptr().cast::<SERVICE_DESCRIPTIONW>()).lpDescription });
        (!text.trim().is_empty()).then(|| text.trim().to_owned())
    });
    let binary = text_at(config.lpBinaryPathName);
    let is_hosted = binary.to_ascii_lowercase().contains("svchost.exe");
    Some(RawService {
        service_dll: is_hosted.then(|| service_dll(&name)).flatten(),
        name,
        display_name,
        description,
        start: start_of(config.dwStartType, is_delayed),
        state,
        binary,
    })
}

fn state_of(state: windows::Win32::System::Services::SERVICE_STATUS_CURRENT_STATE) -> ServiceState {
    match state {
        SERVICE_RUNNING => ServiceState::Running,
        SERVICE_STOPPED => ServiceState::Stopped,
        _ => ServiceState::Changing,
    }
}

/// Every Win32 service (drivers are not services a user starts).
fn raw_services() -> Result<Vec<RawService>, ServiceError> {
    let manager = manager(SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE)?;
    let mut count = 0u32;
    let buffer = sized_buffer(|bytes, needed| {
        let slot = (!bytes.is_empty()).then_some(bytes);
        // SAFETY: the manager handle is open; the buffer is `bytes.len()` long or absent for the size query.
        unsafe {
            EnumServicesStatusExW(
                manager.0,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                slot,
                needed,
                &raw mut count,
                None,
                PCWSTR::null(),
            )
        }
        .is_ok()
    })
    .ok_or_else(|| ServiceError::Manager("the services could not be listed".to_owned()))?;
    // SAFETY: the service manager wrote `count` entries at the start of the aligned buffer.
    let entries = unsafe {
        std::slice::from_raw_parts(
            buffer.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>(),
            usize::try_from(count).unwrap_or(0),
        )
    };
    Ok(entries
        .iter()
        .filter_map(|entry| {
            let name = text_at(entry.lpServiceName);
            let service = open_service(&manager, &name, SERVICE_QUERY_CONFIG).ok()?;
            let state = state_of(entry.ServiceStatusProcess.dwCurrentState);
            raw_of(&service, name, text_at(entry.lpDisplayName), state)
        })
        .collect())
}

fn raw_one(name: &str) -> Result<RawService, ServiceError> {
    let manager = manager(SC_MANAGER_CONNECT)?;
    let service = open_service(&manager, name, SERVICE_QUERY_CONFIG)?;
    let display = name.to_owned();
    raw_of(&service, name.to_owned(), display, ServiceState::Changing)
        .ok_or_else(|| ServiceError::NotFound(name.to_owned()))
}

/// The signatures of the files, each file once: many services share `svchost.exe` and some share a DLL.
fn verify_files(images: &[Option<PathBuf>]) -> HashMap<PathBuf, (Verdict, Option<String>)> {
    let mut unique: Vec<&PathBuf> = images.iter().flatten().collect();
    unique.sort();
    unique.dedup();
    std::thread::scope(|scope| {
        let chunk = unique.len().div_ceil(8).max(1);
        let workers: Vec<_> = unique
            .chunks(chunk)
            .map(|paths| {
                scope.spawn(move || {
                    paths
                        .iter()
                        .map(|path| {
                            (
                                (*path).clone(),
                                (trust::windows::verify(path), trust::windows::company(path)),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok())
            .flatten()
            .collect()
    })
}

fn build(raws: Vec<RawService>, changes: &HashMap<String, StartType>) -> Vec<Service> {
    let windows_dir = crate::startup::windows_dir();
    let images: Vec<Option<PathBuf>> = raws
        .iter()
        .map(|raw| super::image_of(raw, &windows_dir))
        .collect();
    let checked = verify_files(&images);
    raws.into_iter()
        .zip(images)
        .map(|(raw, image)| {
            let (verdict, company) = image
                .as_ref()
                .and_then(|image| checked.get(image).cloned())
                .unwrap_or((Verdict::NO_FILE, None));
            let changed_from = changes.get(&raw.name).copied();
            super::service_of(raw, image, verdict, company, changed_from, &windows_dir)
        })
        .collect()
}

pub fn overview(storage: &Storage) -> Result<ServicesOverview, ServiceError> {
    let changes = storage.service_changes()?;
    let mut services = build(raw_services()?, &changes);
    super::sort_services(&mut services);
    Ok(ServicesOverview { services })
}

/// One service as it is now, with what the panel saved of it.
pub fn read_one(name: &str, storage: &Storage) -> Result<Service, ServiceError> {
    let changes = storage.service_changes()?;
    build(vec![raw_one(name)?], &changes)
        .pop()
        .ok_or_else(|| ServiceError::NotFound(name.to_owned()))
}

/// One service as it is now, for the elevated process that has no database of its own.
pub fn read_unsaved(name: &str) -> Result<Service, ServiceError> {
    build(vec![raw_one(name)?], &HashMap::new())
        .pop()
        .ok_or_else(|| ServiceError::NotFound(name.to_owned()))
}

/// Sets the start type; the delayed flag goes with it, so an automatic start is exactly the one asked for.
pub fn set_start(name: &str, start: StartType) -> Result<(), ServiceError> {
    let raw = match start {
        StartType::Automatic | StartType::AutomaticDelayed => SERVICE_AUTO_START,
        StartType::Manual => SERVICE_DEMAND_START,
        StartType::Disabled => return Err(ServiceError::Forbidden),
    };
    let manager = manager(SC_MANAGER_CONNECT)?;
    let service = open_service(&manager, name, SERVICE_CHANGE_CONFIG | SERVICE_QUERY_CONFIG)?;
    let failed = |error: windows::core::Error| ServiceError::Manager(error.message());
    // SAFETY: the service is open for changing its configuration; every other field is left as it is.
    unsafe {
        ChangeServiceConfigW(
            service.0,
            ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
            raw,
            SERVICE_ERROR(SERVICE_NO_CHANGE),
            PCWSTR::null(),
            PCWSTR::null(),
            None,
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
        )
    }
    .map_err(failed)?;
    if start.is_automatic() {
        let info = SERVICE_DELAYED_AUTO_START_INFO {
            fDelayedAutostart: BOOL::from(start == StartType::AutomaticDelayed),
        };
        // SAFETY: the same open service; `info` lives for the call.
        unsafe {
            ChangeServiceConfig2W(
                service.0,
                SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                Some((&raw const info).cast()),
            )
        }
        .map_err(failed)?;
    }
    Ok(())
}
