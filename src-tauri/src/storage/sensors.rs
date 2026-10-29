//! The history of the sensors: one row per five-minute sample, a month of them.

use rusqlite::{OptionalExtension, params};
use vitals_ipc::SensorPoint;

use super::{Storage, StorageError};

const COLUMNS: &str = "time, cpu_load, cpu_mhz, ram_used, gpu_temp, gpu_load, gpu_power, cpu_temp";

fn point_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<SensorPoint> {
    Ok(SensorPoint {
        time: row.get(0)?,
        cpu_load_pct: row.get(1)?,
        cpu_mhz: row.get(2)?,
        ram_used_pct: row.get(3)?,
        gpu_temp_c: row.get(4)?,
        gpu_load_pct: row.get(5)?,
        gpu_power_w: row.get(6)?,
        cpu_temp_c: row.get(7)?,
    })
}

impl Storage {
    pub fn put_sensor_sample(&self, point: &SensorPoint) -> Result<(), StorageError> {
        self.conn.execute(
            &format!("INSERT OR REPLACE INTO sensor_samples ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
            params![
                point.time,
                point.cpu_load_pct,
                point.cpu_mhz,
                point.ram_used_pct,
                point.gpu_temp_c,
                point.gpu_load_pct,
                point.gpu_power_w,
                point.cpu_temp_c,
            ],
        )?;
        Ok(())
    }

    /// The samples from `since` (Unix seconds) on, oldest first.
    pub fn sensor_samples_since(&self, since: i64) -> Result<Vec<SensorPoint>, StorageError> {
        let mut statement = self.conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM sensor_samples WHERE time >= ?1 ORDER BY time"
        ))?;
        Ok(statement
            .query_map([since], point_from)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn latest_sensor_sample(&self) -> Result<Option<SensorPoint>, StorageError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM sensor_samples ORDER BY time DESC LIMIT 1"),
                [],
                point_from,
            )
            .optional()?)
    }

    /// Drops the samples older than `before` (Unix seconds).
    pub fn prune_sensor_samples(&self, before: i64) -> Result<usize, StorageError> {
        Ok(self
            .conn
            .execute("DELETE FROM sensor_samples WHERE time < ?1", [before])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(time: i64, temp_c: Option<f64>) -> SensorPoint {
        SensorPoint {
            time,
            cpu_load_pct: Some(12.5),
            cpu_mhz: Some(4950.0),
            ram_used_pct: Some(25.0),
            gpu_temp_c: temp_c,
            gpu_load_pct: temp_c.map(|_| 20.0),
            gpu_power_w: None,
            cpu_temp_c: temp_c.map(|temp| temp + 20.0),
        }
    }

    #[test]
    fn a_sample_reads_back_and_old_ones_go() {
        let storage = Storage::in_memory().unwrap();
        assert_eq!(storage.latest_sensor_sample().unwrap(), None);
        let old = point(100, None);
        let new = point(400, Some(41.0));
        storage.put_sensor_sample(&new).unwrap();
        storage.put_sensor_sample(&old).unwrap();
        assert_eq!(storage.sensor_samples_since(0).unwrap(), [old, new]);
        assert_eq!(storage.latest_sensor_sample().unwrap(), Some(new));
        assert_eq!(storage.prune_sensor_samples(200).unwrap(), 1);
        assert_eq!(storage.sensor_samples_since(0).unwrap(), [new]);
    }
}
