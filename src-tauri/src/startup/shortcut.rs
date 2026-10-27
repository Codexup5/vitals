//! The target and arguments of a `.lnk` file, read by the format of [MS-SHLLINK] without COM: the local path of its
//! `LinkInfo`, else the target of its environment block.

/// What a shortcut starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub target: String,
    pub arguments: Option<String>,
}

const HEADER_SIZE: usize = 0x4C;
const HAS_ID_LIST: u32 = 0x1;
const HAS_LINK_INFO: u32 = 0x2;
const HAS_NAME: u32 = 0x4;
const HAS_RELATIVE_PATH: u32 = 0x8;
const HAS_WORKING_DIR: u32 = 0x10;
const HAS_ARGUMENTS: u32 = 0x20;
const HAS_ICON: u32 = 0x40;
const IS_UNICODE: u32 = 0x80;
const LOCAL_PATH: u32 = 0x1;
const ENVIRONMENT_BLOCK: u32 = 0xA000_0001;

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    let pair = bytes.get(at..at + 2)?;
    Some(usize::from(u16::from_le_bytes([pair[0], pair[1]])))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let four = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([four[0], four[1], four[2], four[3]]))
}

fn offset_at(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u32_at(bytes, at)?).ok()
}

/// A string ending at its first zero: bytes as Latin text, or UTF-16.
fn terminated(bytes: &[u8], is_wide: bool) -> String {
    if is_wide {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .take_while(|unit| *unit != 0)
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
}

/// The local path of a `LinkInfo` structure starting at `start`, the Unicode form when there is one.
fn local_path(bytes: &[u8], start: usize) -> Option<String> {
    let info = bytes.get(start..start + offset_at(bytes, start)?)?;
    let header = offset_at(info, 4)?;
    if u32_at(info, 8)? & LOCAL_PATH == 0 {
        return None;
    }
    let (base, suffix) = if header >= 0x24 {
        (
            terminated(info.get(offset_at(info, 28)?..)?, true),
            terminated(info.get(offset_at(info, 32)?..)?, true),
        )
    } else {
        (
            terminated(info.get(offset_at(info, 16)?..)?, false),
            terminated(info.get(offset_at(info, 24)?..)?, false),
        )
    };
    let path = base + &suffix;
    (!path.is_empty()).then_some(path)
}

/// The target the environment block names, `%VARIABLES%` and all.
fn environment_target(bytes: &[u8], mut at: usize) -> Option<String> {
    loop {
        let size = offset_at(bytes, at)?;
        if size < 8 {
            return None;
        }
        if u32_at(bytes, at + 4)? == ENVIRONMENT_BLOCK {
            let target = terminated(bytes.get(at + 8 + 260..at + 8 + 260 + 520)?, true);
            return (!target.is_empty()).then_some(target);
        }
        at += size;
    }
}

/// Reads a shortcut; `None` when it is not one or names no file.
pub fn parse(bytes: &[u8]) -> Option<Link> {
    if offset_at(bytes, 0)? != HEADER_SIZE {
        return None;
    }
    let flags = u32_at(bytes, 0x14)?;
    let mut at = HEADER_SIZE;
    if flags & HAS_ID_LIST != 0 {
        at += 2 + u16_at(bytes, at)?;
    }
    let mut target = None;
    if flags & HAS_LINK_INFO != 0 {
        target = local_path(bytes, at);
        at += offset_at(bytes, at)?;
    }
    let is_wide = flags & IS_UNICODE != 0;
    let mut arguments = None;
    for flag in [
        HAS_NAME,
        HAS_RELATIVE_PATH,
        HAS_WORKING_DIR,
        HAS_ARGUMENTS,
        HAS_ICON,
    ] {
        if flags & flag == 0 {
            continue;
        }
        let count = u16_at(bytes, at)?;
        let length = if is_wide { count * 2 } else { count };
        let text = bytes.get(at + 2..at + 2 + length)?;
        if flag == HAS_ARGUMENTS {
            let text = if is_wide {
                let units: Vec<u16> = text
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair))
                    .collect();
                String::from_utf16_lossy(&units)
            } else {
                String::from_utf8_lossy(text).into_owned()
            };
            arguments = Some(text.trim().to_owned()).filter(|text| !text.is_empty());
        }
        at += 2 + length;
    }
    let target = target.or_else(|| environment_target(bytes, at))?;
    Some(Link { target, arguments })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn header(flags: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; HEADER_SIZE];
        bytes[0] = 0x4C;
        bytes[0x14..0x18].copy_from_slice(&flags.to_le_bytes());
        bytes
    }

    /// A `LinkInfo` with the ANSI local path only, as older shortcuts have
    fn link_info(path: &str) -> Vec<u8> {
        let mut info = vec![0u8; 0x1C];
        info[4] = 0x1C;
        info[8] = 1;
        info[16..20].copy_from_slice(&0x1Cu32.to_le_bytes());
        let suffix = 0x1C + path.len() + 1;
        info[24..28].copy_from_slice(&u32::try_from(suffix).unwrap().to_le_bytes());
        info.extend(path.as_bytes());
        info.extend([0, 0]);
        let size = u32::try_from(info.len()).unwrap();
        info[0..4].copy_from_slice(&size.to_le_bytes());
        info
    }

    #[test]
    fn a_shortcut_gives_its_target_and_arguments() {
        let mut bytes = header(HAS_ID_LIST | HAS_LINK_INFO | HAS_ARGUMENTS | IS_UNICODE);
        bytes.extend(4u16.to_le_bytes());
        bytes.extend([1, 2, 3, 4]);
        bytes.extend(link_info(r"C:\Program Files\ShareX\ShareX.exe"));
        let arguments = "-silent";
        bytes.extend(u16::try_from(arguments.len()).unwrap().to_le_bytes());
        bytes.extend(wide(arguments));
        assert_eq!(
            parse(&bytes),
            Some(Link {
                target: r"C:\Program Files\ShareX\ShareX.exe".to_owned(),
                arguments: Some("-silent".to_owned()),
            })
        );
    }

    #[test]
    fn a_shortcut_without_link_info_uses_its_environment_block() {
        let mut bytes = header(IS_UNICODE);
        let mut block = vec![0u8; 0x314];
        block[0..4].copy_from_slice(&0x314u32.to_le_bytes());
        block[4..8].copy_from_slice(&ENVIRONMENT_BLOCK.to_le_bytes());
        let target = wide(r"%ProgramFiles%\Tool\tool.exe");
        block[8 + 260..8 + 260 + target.len()].copy_from_slice(&target);
        bytes.extend(block);
        bytes.extend([0, 0, 0, 0]);
        assert_eq!(
            parse(&bytes).map(|link| link.target),
            Some(r"%ProgramFiles%\Tool\tool.exe".to_owned())
        );
    }

    #[test]
    fn not_a_shortcut_reads_as_none() {
        assert_eq!(parse(b"@echo off"), None);
        assert_eq!(parse(&header(0)), None);
    }
}
