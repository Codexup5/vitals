//! The Windows side of trust: `WinVerifyTrust` on the file, then on the Windows catalog that holds its hash, and
//! the version resource. Revocation is not checked and no URL is fetched: nothing leaves the PC.
#![allow(
    unsafe_code,
    reason = "WinVerifyTrust, the catalog API and the version resource live in this file"
)]

use std::ffi::c_void;
use std::fmt::Write as _;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use vitals_ipc::SignatureState;
use windows::Win32::Foundation::{HANDLE, HWND, TRUST_E_NOSIGNATURE, TRUST_E_SUBJECT_FORM_UNKNOWN};
use windows::Win32::Security::Cryptography::Catalog::{
    CATALOG_INFO, CryptCATAdminAcquireContext2, CryptCATAdminCalcHashFromFileHandle2,
    CryptCATAdminEnumCatalogFromHash, CryptCATAdminReleaseCatalogContext,
    CryptCATAdminReleaseContext, CryptCATCatalogInfoFromContext,
};
use windows::Win32::Security::Cryptography::{
    BCRYPT_SHA256_ALGORITHM, CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW,
};
use windows::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_CATALOG_INFO, WINTRUST_DATA, WINTRUST_DATA_0,
    WINTRUST_DATA_UNION_CHOICE, WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL,
    WTD_CHOICE_CATALOG, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvSignerFromChain,
    WTHelperProvDataFromStateData, WinVerifyTrust,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::core::{HRESULT, HSTRING, PCWSTR};

use super::Verdict;

/// What one `WinVerifyTrust` call said.
enum Check {
    Verdict(Verdict),
    /// The subject carries no signature of its own
    NoSignature,
}

fn size_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(0)
}

fn wintrust_data(choice: WINTRUST_DATA_UNION_CHOICE, subject: WINTRUST_DATA_0) -> WINTRUST_DATA {
    WINTRUST_DATA {
        cbStruct: size_u32::<WINTRUST_DATA>(),
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: choice,
        Anonymous: subject,
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    }
}

/// Verifies the subject `data` points at, reads the signer, frees the state.
fn verify_with(data: &mut WINTRUST_DATA) -> Check {
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: `data` is fully initialised and the subject it points at outlives both calls.
    let status =
        unsafe { WinVerifyTrust(HWND::default(), &raw mut action, (&raw mut *data).cast()) };
    let signer = signer_of(data.hWVTStateData);
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // SAFETY: the same structure, now asking to free the state the first call kept.
    let _ = unsafe { WinVerifyTrust(HWND::default(), &raw mut action, (&raw mut *data).cast()) };
    let code = HRESULT(status);
    if code == TRUST_E_NOSIGNATURE || code == TRUST_E_SUBJECT_FORM_UNKNOWN {
        return Check::NoSignature;
    }
    Check::Verdict(Verdict {
        state: if status == 0 {
            SignatureState::Valid
        } else {
            SignatureState::Invalid
        },
        signer,
    })
}

/// The simple name of the leaf certificate of the first signer, from the state `WinVerifyTrust` kept.
fn signer_of(state: HANDLE) -> Option<String> {
    if state.is_invalid() {
        return None;
    }
    // SAFETY: `state` is the live state of a verify call; every pointer read below is checked for null and points
    // into structures that live until the close call.
    unsafe {
        let provider = WTHelperProvDataFromStateData(state);
        if provider.is_null() {
            return None;
        }
        let signer = WTHelperGetProvSignerFromChain(provider, 0, false, 0);
        if signer.is_null() || (*signer).csCertChain == 0 || (*signer).pasCertChain.is_null() {
            return None;
        }
        let cert = (*(*signer).pasCertChain).pCert;
        if cert.is_null() {
            return None;
        }
        let mut buffer = [0u16; 256];
        let length = CertGetNameStringW(
            cert,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            None,
            Some(&mut buffer),
        );
        let length = usize::try_from(length)
            .unwrap_or(0)
            .saturating_sub(1)
            .min(buffer.len());
        let name = String::from_utf16_lossy(&buffer[..length]);
        let name = name.trim();
        (!name.is_empty()).then(|| name.to_owned())
    }
}

fn verify_embedded(path: &HSTRING) -> Check {
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_u32::<WINTRUST_FILE_INFO>(),
        pcwszFilePath: PCWSTR(path.as_ptr()),
        ..Default::default()
    };
    let mut data = wintrust_data(
        WTD_CHOICE_FILE,
        WINTRUST_DATA_0 {
            pFile: &raw mut file,
        },
    );
    verify_with(&mut data)
}

/// A catalog admin context, released when dropped.
struct CatalogAdmin(isize);

impl Drop for CatalogAdmin {
    fn drop(&mut self) {
        // SAFETY: the context came from CryptCATAdminAcquireContext2 and is released once, here.
        let _ = unsafe { CryptCATAdminReleaseContext(self.0, 0) };
    }
}

