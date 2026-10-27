//! The network samples: one row per five-minute measurement, a burst to the gateway and one to the internet.

use rusqlite::params;
use vitals_ipc::{NetSample, Probe};

use super::{Storage, StorageError};
use crate::network::devices::{Known, Seen, mac_text};

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

impl Storage {
    /// Records what a scan saw: a new device gets its first sighting, a known one its new address and time; a
    /// name that did not resolve this time keeps the last one.
    pub fn put_seen(&mut self, seen: &[Seen], now: i64) -> Result<(), StorageError> {
        let transaction = self.conn.transaction()?;
        {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO devices (mac, ip, name, first_seen, last_seen) VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT (mac) DO UPDATE SET
                   ip = excluded.ip, name = COALESCE(excluded.name, devices.name), last_seen = excluded.last_seen",
            )?;
            for device in seen {
                statement.execute(params![
                    mac_text(device.mac),
                    device.ip.to_string(),
                    device.name,
                    now
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn known_devices(&self) -> Result<Vec<Known>, StorageError> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT mac, ip, name, first_seen, last_seen FROM devices")?;
        Ok(statement
            .query_map([], |row| {
                Ok(Known {
                    mac: row.get(0)?,
                    ip: row.get(1)?,
                    name: row.get(2)?,
                    first_seen: row.get(3)?,
                    last_seen: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn a_device_keeps_its_first_sighting_and_its_last_name() {
        let mut storage = Storage::in_memory().unwrap();
        let mac = [0x04, 0xcb, 0x01, 0xfd, 0x57, 0x68];
        let named = Seen {
            mac,
            ip: Ipv4Addr::new(192, 168, 10, 3),
            name: Some("tv".to_owned()),
        };
        storage.put_seen(&[named], 100).unwrap();
        let moved = Seen {
            mac,
            ip: Ipv4Addr::new(192, 168, 10, 4),
            name: None,
        };
        storage.put_seen(&[moved], 200).unwrap();
        let known = storage.known_devices().unwrap();
        assert_eq!(known.len(), 1);
        assert_eq!(
            (
                known[0].ip.as_str(),
                known[0].name.as_deref(),
                known[0].first_seen,
                known[0].last_seen
            ),
            ("192.168.10.4", Some("tv"), 100, 200)
        );
    }

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
