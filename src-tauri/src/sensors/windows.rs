//! The Windows side of the sensors: the processor's performance counters (PDH), its name in the registry, the memory,
//! and `nvidia-smi` of the NVIDIA driver.
#![allow(
    unsafe_code,
    reason = "the PDH and registry calls of the sensors live in this file"
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Performance::{
    PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
    PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
    PdhGetFormattedCounterValue, PdhOpenQueryW,
};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::core::{HSTRING, PCWSTR};

use super::{CpuCounters, GpuReading, NVIDIA_SMI_FIELDS, SensorReading, cpu_of, gpus_of};
use crate::process::output;
use crate::system::windows::memory_status;

/// `PDH_FMT_NOCAP100` of pdh.h, which the windows crate does not export: the performance passes 100 under boost
const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
const SMI_DEADLINE: Duration = Duration::from_secs(3);

// English names: `PdhAddEnglishCounterW` finds them in a Windows of any language
const UTILITY: &str = r"\Processor Information(_Total)\% Processor Utility";
const PERFORMANCE: &str = r"\Processor Information(_Total)\% Processor Performance";
const FREQUENCY: &str = r"\Processor Information(_Total)\Processor Frequency";

/// A PDH query of the processor's counters. Rate counters need two collections, so each `read` gives the interval
/// since the one before; the query is opened with a first collection.
#[derive(Debug)]
pub struct CpuSampler {
    query: PDH_HQUERY,
    utility: Option<PDH_HCOUNTER>,
    performance: Option<PDH_HCOUNTER>,
    frequency: Option<PDH_HCOUNTER>,
}

impl CpuSampler {
    pub fn open() -> Option<Self> {
        let mut query = PDH_HQUERY(std::ptr::null_mut());
        // SAFETY: no data source means the live counters; `query` is a live slot the call fills.
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &raw mut query) } != ERROR_SUCCESS.0 {
            return None;
        }
        let counter = |path: &str| {
            let mut counter = PDH_HCOUNTER(std::ptr::null_mut());
            // SAFETY: the query is open, the path is a terminated string for the call, `counter` is a live slot.
            let added =
                unsafe { PdhAddEnglishCounterW(query, &HSTRING::from(path), 0, &raw mut counter) };
            (added == ERROR_SUCCESS.0).then_some(counter)
        };
        let sampler = Self {
            query,
            utility: counter(UTILITY),
            performance: counter(PERFORMANCE),
            frequency: counter(FREQUENCY),
        };
        sampler.collect();
        Some(sampler)
    }

    fn collect(&self) -> bool {
        // SAFETY: the query stays open until the sampler drops.
        let status = unsafe { PdhCollectQueryData(self.query) };
        status == ERROR_SUCCESS.0
    }

    fn value(counter: Option<PDH_HCOUNTER>, format: u32) -> Option<f64> {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: the counter belongs to the open query; `value` is a live struct the call fills.
        let status =
            unsafe { PdhGetFormattedCounterValue(counter?, PDH_FMT(format), None, &raw mut value) };
        if status != ERROR_SUCCESS.0
            || !matches!(value.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA)
        {
            return None;
        }
        // SAFETY: an answer in PDH_FMT_DOUBLE fills the double of the union.
        let number = unsafe { value.Anonymous.doubleValue };
        number.is_finite().then_some(number)
    }

    pub fn read(&self) -> CpuCounters {
        if !self.collect() {
            return CpuCounters::default();
        }
        let uncapped = PDH_FMT_DOUBLE.0 | PDH_FMT_NOCAP100;
        CpuCounters {
            utility_pct: Self::value(self.utility, uncapped),
            performance_pct: Self::value(self.performance, uncapped),
            base_mhz: Self::value(self.frequency, PDH_FMT_DOUBLE.0),
        }
    }
}

impl Drop for CpuSampler {
    fn drop(&mut self) {
        // SAFETY: the query was opened by `open` and is closed once, with its counters.
        unsafe { PdhCloseQuery(self.query) };
    }
}

/// `AMD Ryzen 9 9900X 12-Core Processor`, as Windows keeps it for the first processor.
fn cpu_name() -> Option<String> {
    let mut buffer = vec![0u16; 256];
    let mut size = u32::try_from(buffer.len() * 2).unwrap_or(0);
    // SAFETY: `buffer` holds `size` bytes and outlives the call; the value is a terminated string.
    let read = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"),
            &HSTRING::from("ProcessorNameString"),
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
    let end = buffer[..units]
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units);
    let text = String::from_utf16_lossy(&buffer[..end]);
    (!text.trim().is_empty()).then(|| text.trim().to_owned())
}

/// `nvidia-smi` by its full path, never by a search of `PATH`: the DCH driver puts it in System32, older ones in
/// `NVSMI`.
fn nvidia_smi() -> Option<PathBuf> {
    let system = std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("System32").join("nvidia-smi.exe"));
    let program_files = std::env::var_os("ProgramFiles").map(|dir| {
        PathBuf::from(dir)
            .join("NVIDIA Corporation")
            .join("NVSMI")
            .join("nvidia-smi.exe")
    });
    [system, program_files]
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
}

fn gpus(smi: &Path) -> Vec<GpuReading> {
    let Some(program) = smi.to_str() else {
        return Vec::new();
    };
    let query = format!("--query-gpu={NVIDIA_SMI_FIELDS}");
    match output(
        program,
        &[&query, "--format=csv,noheader,nounits"],
        SMI_DEADLINE,
    ) {
        Ok(done) if done.code == 0 => gpus_of(&done.stdout),
        // Every two seconds while the screen is open: a failure is not worth a line of the log each time
        Ok(done) => {
            tracing::debug!("nvidia-smi exited with {}", done.code);
            Vec::new()
        }
        Err(error) => {
            tracing::debug!("nvidia-smi did not run: {error}");
            Vec::new()
        }
    }
}

/// What a sampler reads each time: the counters stay open, the processor's name and `nvidia-smi` are found once.
#[derive(Debug)]
pub struct Reader {
    cpu: Option<CpuSampler>,
    cpu_name: Option<String>,
    smi: Option<PathBuf>,
}

impl Reader {
    pub fn open() -> Self {
        Self {
            cpu: CpuSampler::open(),
            cpu_name: cpu_name(),
            smi: nvidia_smi(),
        }
    }

    pub fn read(&self, now: i64) -> SensorReading {
        let counters = self.cpu.as_ref().map(CpuSampler::read).unwrap_or_default();
        let memory = memory_status();
        SensorReading {
            time: now,
            cpu: cpu_of(self.cpu_name.clone(), counters),
            ram_used_bytes: memory.map(|(total, available)| total.saturating_sub(available)),
            ram_total_bytes: memory.map(|(total, _)| total),
            gpus: self.smi.as_deref().map(gpus).unwrap_or_default(),
        }
    }
}
