//! The journal of the optimizer: one row per value a run changed, with what it replaced, written before the change.
//! A revert puts back the old value of each row of a run and marks the row; a marked row is never reverted again.

// Until the commands of step 11 call it, the journal is reached only from its tests
#![allow(dead_code, reason = "wired to the commands in step 11 of the 11a plan")]
#![allow(clippy::unused_self, clippy::unnecessary_wraps, reason = "stubs until step 6")]

use vitals_ipc::TweakId;

use super::{Storage, StorageError};
use crate::optimize::Change;

/// A change of the journal, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChange {
    pub row: i64,
    pub id: TweakId,
    pub change: Change,
    /// Unix seconds
    pub reverted_at: Option<i64>,
}

/// A run of the journal, its changes in the order they were written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRun {
    pub run: i64,
    /// Unix seconds
    pub time: i64,
    pub changes: Vec<StoredChange>,
}

impl Storage {
    /// Opens a run; its changes are put one by one before each is written.
    pub fn begin_optimize_run(&self, _time: i64) -> Result<i64, StorageError> {
        Ok(0)
    }

    pub fn put_optimize_change(
        &self,
        _run: i64,
        _id: TweakId,
        _change: &Change,
    ) -> Result<i64, StorageError> {
        Ok(0)
    }

    /// Forgets changes that were never written; a run left without changes goes with them.
    pub fn drop_optimize_changes(&self, _rows: &[i64]) -> Result<(), StorageError> {
        Ok(())
    }

    pub fn mark_reverted(&self, _rows: &[i64], _time: i64) -> Result<(), StorageError> {
        Ok(())
    }

    /// Every run, the newest first.
    pub fn optimize_runs(&self) -> Result<Vec<StoredRun>, StorageError> {
        Ok(Vec::new())
    }

    pub fn optimize_run(&self, _run: i64) -> Result<Option<StoredRun>, StorageError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimize::{Data, Hive, Old};

    fn registry(old: Old, new: Data) -> Change {
        Change::Registry {
            hive: Hive::User,
            key: r"Software\Microsoft\GameBar".to_owned(),
            name: "AutoGameModeEnabled".to_owned(),
            old,
            new,
        }
    }

    #[test]
    fn a_change_reads_back_with_its_old_value_and_type() {
        let storage = Storage::in_memory().unwrap();
        let run = storage.begin_optimize_run(100).unwrap();
        let changes = [
            registry(
                Old::Value {
                    data: Data::Text("1".to_owned()),
                },
                Data::Text("0".to_owned()),
            ),
            registry(Old::Absent, Data::Dword(1)),
            registry(
                Old::KeyAbsent {
                    created: r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}"
                        .to_owned(),
                },
                Data::Text(String::new()),
            ),
            Change::Scheme {
                old: "a".to_owned(),
                new: "b".to_owned(),
            },
        ];
        for change in &changes {
            storage
                .put_optimize_change(run, TweakId::GameMode, change)
                .unwrap();
        }
        let stored = storage.optimize_run(run).unwrap().unwrap();
        assert_eq!(stored.time, 100);
        let read: Vec<_> = stored
            .changes
            .iter()
            .map(|one| one.change.clone())
            .collect();
        assert_eq!(read, changes, "in the order they were put, exactly");
        assert!(stored.changes.iter().all(|one| one.reverted_at.is_none()));
    }

    #[test]
    fn a_reverted_change_stays_marked() {
        let storage = Storage::in_memory().unwrap();
        let run = storage.begin_optimize_run(1).unwrap();
        let row = storage
            .put_optimize_change(
                run,
                TweakId::GameMode,
                &registry(Old::Absent, Data::Dword(1)),
            )
            .unwrap();
        storage.mark_reverted(&[row], 5).unwrap();
        storage.mark_reverted(&[row], 9).unwrap();
        let stored = storage.optimize_run(run).unwrap().unwrap();
        assert_eq!(
            stored.changes[0].reverted_at,
            Some(5),
            "the first revert's time is kept"
        );
    }

    #[test]
    fn two_runs_on_one_value_keep_each_its_own_original() {
        let storage = Storage::in_memory().unwrap();
        let first = storage.begin_optimize_run(1).unwrap();
        storage
            .put_optimize_change(
                first,
                TweakId::GameMode,
                &registry(Old::Absent, Data::Dword(1)),
            )
            .unwrap();
        let second = storage.begin_optimize_run(2).unwrap();
        let again = registry(
            Old::Value {
                data: Data::Dword(0),
            },
            Data::Dword(1),
        );
        storage
            .put_optimize_change(second, TweakId::GameMode, &again)
            .unwrap();
        let runs = storage.optimize_runs().unwrap();
        assert_eq!(
            runs.iter().map(|run| run.run).collect::<Vec<_>>(),
            [second, first],
            "the newest first"
        );
        assert_eq!(runs[0].changes[0].change, again);
        assert_eq!(
            runs[1].changes[0].change,
            registry(Old::Absent, Data::Dword(1))
        );
    }

    #[test]
    fn dropping_every_change_of_a_run_drops_the_run() {
        let storage = Storage::in_memory().unwrap();
        let run = storage.begin_optimize_run(1).unwrap();
        let kept = storage
            .put_optimize_change(
                run,
                TweakId::GameMode,
                &registry(Old::Absent, Data::Dword(1)),
            )
            .unwrap();
        let dropped = storage
            .put_optimize_change(
                run,
                TweakId::GameMode,
                &registry(Old::Absent, Data::Dword(2)),
            )
            .unwrap();
        storage.drop_optimize_changes(&[dropped]).unwrap();
        assert_eq!(storage.optimize_run(run).unwrap().unwrap().changes.len(), 1);
        storage.drop_optimize_changes(&[kept]).unwrap();
        assert_eq!(storage.optimize_run(run).unwrap(), None);
        assert_eq!(storage.optimize_runs().unwrap(), []);
    }
}
