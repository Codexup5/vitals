//! The start types the panel changed: one row per service it turned to manual, with the type it had.

use std::collections::HashMap;

use rusqlite::params;
use vitals_ipc::StartType;

use super::{Storage, StorageError};

impl Storage {
    /// Saves what a service had before the panel changes it; a second change keeps the first original.
    pub fn put_service_change(
        &self,
        name: &str,
        original: StartType,
        changed_at: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO service_changes (name, original, changed_at) VALUES (?1, ?2, ?3)",
            params![name, serde_json::to_string(&original)?, changed_at],
        )?;
        Ok(())
    }

    pub fn remove_service_change(&self, name: &str) -> Result<(), StorageError> {
        self.conn
            .execute("DELETE FROM service_changes WHERE name = ?1", [name])?;
        Ok(())
    }

    /// The original start type of every service the panel changed.
    pub fn service_changes(&self) -> Result<HashMap<String, StartType>, StorageError> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT name, original FROM service_changes")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(name, original)| Ok((name, serde_json::from_str(&original)?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_original_is_kept_until_it_is_restored() {
        let storage = Storage::in_memory().unwrap();
        storage
            .put_service_change("svc", StartType::AutomaticDelayed, 1)
            .unwrap();
        storage
            .put_service_change("svc", StartType::Manual, 2)
            .unwrap();
        assert_eq!(
            storage.service_changes().unwrap().get("svc"),
            Some(&StartType::AutomaticDelayed)
        );
        storage.remove_service_change("svc").unwrap();
        assert!(storage.service_changes().unwrap().is_empty());
    }
}
