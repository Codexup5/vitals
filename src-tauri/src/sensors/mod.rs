//! The sensors that need no driver: the load and the clock of the processor from Windows' performance counters, the
//! memory in use, and NVIDIA cards through `nvidia-smi`, which comes with their driver. The temperature of the
//! processor and the fans need `LibreHardwareMonitor` (the brief, §1 p.8) and come with it.
//!
//! The live chart reads every two seconds, but only while the screen asks: with the window closed only the
//! five-minute sample of the history runs (the brief, §1 p.12).

#[cfg(windows)]
pub mod windows;

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

pub use vitals_ipc::{CpuReading, GpuReading, SensorPoint, SensorReading, SensorsLive};

pub const LIVE_STEP: Duration = Duration::from_secs(2);
pub const LIVE_SPAN_SECONDS: i64 = 10 * 60;
/// The live sampler stops when the screen has not asked for this long: the window closed or another screen opened
const IDLE: Duration = Duration::from_secs(15);
/// The history keeps a month of five-minute samples
pub const KEEP_DAYS: i64 = 30;
/// What `nvidia-smi --query-gpu` is asked, in the order `gpus_of` reads
pub const NVIDIA_SMI_FIELDS: &str = "name,temperature.gpu,utilization.gpu,power.draw,power.limit,fan.speed,clocks.gr,clocks.mem,memory.used,memory.total";
const NVIDIA_SMI_VALUES: usize = 9;

/// One value of `--format=csv,noheader,nounits`: `[N/A]` and `[Not Supported]` are no value.
fn csv_number(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    value.is_finite().then_some(value)
}

/// The cards of `nvidia-smi --query-gpu=<NVIDIA_SMI_FIELDS> --format=csv,noheader,nounits`, a line per card. The
/// values are the last nine fields, so a comma in a card's name stays in the name.
pub fn gpus_of(csv: &str) -> Vec<GpuReading> {
    csv.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            let (name, values) = fields.split_at(fields.len().checked_sub(NVIDIA_SMI_VALUES)?);
            let name = name.join(",");
            let [temp, load, power, limit, fan, core, memory, used, total] = values else {
                return None;
            };
            (!name.is_empty()).then(|| GpuReading {
                name,
                temp_c: csv_number(temp),
                load_pct: csv_number(load),
                power_w: csv_number(power),
                power_limit_w: csv_number(limit),
                fan_pct: csv_number(fan),
                core_mhz: csv_number(core),
                memory_mhz: csv_number(memory),
                vram_used_mb: csv_number(used),
                vram_total_mb: csv_number(total),
            })
        })
        .collect()
}

/// What the counters of `Processor Information(_Total)` said over the last interval.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CpuCounters {
    /// `% Processor Utility`: the work done against the base clock, Task Manager's utilization; over 100 under boost
    pub utility_pct: Option<f64>,
    /// `% Processor Performance`: the clock against the base one
    pub performance_pct: Option<f64>,
    /// `Processor Frequency`: the base clock, MHz
    pub base_mhz: Option<f64>,
}

/// The processor as Task Manager shows it: the utilization capped at 100, the base clock times the performance.
pub fn cpu_of(name: Option<String>, counters: CpuCounters) -> CpuReading {
    CpuReading {
        name,
        load_pct: counters.utility_pct.map(|value| value.clamp(0.0, 100.0)),
        mhz: counters
            .performance_pct
            .zip(counters.base_mhz)
            .map(|(performance, base)| base * performance / 100.0),
        base_mhz: counters.base_mhz,
    }
}

/// The point a chart draws from a reading.
#[allow(
    clippy::cast_precision_loss,
    reason = "a share of memory in percent: bytes beyond 2^53 lose nothing visible"
)]
pub fn point_of(reading: &SensorReading) -> SensorPoint {
    let gpu = reading.gpus.first();
    SensorPoint {
        time: reading.time,
        cpu_load_pct: reading.cpu.load_pct,
        cpu_mhz: reading.cpu.mhz,
        ram_used_pct: reading
            .ram_used_bytes
            .zip(reading.ram_total_bytes)
            .filter(|(_, total)| *total > 0)
            .map(|(used, total)| used as f64 * 100.0 / total as f64),
        gpu_temp_c: gpu.and_then(|gpu| gpu.temp_c),
        gpu_load_pct: gpu.and_then(|gpu| gpu.load_pct),
        gpu_power_w: gpu.and_then(|gpu| gpu.power_w),
    }
}