/// Most files of Windows carry no signature of their own: their hash sits in a signed catalog.
fn verify_by_catalog(path: &Path, wide_path: &HSTRING) -> Option<Verdict> {
    let file = std::fs::File::open(path).ok()?;
    let handle = HANDLE(file.as_raw_handle());
    let mut admin = 0isize;
    // SAFETY: `admin` is a valid out pointer; the algorithm name is a static string.
    unsafe {
        CryptCATAdminAcquireContext2(&raw mut admin, None, BCRYPT_SHA256_ALGORITHM, None, None)
    }
    .ok()?;
    let admin = CatalogAdmin(admin);
    let mut hash = [0u8; 64];
    let mut size = u32::try_from(hash.len()).unwrap_or(0);
    // SAFETY: `hash` holds `size` bytes; the file handle stays open for the call.
    unsafe {
        CryptCATAdminCalcHashFromFileHandle2(
            admin.0,
            handle,
            &raw mut size,
            Some(hash.as_mut_ptr()),
            None,
        )
    }
    .ok()?;
    let hash = &mut hash[..usize::try_from(size).unwrap_or(0).min(64)];
    // SAFETY: `hash` is the computed hash; no previous catalog is passed.
    let catalog = unsafe { CryptCATAdminEnumCatalogFromHash(admin.0, hash, None, None) };
    if catalog == 0 {
        return None;
    }
    let mut info = CATALOG_INFO {
        cbStruct: size_u32::<CATALOG_INFO>(),
        ..Default::default()
    };
    // SAFETY: `catalog` is the live context just found and `info` a valid out structure.
    let found = unsafe { CryptCATCatalogInfoFromContext(catalog, &raw mut info, 0) };
    let verdict = found.ok().and_then(|()| {
        let tag = hash.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02X}");
            text
        });
        let tag = HSTRING::from(tag);
        let mut member = WINTRUST_CATALOG_INFO {
            cbStruct: size_u32::<WINTRUST_CATALOG_INFO>(),
            pcwszCatalogFilePath: PCWSTR(info.wszCatalogFile.as_ptr()),
            pcwszMemberTag: PCWSTR(tag.as_ptr()),
            pcwszMemberFilePath: PCWSTR(wide_path.as_ptr()),
            hMemberFile: handle,
            pbCalculatedFileHash: hash.as_mut_ptr(),
            cbCalculatedFileHash: u32::try_from(hash.len()).unwrap_or(0),
            hCatAdmin: admin.0,
            ..Default::default()
        };
        let mut data = wintrust_data(
            WTD_CHOICE_CATALOG,
            WINTRUST_DATA_0 {
                pCatalog: &raw mut member,
            },
        );
        match verify_with(&mut data) {
            Check::Verdict(verdict) => Some(verdict),
            Check::NoSignature => None,
        }
    });
    // SAFETY: the catalog context came from the enumeration above and is released once.
    let _ = unsafe { CryptCATAdminReleaseCatalogContext(admin.0, catalog, 0) };
    drop(file);
    verdict
}

/// The signature of a file: its own, else the catalog's.
pub fn verify(path: &Path) -> Verdict {
    if !path.is_file() {
        return Verdict::NO_FILE;
    }
    let wide = HSTRING::from(path.as_os_str());
    match verify_embedded(&wide) {
        Check::Verdict(verdict) => verdict,
        Check::NoSignature => verify_by_catalog(path, &wide).unwrap_or(Verdict {
            state: SignatureState::Unsigned,
            signer: None,
        }),
    }
}

/// A value of the version resource as a slice of u16 inside `block`.
fn query_units<'a>(block: &'a [u8], query: &str) -> Option<&'a [u16]> {
    let mut pointer: *mut c_void = std::ptr::null_mut();
    let mut length = 0u32;
    // SAFETY: `block` holds the whole resource GetFileVersionInfoW wrote; the pointer returned points into it.
    let found = unsafe {
        VerQueryValueW(
            block.as_ptr().cast(),
            &HSTRING::from(query),
            &raw mut pointer,
            &raw mut length,
        )
    };
    if !found.as_bool() || pointer.is_null() || length == 0 {
        return None;
    }
    // Translation answers in bytes, a string value in u16 with its terminator
    let units = if query.ends_with("Translation") {
        length / 2
    } else {
        length
    };
    // SAFETY: the value lies inside `block` and is `units` u16 long; `block` outlives the slice.
    Some(unsafe {
        std::slice::from_raw_parts(pointer.cast::<u16>(), usize::try_from(units).unwrap_or(0))
    })
}

/// A string of the version resource, in the file's first language, else US English.
fn version_text(block: &[u8], name: &str) -> Option<String> {
    let mut languages: Vec<String> = query_units(block, r"\VarFileInfo\Translation")
        .map(|pairs| {
            pairs
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| format!("{:04x}{:04x}", pair[0], pair[1]))
                .collect()
        })
        .unwrap_or_default();
    languages.push("040904b0".to_owned());
    languages.into_iter().find_map(|language| {
        let units = query_units(block, &format!(r"\StringFileInfo\{language}\{name}"))?;
        // Some files count the value in bytes, not characters: what follows the terminator is not the value
        let end = units
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(units.len());
        let text = String::from_utf16_lossy(&units[..end]);
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    })
}

/// The `CompanyName` the file gives for itself.
pub fn company(path: &Path) -> Option<String> {
    let wide = HSTRING::from(path.as_os_str());
    // SAFETY: `wide` is a live null-terminated path.
    let size = unsafe { GetFileVersionInfoSizeW(&wide, None) };
    if size == 0 {
        return None;
    }
    let mut block = vec![0u8; usize::try_from(size).ok()?];
    // SAFETY: `block` holds `size` bytes.
    unsafe { GetFileVersionInfoW(&wide, None, size, block.as_mut_ptr().cast()) }.ok()?;
    version_text(&block, "CompanyName")
}
