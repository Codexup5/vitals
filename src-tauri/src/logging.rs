//! The log of the panel: `%APPDATA%\Vitals\logs\vitals.log.<date>`, a file a day, two weeks kept.

use std::path::Path;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

const KEEP_FILES: usize = 14;

pub fn init(data_dir: &Path) {
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("vitals.log")
        .max_log_files(KEEP_FILES)
        .build(data_dir.join("logs"));
    let filter = EnvFilter::try_from_env("VITALS_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let result = match appender {
        Ok(appender) => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(appender)
            .try_init(),
        // No log folder: the messages go to stderr rather than nowhere
        Err(_) => tracing_subscriber::fmt().with_env_filter(filter).try_init(),
    };
    if result.is_err() {
        tracing::warn!("the log was already set up");
    }
}
