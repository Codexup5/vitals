//! `C:\Windows\Minidump`: the crash dumps a normal user can list, matched to the BSOD codes of the event log.

use std::path::{Path, PathBuf};
pub use vitals_ipc::Dump;

use chrono::{DateTime, Utc};

/// A dump and its BSOD event are written minutes apart: the event comes after the reboot.
const MATCH_WINDOW_SECONDS: i64 = 30 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpFile {
    pub name: String,
    pub size: u64,
    pub written: DateTime<Utc>,
}

pub fn minidump_dir() -> PathBuf {
    let root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    root.join("Minidump")
}

/// Newest first; each dump takes the code of the nearest BSOD event within half an hour.
pub fn match_codes(files: Vec<DumpFile>, bsods: &[crate::storage::queries::BsodTime]) -> Vec<Dump> {
    let mut dumps: Vec<Dump> = files
        .into_iter()
        .map(|file| {
            let code = bsods
                .iter()
                .map(|(time, code)| ((*time - file.written).num_seconds().abs(), code))
                .filter(|(distance, _)| *distance <= MATCH_WINDOW_SECONDS)
                .min_by_key(|(distance, _)| *distance)
                .and_then(|(_, code)| code.clone());
            Dump {
                name: file.name,
                size: file.size,
                time: file.written.timestamp(),
                code,
            }
        })
        .collect();
    dumps.sort_by_key(|dump| std::cmp::Reverse(dump.time));
    dumps
}

pub fn list_files(dir: &Path) -> std::io::Result<Vec<DumpFile>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let metadata = entry.metadata()?;
        if !metadata.is_file() || !name.to_ascii_lowercase().ends_with(".dmp") {
            continue;
        }
        files.push(DumpFile {
            name,
            size: metadata.len(),
            written: DateTime::<Utc>::from(metadata.modified()?),
        });
    }
    Ok(files)
}

/// A bare file name of the dump folder; anything with a separator or `..` is refused.
pub fn is_plain_dump_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && name != "."
        && name != ".."
        && name.to_ascii_lowercase().ends_with(".dmp")
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 13, hour, minute, 0).unwrap()
    }

    #[test]
    fn a_dump_takes_the_code_of_the_nearest_bsod() {
        let files = vec![
            DumpFile {
                name: "101326-15296-01.dmp".to_owned(),
                size: 2_000_000,
                written: at(10, 52),
            },
            DumpFile {
                name: "101026-9000-01.dmp".to_owned(),
                size: 1_000_000,
                written: at(1, 0),
            },
        ];
        let bsods = vec![
            (at(10, 56), Some("0x0000004e".to_owned())),
            (at(12, 0), Some("0x000000d1".to_owned())),
        ];
        let dumps = match_codes(files, &bsods);
        assert_eq!(dumps[0].name, "101326-15296-01.dmp");
        assert_eq!(dumps[0].code.as_deref(), Some("0x0000004e"));
        // Nothing within half an hour: the log no longer holds that event
        assert_eq!(dumps[1].code, None);
    }

    #[test]
    fn only_a_bare_dump_name_is_accepted() {
        assert!(is_plain_dump_name("101326-15296-01.dmp"));
        assert!(!is_plain_dump_name(r"..\system32\config.dmp"));
        assert!(!is_plain_dump_name("C:evil.dmp"));
        assert!(!is_plain_dump_name("notes.txt"));
    }
}
