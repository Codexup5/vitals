//! The speed test on the user's click: a download from and an upload to Cloudflare's speed test, through the `curl`
//! that ships with Windows. Nothing else goes out, and nothing runs unless the button is pressed.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use vitals_ipc::{SpeedResult, SpeedStatus};

use crate::process::output;

const DOWN_URL: &str = "https://speed.cloudflare.com/__down?bytes=50000000";
const UP_URL: &str = "https://speed.cloudflare.com/__up";
const UP_BYTES: usize = 20_000_000;
/// A transfer stops here; what came through by then still gives the speed
const TRANSFER_SECONDS: &str = "20";
const DEADLINE: Duration = Duration::from_secs(40);
const UPLOAD_FILE: &str = "speed-test-upload.bin";

/// `12345678.000` that curl prints for `%{speed_download}`, as whole bytes per second.
pub fn bytes_per_second(text: &str) -> Option<u64> {
    let value: f64 = text.trim().replace(',', ".").parse().ok()?;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive, finite speed rounded to whole bytes"
    )]
    (value.is_finite() && value > 0.0).then(|| value.round() as u64)
}

/// curl exits 28 when its time limit stops a transfer that still moved data: the speed it printed counts.
fn transfer(args: &[&str]) -> Option<u64> {
    let done = output("curl", args, DEADLINE)
        .map_err(|error| tracing::warn!("curl did not run: {error}"))
        .ok()?;
    if done.code != 0 && done.code != 28 {
        tracing::warn!("curl exited with {}", done.code);
        return None;
    }
    bytes_per_second(&done.stdout)
}

fn download() -> Option<u64> {
    transfer(&[
        "--silent",
        "--output",
        "NUL",
        "--max-time",
        TRANSFER_SECONDS,
        "--write-out",
        "%{speed_download}",
        DOWN_URL,
    ])
}

/// The upload sends zeros from a file in the panel's own folder, removed after.
fn upload(work_dir: &Path) -> Option<u64> {
    let file: PathBuf = work_dir.join(UPLOAD_FILE);
    std::fs::write(&file, vec![0u8; UP_BYTES])
        .map_err(|error| tracing::warn!("the upload file was not written: {error}"))
        .ok()?;
    let body = format!("@{}", file.display());
    let speed = transfer(&[
        "--silent",
        "--output",
        "NUL",
        "--max-time",
        TRANSFER_SECONDS,
        "--write-out",
        "%{speed_upload}",
        "--data-binary",
        &body,
        UP_URL,
    ]);
    if let Err(error) = std::fs::remove_file(&file) {
        tracing::warn!("the upload file was not removed: {error}");
    }
    speed
}

#[derive(Debug, Default)]
pub struct SpeedJob {
    status: Mutex<SpeedStatus>,
}

impl SpeedJob {
    fn lock(&self) -> MutexGuard<'_, SpeedStatus> {
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn status(&self) -> SpeedStatus {
        *self.lock()
    }

    /// Starts a test unless one runs.
    pub fn start(self: &Arc<Self>, work_dir: PathBuf) -> bool {
        {
            let mut status = self.lock();
            if status.is_running {
                return false;
            }
            status.is_running = true;
        }
        let job = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("speed-test".to_owned())
            .spawn(move || {
                let down = download();
                let up = upload(&work_dir);
                tracing::info!("speed test: down {down:?} B/s, up {up:?} B/s");
                let mut status = job.lock();
                status.is_running = false;
                status.last = Some(SpeedResult {
                    down,
                    up,
                    finished_at: chrono::Utc::now().timestamp(),
                });
            });
        if let Err(error) = spawned {
            tracing::error!("the speed test thread did not start: {error}");
            self.lock().is_running = false;
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_curls_speed_in_any_locale() {
        assert_eq!(bytes_per_second("12345678.000"), Some(12_345_678));
        assert_eq!(bytes_per_second("1234,5"), Some(1235));
        assert_eq!(bytes_per_second("0.000"), None);
        assert_eq!(bytes_per_second(""), None);
    }
}
