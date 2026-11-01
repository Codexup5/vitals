use rusqlite::Connection;

/// Each entry moves the schema one version up; an applied entry is never edited, a change is a new entry.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE events (
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
     );",
    "CREATE TABLE scans (
       id INTEGER PRIMARY KEY,
       finished_at INTEGER NOT NULL,
       body TEXT NOT NULL
     );",
    "CREATE TABLE net_samples (
       time INTEGER PRIMARY KEY,
       gateway_ms REAL,
       gateway_loss REAL,
       gateway_jitter REAL,
       internet_ms REAL,
       internet_loss REAL NOT NULL,
       internet_jitter REAL
     );",
    "CREATE TABLE devices (
       mac TEXT PRIMARY KEY,
       ip TEXT NOT NULL,
       name TEXT,
       first_seen INTEGER NOT NULL,
       last_seen INTEGER NOT NULL
     );",
    "CREATE TABLE service_changes (
       name TEXT PRIMARY KEY,
       original TEXT NOT NULL,
       changed_at INTEGER NOT NULL
     );",
    "CREATE TABLE settings (
       key TEXT PRIMARY KEY,
       value TEXT NOT NULL
     );",
    "CREATE TABLE sensor_samples (
       time INTEGER PRIMARY KEY,
       cpu_load REAL,
       cpu_mhz REAL,
       ram_used REAL,
       gpu_temp REAL,
       gpu_load REAL,
       gpu_power REAL
     );",
    "ALTER TABLE sensor_samples ADD COLUMN cpu_temp REAL;",
    "CREATE TABLE optimize_runs (
       run INTEGER PRIMARY KEY AUTOINCREMENT,
       time INTEGER NOT NULL
     );
     CREATE TABLE optimize_changes (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       run INTEGER NOT NULL REFERENCES optimize_runs (run) ON DELETE CASCADE,
       item TEXT NOT NULL,
       change TEXT NOT NULL,
       reverted_at INTEGER
     );
     CREATE INDEX optimize_changes_run ON optimize_changes (run);",
];

/// Moves the schema up one step at a time, each step under a write lock that reads the version again: the collector,
/// the network sampler and the commands open the database at once on a start, and only one of them may apply a step.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    loop {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let version = usize::try_from(version).unwrap_or(0);
        let Some(migration) = MIGRATIONS.get(version) else {
            return conn.execute_batch("COMMIT");
        };
        let applied = conn.execute_batch(&format!(
            "{migration}; PRAGMA user_version = {};",
            version + 1
        ));
        if let Err(error) = applied {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(error);
        }
        conn.execute_batch("COMMIT")?;
    }
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

    #[test]
    fn connections_that_open_at_once_apply_each_step_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vitals.db");
        let workers: Vec<_> = (0..6)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let conn = Connection::open(path).unwrap();
                    conn.busy_timeout(std::time::Duration::from_secs(10))
                        .unwrap();
                    migrate(&conn)
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap().unwrap();
        }
    }
}
