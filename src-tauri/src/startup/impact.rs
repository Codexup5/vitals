//! What a program costs the sign-in, from the traces Windows keeps for Task Manager:
//! `%SystemRoot%\System32\wdi\LogFiles\StartupInfo\<SID>_StartupInfo<N>.xml`, one per recent sign-in, readable by the
//! user. Each lists the processes of the first two minutes with their CPU time and disk bytes; a program costs what
//! it and every process it started cost, as Task Manager counts it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use vitals_ipc::{ImpactLevel, StartupImpact};

/// Task Manager's bars: high above 1 s of CPU or 3 MB of disk, medium from 300 ms or 300 KB
const HIGH_CPU_MS: u64 = 1000;
const HIGH_DISK_BYTES: u64 = 3 * 1024 * 1024;
const MEDIUM_CPU_MS: u64 = 300;
const MEDIUM_DISK_BYTES: u64 = 300 * 1024;

#[derive(Deserialize)]
struct StartupDataXml {
    #[serde(rename = "Process", default)]
    processes: Vec<ProcessXml>,
}

#[derive(Deserialize)]
struct ProcessXml {
    #[serde(rename = "@Name")]
    name: String,
    #[serde(rename = "@PID")]
    pid: u32,
    #[serde(rename = "StartTime")]
    start_time: String,
    #[serde(rename = "DiskUsage", default)]
    disk: Option<UsageXml>,
    #[serde(rename = "CpuUsage", default)]
    cpu: Option<UsageXml>,
    #[serde(rename = "ParentPID", default)]
    parent_pid: Option<u32>,
    #[serde(rename = "ParentStartTime", default)]
    parent_start: Option<String>,
}

#[derive(Deserialize)]
struct UsageXml {
    #[serde(rename = "$text")]
    value: u64,
}

/// A process is known by its id and start time: an id is reused, the pair is not.
type ProcessKey = (u32, String);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Traced {
    /// Lowercase full path
    path: String,
    /// Lowercase file name
    file: String,
    key: ProcessKey,
    parent: Option<ProcessKey>,
    cpu_us: u64,
    disk_bytes: u64,
}

/// One sign-in as Windows traced it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Trace {
    processes: Vec<Traced>,
}

fn file_of(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_owned()
}

/// The traces are UTF-16 with a byte order mark.
pub fn decode(bytes: &[u8]) -> String {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Reads a trace; its declaration names UTF-16, which the decoded text no longer is, so it is dropped.
pub fn parse(text: &str) -> Result<Trace, quick_xml::DeError> {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let body = match text.strip_prefix("<?xml") {
        Some(rest) => rest.split_once("?>").map_or(rest, |(_, body)| body),
        None => text,
    };
    let data: StartupDataXml = quick_xml::de::from_str(body)?;
    Ok(Trace {
        processes: data
            .processes
            .into_iter()
            .map(|process| {
                let path = process.name.to_lowercase();
                Traced {
                    file: file_of(&path),
                    path,
                    key: (process.pid, process.start_time),
                    parent: process.parent_pid.zip(process.parent_start),
                    cpu_us: process.cpu.map_or(0, |usage| usage.value),
                    disk_bytes: process.disk.map_or(0, |usage| usage.value),
                }
            })
            .collect(),
    })
}

/// The trace files of one user, newest sign-in data first by name order of Windows.
pub fn trace_files(dir: &Path, sid: &str) -> Vec<PathBuf> {
    let prefix = format!("{sid}_StartupInfo").to_lowercase();
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .is_some_and(|name| name.starts_with(&prefix))
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        })
        .collect();
    files.sort();
    files
}

/// CPU microseconds and disk bytes of a program and its descendants in one sign-in; none when it did not run.
fn cost_in(trace: &Trace, image: &Path) -> Option<(u64, u64)> {
    let path = image.to_string_lossy().to_lowercase();
    let file = file_of(&path);
    // A path through an alias (an app of the Store) differs from the real one: then the file name has to do
    let mut matched: Vec<usize> = (0..trace.processes.len())
        .filter(|&at| trace.processes[at].path == path)
        .collect();
    if matched.is_empty() {
        matched = (0..trace.processes.len())
            .filter(|&at| trace.processes[at].file == file)
            .collect();
    }
    if matched.is_empty() {
        return None;
    }
    let mut children: HashMap<&ProcessKey, Vec<usize>> = HashMap::new();
    for (at, process) in trace.processes.iter().enumerate() {
        if let Some(parent) = &process.parent {
            children.entry(parent).or_default().push(at);
        }
    }
    let mut seen = HashSet::new();
    let mut queue = matched;
    while let Some(at) = queue.pop() {
        if seen.insert(at)
            && let Some(more) = children.get(&trace.processes[at].key)
        {
            queue.extend(more);
        }
    }
    Some(seen.iter().fold((0, 0), |(cpu, disk), &at| {
        let process = &trace.processes[at];
        (cpu + process.cpu_us, disk + process.disk_bytes)
    }))
}

pub const fn level_of(cpu_ms: u64, disk_bytes: u64) -> ImpactLevel {
    if cpu_ms > HIGH_CPU_MS || disk_bytes > HIGH_DISK_BYTES {
        ImpactLevel::High
    } else if cpu_ms >= MEDIUM_CPU_MS || disk_bytes >= MEDIUM_DISK_BYTES {
        ImpactLevel::Medium
    } else {
        ImpactLevel::Low
    }
}

