//! The network samples: one row per five-minute measurement, a burst to the gateway and one to the internet.

use rusqlite::params;
use vitals_ipc::{NetSample, Probe};

use super::{Storage, StorageError};

fn probe_from(avg: Option<f64>, loss: Option<f64>, jitter: Option<f64>) -> Option<Probe> {
    loss.map(|loss_pct| Probe {
        avg_ms: avg,
        loss_pct,
        jitter_ms: jitter,
    })
}

impl Storage {
    pub fn put_net_sample(&self, sample: &NetSample) -> Result<(), StorageError> {
        let gateway = sample.gateway.as_ref();
        self.conn.execute(
            "INSERT OR REPLACE INTO net_samples
               (time, gateway_ms, gateway_loss, gateway_jitter, internet_ms, internet_loss, internet_jitter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                sample.time,
                gateway.and_then(|probe| probe.avg_ms),
                gateway.map(|probe| probe.loss_pct),
                gateway.and_then(|probe| probe.jitter_ms),
                sample.internet.avg_ms,
                sample.internet.loss_pct,
                sample.internet.jitter_ms,
            ],
        )?;
        Ok(())
    }

    /// The samples from `since` (Unix seconds) on, oldest first.
    pub fn net_samples_since(&self, since: i64) -> Result<Vec<NetSample>, StorageError> {
        let mut statement = self.conn.prepare_cached(
            "SELECT time, gateway_ms, gateway_loss, gateway_jitter, internet_ms, internet_loss, internet_jitter
             FROM net_samples WHERE time >= ?1 ORDER BY time",
        )?;
        Ok(statement
            .query_map([since], |row| {
                Ok(NetSample {
                    time: row.get(0)?,
                    gateway: probe_from(row.get(1)?, row.get(2)?, row.get(3)?),
                    internet: probe_from(row.get(4)?, row.get(5)?, row.get(6)?).unwrap_or(Probe {
                        avg_ms: None,
                        loss_pct: 100.0,
                        jitter_ms: None,
                    }),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Drops the samples older than `before` (Unix seconds).
    pub fn prune_net_samples(&self, before: i64) -> Result<usize, StorageError> {
        Ok(self
            .conn
            .execute("DELETE FROM net_samples WHERE time < ?1", [before])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(avg: f64, loss: f64) -> Probe {
        Probe {
            avg_ms: Some(avg),
            loss_pct: loss,
            jitter_ms: Some(1.5),
        }
    }

    #[test]
    fn a_sample_reads_back_and_old_ones_go() {
        let storage = Storage::in_memory().unwrap();
        let old = NetSample {
            time: 100,
            gateway: None,
            internet: probe(30.0, 0.0),
        };
        let new = NetSample {
            time: 400,
            gateway: Some(probe(1.0, 10.0)),
            internet: Probe {
                avg_ms: None,
                loss_pct: 100.0,
                jitter_ms: None,
            },
        };
        storage.put_net_sample(&old).unwrap();
        storage.put_net_sample(&new).unwrap();
        assert_eq!(storage.net_samples_since(0).unwrap(), [old, new]);
        assert_eq!(storage.prune_net_samples(200).unwrap(), 1);
        assert_eq!(storage.net_samples_since(0).unwrap(), [new]);
    }
}