fn trim(points: &mut VecDeque<SensorPoint>, from: i64) {
    while points.front().is_some_and(|point| point.time < from) {
        points.pop_front();
    }
}

#[derive(Debug, Default)]
struct LiveState {
    is_running: bool,
    asked_at: Option<Instant>,
    reading: Option<SensorReading>,
    points: VecDeque<SensorPoint>,
}

/// The live readings the screen polls; the sampler thread that fills them runs only while the screen asks.
#[derive(Debug, Default)]
pub struct LiveSensors {
    state: Mutex<LiveState>,
}

impl LiveSensors {
    fn lock(&self) -> MutexGuard<'_, LiveState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes that the screen asked, and says whether a sampler must be started: none runs.
    pub fn claim_start(&self) -> bool {
        let mut state = self.lock();
        state.asked_at = Some(Instant::now());
        !std::mem::replace(&mut state.is_running, true)
    }

    /// Gives a start back when its thread did not spawn.
    pub fn release(&self) {
        self.lock().is_running = false;
    }

    /// Keeps a reading and drops the points older than the live span.
    pub fn keep(&self, reading: SensorReading) {
        let mut state = self.lock();
        state.points.push_back(point_of(&reading));
        trim(&mut state.points, reading.time - LIVE_SPAN_SECONDS);
        state.reading = Some(reading);
    }

    /// Whether the sampler goes on at `now`: the screen asked within the idle time. The stop is marked under the same
    /// lock, so a screen that asks right after it starts a new sampler.
    pub fn should_continue(&self, now: Instant) -> bool {
        let mut state = self.lock();
        let is_wanted = state
            .asked_at
            .is_some_and(|at| now.saturating_duration_since(at) < IDLE);
        if !is_wanted {
            state.is_running = false;
        }
        is_wanted
    }

    pub fn has_reading(&self) -> bool {
        self.lock().reading.is_some()
    }

    /// The latest reading and the points of the last ten minutes before `now` (Unix seconds).
    pub fn snapshot(&self, now: i64) -> SensorsLive {
        let mut state = self.lock();
        trim(&mut state.points, now - LIVE_SPAN_SECONDS);
        SensorsLive {
            reading: state.reading.clone(),
            points: state.points.iter().copied().collect(),
        }
    }
}

/// The counters need an interval before their first reading: a sampler waits this long after it opens them.
pub const FIRST_INTERVAL: Duration = Duration::from_secs(1);
/// The screen waits this long for the first reading of a sampler it just started, rather than drawing empty
const FIRST_WAIT: Duration = Duration::from_secs(3);

