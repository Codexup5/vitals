//! The panel's own settings and the memory of the alerts, as key and text; and the events an alert speaks of.

use rusqlite::{OptionalExtension, params};

use super::{Storage, StorageError};
use crate::alerts::Found;

impl Storage {
    pub fn setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// The crashes, BSODs, WHEA errors and GPU resets after `since` (Unix seconds).
    pub fn alert_events_since(&self, since: i64) -> Result<Found, StorageError> {
        let mut statement = self.conn.prepare_cached(
            "SELECT kind, time, detail FROM events
             WHERE time > ?1 AND kind IN ('crash', 'bsod', 'whea', 'gpuReset') ORDER BY time",
        )?;
        let rows = statement
            .query_map([since], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut found = Found::default();
        for (kind, time, detail) in rows {
            match kind.as_str() {
                "crash" => found.crashes.push(time),
                "bsod" => found.bsod_codes.push(detail),
                "whea" => found.whea += 1,
                _ => found.gpu_resets += 1,
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_reads_back_and_is_replaced() {
        let storage = Storage::in_memory().unwrap();
        assert_eq!(storage.setting("alerts.sending").unwrap(), None);
        storage.set_setting("alerts.sending", "false").unwrap();
        storage.set_setting("alerts.sending", "true").unwrap();
        assert_eq!(
            storage.setting("alerts.sending").unwrap().as_deref(),
            Some("true")
        );
    }
}