/// The mean cost over the sign-ins the program ran in.
pub fn impact_of(traces: &[Trace], image: &Path) -> Option<StartupImpact> {
    let costs: Vec<(u64, u64)> = traces
        .iter()
        .filter_map(|trace| cost_in(trace, image))
        .collect();
    let boots = u32::try_from(costs.len()).ok().filter(|boots| *boots > 0)?;
    let (total_us, total_bytes) = costs
        .iter()
        .fold((0, 0), |(cpu, disk), (one_cpu, one_disk)| {
            (cpu + one_cpu, disk + one_disk)
        });
    let (cpu_ms, disk_bytes) = (
        total_us / 1000 / u64::from(boots),
        total_bytes / u64::from(boots),
    );
    Some(StartupImpact {
        level: level_of(cpu_ms, disk_bytes),
        cpu_ms,
        disk_bytes,
        boots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of a real trace, with a launcher that starts its app, the app's child, and a stranger
    const TRACE: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<StartupData IntervalStartMs="0" IntervalEndMs="120066">
	<Process Name="C:\Users\someone\AppData\Local\Discord\Update.exe" PID="100" StartedInTraceSec="3.1">
		<StartTime>2026/10/26:21:33:10.1</StartTime>
		<CommandLine><![CDATA["C:\Users\someone\AppData\Local\Discord\Update.exe" --processStart Discord.exe]]></CommandLine>
		<DiskUsage Units="bytes">100000</DiskUsage>
		<CpuUsage Units="us">200000</CpuUsage>
		<ParentPID>4</ParentPID>
		<ParentStartTime>2026/10/26:21:33:00.0</ParentStartTime>
		<ParentName>explorer.exe</ParentName>
	</Process>
	<Process Name="C:\Users\someone\AppData\Local\Discord\app-1.0\Discord.exe" PID="200" StartedInTraceSec="4.0">
		<StartTime>2026/10/26:21:33:11.0</StartTime>
		<DiskUsage Units="bytes">4000000</DiskUsage>
		<CpuUsage Units="us">900000</CpuUsage>
		<ParentPID>100</ParentPID>
		<ParentStartTime>2026/10/26:21:33:10.1</ParentStartTime>
	</Process>
	<Process Name="C:\Users\someone\AppData\Local\Discord\app-1.0\Discord.exe" PID="300" StartedInTraceSec="5.0">
		<StartTime>2026/10/26:21:33:12.0</StartTime>
		<DiskUsage Units="bytes">0</DiskUsage>
		<CpuUsage Units="us">100000</CpuUsage>
		<ParentPID>200</ParentPID>
		<ParentStartTime>2026/10/26:21:33:11.0</ParentStartTime>
	</Process>
	<Process Name="C:\Windows\System32\conhost.exe" PID="200" StartedInTraceSec="50.7">
		<StartTime>2026/10/26:21:34:02.5</StartTime>
		<DiskUsage Units="bytes">0</DiskUsage>
		<CpuUsage Units="us">7235</CpuUsage>
		<ParentPID>100</ParentPID>
		<ParentStartTime>2026/10/26:21:30:00.0</ParentStartTime>
	</Process>
</StartupData>"#;

    fn utf16(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        bytes
    }

    #[test]
    fn a_program_costs_what_it_and_its_descendants_cost() {
        let trace = parse(&decode(&utf16(TRACE))).unwrap();
        let launcher = Path::new(r"C:\Users\someone\AppData\Local\Discord\Update.exe");
        // The conhost with a reused id of a process that started earlier is not a child
        assert_eq!(cost_in(&trace, launcher), Some((1_200_000, 4_100_000)));
        let impact = impact_of(&[trace.clone(), Trace::default()], launcher).unwrap();
        assert_eq!(
            (impact.level, impact.cpu_ms, impact.boots),
            (ImpactLevel::High, 1200, 1)
        );
        assert_eq!(impact_of(&[trace], Path::new(r"C:\nope.exe")), None);
    }

    #[test]
    fn an_alias_path_falls_back_to_the_file_name() {
        let trace = parse(TRACE).unwrap();
        let alias = Path::new(r"C:\Users\someone\AppData\Local\Microsoft\WindowsApps\conhost.exe");
        assert_eq!(cost_in(&trace, alias), Some((7235, 0)));
    }

    #[test]
    fn the_levels_follow_task_managers_bars() {
        assert_eq!(level_of(299, 300 * 1024 - 1), ImpactLevel::Low);
        assert_eq!(level_of(300, 0), ImpactLevel::Medium);
        assert_eq!(level_of(0, 3 * 1024 * 1024), ImpactLevel::Medium);
        assert_eq!(level_of(1001, 0), ImpactLevel::High);
    }

    #[test]
    fn the_files_of_one_user_are_found() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "S-1-5-21-1-1000_StartupInfo1.xml",
            "S-1-5-21-1-1001_StartupInfo1.xml",
            "S-1-5-21-1-1001_StartupInfo2.xml",
        ] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        assert_eq!(trace_files(dir.path(), "S-1-5-21-1-1001").len(), 2);
    }
}