impl LiveSensors {
    /// Waits for the first reading of a sampler that just started; a running one answers at once.
    pub fn wait_reading(&self) {
        let started = Instant::now();
        while !self.has_reading() && started.elapsed() < FIRST_WAIT {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Starts the live sampler on a thread of its own unless one runs; it ends when the screen stops asking.
#[cfg(windows)]
pub fn start_live(live: &std::sync::Arc<LiveSensors>) {
    if !live.claim_start() {
        return;
    }
    let job = std::sync::Arc::clone(live);
    let spawned = std::thread::Builder::new()
        .name("sensors-live".to_owned())
        .spawn(move || {
            let reader = windows::Reader::open();
            std::thread::sleep(FIRST_INTERVAL);
            run_live(
                &job,
                || reader.read(chrono::Utc::now().timestamp()),
                std::thread::sleep,
            );
        });
    if let Err(error) = spawned {
        tracing::error!("the live sensors did not start: {error}");
        live.release();
    }
}

/// The live sampler: a reading every step until the screen stops asking.
pub fn run_live(
    live: &LiveSensors,
    mut read: impl FnMut() -> SensorReading,
    sleep: impl Fn(Duration),
) {
    loop {
        live.keep(read());
        if !live.should_continue(Instant::now()) {
            break;
        }
        sleep(LIVE_STEP);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    // Two cards, the second one an older card that reports little; the values as the 621.01 driver prints them
    const SMI: &str = "NVIDIA GeForce RTX 4070 Ti SUPER, 26.72, 20, 23.54, 285.00, 0, 210, 405, 1516, 16376\n\
                       Quadro, K620, [N/A], 3, [Not Supported], [Not Supported], 35, 135, 324, 10, 2048\n";

    #[test]
    fn reads_every_card_of_nvidia_smi() {
        let gpus = gpus_of(SMI);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4070 Ti SUPER");
        assert_eq!(gpus[0].temp_c, Some(26.72));
        assert_eq!(gpus[0].power_limit_w, Some(285.0));
        assert_eq!(gpus[0].fan_pct, Some(0.0));
        assert_eq!(gpus[0].vram_total_mb, Some(16376.0));
        // A comma in the name stays in it; what the card does not report is absent
        assert_eq!(gpus[1].name, "Quadro,K620");
        assert_eq!((gpus[1].temp_c, gpus[1].load_pct), (None, Some(3.0)));
        assert_eq!(gpus[1].power_w, None);
    }

    #[test]
    fn a_line_that_is_not_a_card_is_skipped() {
        for text in ["", "No devices were found\n", ", 1, 2, 3, 4, 5, 6, 7, 8, 9"] {
            assert_eq!(gpus_of(text), Vec::<GpuReading>::new(), "{text}");
        }
    }

    #[test]
    fn the_processor_as_task_manager_shows_it() {
        let cpu = cpu_of(
            Some("AMD Ryzen 9 9900X".to_owned()),
            CpuCounters {
                utility_pct: Some(131.0),
                performance_pct: Some(112.5),
                base_mhz: Some(4400.0),
            },
        );
        assert_eq!(cpu.load_pct, Some(100.0));
        assert_eq!(cpu.mhz, Some(4950.0));
        // The first reading has no interval yet: no load and no clock, the base is known
        let first = cpu_of(
            None,
            CpuCounters {
                base_mhz: Some(4400.0),
                ..CpuCounters::default()
            },
        );
        assert_eq!((first.load_pct, first.mhz), (None, None));
    }

    fn reading(time: i64) -> SensorReading {
        SensorReading {
            time,
            cpu: cpu_of(None, CpuCounters::default()),
            ram_used_bytes: Some(16),
            ram_total_bytes: Some(64),
            gpus: gpus_of(SMI),
        }
    }

    #[test]
    fn a_point_takes_the_first_card_and_the_share_of_memory() {
        let point = point_of(&reading(100));
        assert_eq!(point.ram_used_pct, Some(25.0));
        assert_eq!(point.gpu_temp_c, Some(26.72));
        assert_eq!(point.gpu_power_w, Some(23.54));
        let bare = SensorReading {
            ram_total_bytes: Some(0),
            gpus: Vec::new(),
            ..reading(100)
        };
        assert_eq!(
            (point_of(&bare).ram_used_pct, point_of(&bare).gpu_temp_c),
            (None, None)
        );
    }

    #[test]
    fn the_live_points_cover_ten_minutes() {
        let live = LiveSensors::default();
        for time in [0, 300, 600, 700] {
            live.keep(reading(time));
        }
        let times: Vec<i64> = live
            .snapshot(700)
            .points
            .iter()
            .map(|point| point.time)
            .collect();
        assert_eq!(times, [300, 600, 700]);
        assert_eq!(live.snapshot(1250).points.len(), 1);
        assert_eq!(
            live.snapshot(1250).reading.map(|reading| reading.time),
            Some(700)
        );
    }

    #[test]
    fn one_sampler_runs_and_stops_when_the_screen_stops_asking() {
        let live = LiveSensors::default();
        assert!(live.claim_start());
        assert!(!live.claim_start());
        assert!(live.should_continue(Instant::now()));
        assert!(!live.should_continue(Instant::now() + IDLE * 2));
        // Stopped: the next ask starts a sampler again
        assert!(live.claim_start());
    }

    #[test]
    fn the_sampler_reads_until_the_screen_is_gone() {
        let live = LiveSensors::default();
        live.claim_start();
        let reads = Cell::new(0);
        run_live(
            &live,
            || {
                reads.set(reads.get() + 1);
                reading(reads.get())
            },
            // The screen stops asking after the third step
            |_| {
                if reads.get() == 3 {
                    live.lock().asked_at = None;
                }
            },
        );
        assert_eq!(reads.get(), 4);
        assert!(live.has_reading());
        assert!(live.claim_start());
    }
}
