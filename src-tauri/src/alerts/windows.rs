//! The bot kept with DPAPI: `telegram.bin` in `%APPDATA%\Vitals` opens only for this Windows user on this PC.
#![allow(
    unsafe_code,
    reason = "CryptProtectData and CryptUnprotectData live in this file"
)]

use std::path::Path;

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;

use super::telegram::Bot;

const BOT_FILE: &str = "telegram.bin";

/// The bytes DPAPI hands back, freed with `LocalFree`.
fn take(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    if blob.pbData.is_null() {
        return Vec::new();
    }
    // SAFETY: DPAPI wrote `cbData` bytes at `pbData`; they are copied before the memory is freed once.
    let bytes = unsafe {
        std::slice::from_raw_parts(blob.pbData, usize::try_from(blob.cbData).unwrap_or(0)).to_vec()
    };
    // SAFETY: the buffer was allocated by DPAPI with LocalAlloc and is freed once, here.
    let _ = unsafe { LocalFree(Some(HLOCAL(blob.pbData.cast()))) };
    bytes
}

fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(plain.len()).ok()?,
        pbData: plain.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` points at `plain`, which DPAPI only reads; `output` is a valid out structure.
    unsafe {
        CryptProtectData(
            &raw const input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }
    .ok()?;
    Some(take(output))
}

fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(sealed.len()).ok()?,
        pbData: sealed.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: as in `protect`; no description is asked back.
    unsafe {
        CryptUnprotectData(
            &raw const input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &raw mut output,
        )
    }
    .ok()?;
    Some(take(output))
}

pub fn save_bot(data_dir: &Path, bot: &Bot) -> std::io::Result<()> {
    let plain = serde_json::to_vec(bot)?;
    let sealed =
        protect(&plain).ok_or_else(|| std::io::Error::other("DPAPI did not seal the bot"))?;
    std::fs::write(data_dir.join(BOT_FILE), sealed)
}

pub fn load_bot(data_dir: &Path) -> Option<Bot> {
    let sealed = std::fs::read(data_dir.join(BOT_FILE)).ok()?;
    serde_json::from_slice(&unprotect(&sealed)?).ok()
}

pub fn forget_bot(data_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(data_dir.join(BOT_FILE)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bot_seals_and_opens_for_this_user_only_as_itself() {
        let dir = tempfile::tempdir().unwrap();
        let bot = Bot::new("123:abc", "456").unwrap();
        save_bot(dir.path(), &bot).unwrap();
        let sealed = std::fs::read(dir.path().join(BOT_FILE)).unwrap();
        assert!(!String::from_utf8_lossy(&sealed).contains("123:abc"));
        assert_eq!(load_bot(dir.path()), Some(bot));
        forget_bot(dir.path()).unwrap();
        assert_eq!(load_bot(dir.path()), None);
        forget_bot(dir.path()).unwrap();
    }
}
