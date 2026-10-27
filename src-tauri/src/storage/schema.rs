use rusqlite::Connection;

/// Each entry moves the schema one version up; an applied entry is never edited, a change is a new entry.
const MIGRATIONS: &[&str] = &["CREATE TABLE events (
       log TEXT NOT NULL,
       record_id INTEGER NOT NULL,
       time INTEGER NOT NULL,
       day TEXT NOT NULL,
       provider TEXT NOT NULL,
       event_id INTEGER NOT NULL,
       level INTEGER NOT NULL,
       kind TEXT,
       detail TEXT,
       PRIMARY KEY (log, record_id, time)
     ) WITHOUT ROWID;
     CREATE INDEX events_day ON events (day, kind);
     CREATE INDEX events_time ON events (time);
     CREATE TABLE days (
       day TEXT PRIMARY KEY,
       crashes INTEGER NOT NULL,
       bsod INTEGER NOT NULL,
       gpu_reset INTEGER NOT NULL,
       nv_container INTEGER NOT NULL,
       whea INTEGER NOT NULL,
       iommu INTEGER NOT NULL,
       app_crashes INTEGER NOT NULL,
       errors INTEGER NOT NULL,
       top_apps TEXT NOT NULL,
       source TEXT NOT NULL CHECK (source IN ('collector', 'import'))
     );
     CREATE TABLE snapshots (
       day TEXT PRIMARY KEY,
       taken_at INTEGER,
       body TEXT NOT NULL
     );"];

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let version = usize::try_from(version).unwrap_or(0);
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version) {
        conn.execute_batch(&format!(
            "BEGIN; {migration}; PRAGMA user_version = {}; COMMIT;",
            index + 1
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_once_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(usize::try_from(version).unwrap(), MIGRATIONS.len());
    }
}
