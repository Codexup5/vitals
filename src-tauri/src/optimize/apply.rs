//! The order of a run, which makes "No" at the UAC prompt safe: every change journaled first, then the admin items
//! under one UAC (the restore point before them), then the user items. A declined prompt stops before any user write
//! and forgets the run, so "No" changes nothing at all. A revert puts back a run's changes in reverse order.

use std::collections::HashSet;

use vitals_ipc::{
    ApplyOutcome, ChangeOutcome, Effect, HardwareFacts, ItemOutcome, ItemResult, PreviewArgs,
    RestorePoint, TweakId,
};

use super::catalog::tweak;
use super::elevated::{ElevatedReport, ElevatedRequest};
use super::plan::plan_of;
use super::{Change, Hive, OptimizeError, Put, Reader, RegWrite};
use crate::storage::Storage;

/// Why the elevated half of a run did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elevation {
    /// The user said no at the prompt
    Declined,
    /// The elevated process never started: nothing was written
    Failed(String),
    /// It started and its report did not come back: some writes may have happened
    Unknown(String),
}

/// What a run does to the machine; Windows implements it, the tests fake it.
pub trait Machine: Reader {
    /// A write of the normal process: HKCU only
    fn write(&mut self, write: &RegWrite) -> Result<(), String>;
    fn set_scheme(&mut self, guid: &str) -> Result<(), String>;
    /// The admin writes under one UAC
    fn elevate(&mut self, request: &ElevatedRequest) -> Result<ElevatedReport, Elevation>;
    /// What a setting needs after its values change: `SPI_SETMOUSE` for the pointer
    fn refresh(&mut self, id: TweakId);
}

/// One write of a run, tied to its journal row.
struct Step {
    row: i64,
    id: TweakId,
    action: Action,
}

enum Action {
    Registry(RegWrite),
    Scheme(String),
}

impl Step {
    fn is_admin(&self) -> bool {
        matches!(&self.action, Action::Registry(write) if write.hive == Hive::Machine)
    }
}

/// What the steps came to: the rows written, the rows not written, why an item failed.
#[derive(Default)]
struct Executed {
    done: Vec<i64>,
    skipped: Vec<i64>,
    failed: Vec<(TweakId, String)>,
    restore_point: Option<RestorePoint>,
}

/// The admin steps under one UAC, then the user steps; an item stops at its first failed write.
fn execute(
    machine: &mut dyn Machine,
    steps: &[Step],
    restore_point: bool,
    enable_protection: bool,
) -> Result<Executed, ()> {
    let mut executed = Executed::default();
    let (admin, user): (Vec<&Step>, Vec<&Step>) = steps.iter().partition(|step| step.is_admin());
    if !admin.is_empty() {
        let request = ElevatedRequest {
            writes: admin
                .iter()
                .filter_map(|step| match &step.action {
                    Action::Registry(write) => Some(write.clone()),
                    Action::Scheme(_) => None,
                })
                .collect(),
            restore_point,
            enable_protection,
        };
        match machine.elevate(&request) {
            Err(Elevation::Declined) => return Err(()),
            Err(Elevation::Failed(message)) => {
                for step in &admin {
                    executed.failed.push((step.id, message.clone()));
                    executed.skipped.push(step.row);
                }
            }
            // The rows stay: reverting a change that was never written puts back what is already there
            Err(Elevation::Unknown(message)) => {
                for step in &admin {
                    executed.failed.push((step.id, message.clone()));
                }
            }
            Ok(report) => {
                executed.restore_point = report.restore_point;
                let mut results = report.results.into_iter();
                for step in &admin {
                    match results.next() {
                        Some(Ok(())) => executed.done.push(step.row),
                        Some(Err(message)) => {
                            executed.failed.push((step.id, message));
                            executed.skipped.push(step.row);
                        }
                        None => {
                            executed.failed.push((step.id, "no report".to_owned()));
                            executed.skipped.push(step.row);
                        }
                    }
                }
            }
        }
    }
    let mut failed_items = HashSet::new();
    let mut touched = Vec::new();
    for step in user {
        if failed_items.contains(&step.id) {
            executed.skipped.push(step.row);
            continue;
        }
        let result = match &step.action {
            Action::Registry(write) => machine.write(write),
            Action::Scheme(guid) => machine.set_scheme(guid),
        };
        match result {
            Ok(()) => {
                executed.done.push(step.row);
                if !touched.contains(&step.id) {
                    touched.push(step.id);
                }
            }
            Err(message) => {
                failed_items.insert(step.id);
                executed.failed.push((step.id, message));
                executed.skipped.push(step.row);
            }
        }
    }
    for id in touched {
        machine.refresh(id);
    }
    Ok(executed)
}

fn forward(id: TweakId, change: &Change) -> Action {
    match change {
        Change::Registry {
            hive,
            key,
            name,
            new,
            ..
        } => Action::Registry(RegWrite {
            id,
            hive: *hive,
            key: key.clone(),
            name: name.clone(),
            put: Put::Data { data: new.clone() },
        }),
        Change::Scheme { new, .. } => Action::Scheme(new.clone()),
    }
}

fn backward(id: TweakId, change: &Change) -> Action {
    match change {
        Change::Registry {
            hive,
            key,
            name,
            old,
            ..
        } => Action::Registry(RegWrite {
            id,
            hive: *hive,
            key: key.clone(),
            name: name.clone(),
            put: Put::restoring(old),
        }),
        Change::Scheme { old, .. } => Action::Scheme(old.clone()),
    }
}

fn outcome_of(steps: &[Step], executed: Executed, run: Option<i64>) -> ApplyOutcome {
    let mut ids: Vec<TweakId> = Vec::new();
    for step in steps {
        if !ids.contains(&step.id) {
            ids.push(step.id);
        }
    }
    let items: Vec<ItemOutcome> = ids
        .into_iter()
        .map(|id| {
            let failure = executed.failed.iter().find(|(failed, _)| *failed == id);
            ItemOutcome {
                id,
                result: failure.map_or(ItemResult::Done, |(_, message)| ItemResult::Failed {
                    message: message.clone(),
                }),
            }
        })
        .collect();
    let mut effects: Vec<Effect> = items
        .iter()
        .filter(|item| item.result == ItemResult::Done)
        .filter_map(|item| tweak(item.id).map(|tweak| tweak.effect))
        .filter(|effect| *effect != Effect::Now)
        .collect();
    effects.sort_unstable();
    effects.dedup();
    ApplyOutcome {
        outcome: ChangeOutcome::Done,
        run,
        items,
        restore_point: executed.restore_point,
        effects,
    }
}

const fn untouched(outcome: ChangeOutcome) -> ApplyOutcome {
    ApplyOutcome {
        outcome,
        run: None,
        items: Vec::new(),
        restore_point: None,
        effects: Vec::new(),
    }
}

/// Applies the checked settings: the plan is made again from the machine as it is now.
pub fn apply(
    machine: &mut dyn Machine,
    storage: &Storage,
    facts: &HardwareFacts,
    args: &PreviewArgs,
    now: i64,
) -> Result<ApplyOutcome, OptimizeError> {
    let plan = plan_of(machine, facts, &args.checked);
    if plan.is_empty() {
        return Ok(untouched(ChangeOutcome::Done));
    }
    // Every old value is in the journal before anything is written
    let run = storage.begin_optimize_run(now)?;
    let mut steps = Vec::new();
    for item in &plan.items {
        for change in &item.changes {
            let row = storage.put_optimize_change(run, item.id, change)?;
            steps.push(Step {
                row,
                id: item.id,
                action: forward(item.id, change),
            });
        }
    }
    let Ok(executed) = execute(machine, &steps, true, args.enable_protection) else {
        let rows: Vec<i64> = steps.iter().map(|step| step.row).collect();
        storage.drop_optimize_changes(&rows)?;
        return Ok(untouched(ChangeOutcome::Declined));
    };
    storage.drop_optimize_changes(&executed.skipped)?;
    let run = storage.optimize_run(run)?.map(|stored| stored.run);
    Ok(outcome_of(&steps, executed, run))
}

/// Puts back what a run changed and is not reverted yet, the last change first.
pub fn revert(
    machine: &mut dyn Machine,
    storage: &Storage,
    run: i64,
    now: i64,
) -> Result<ApplyOutcome, OptimizeError> {
    let stored = storage
        .optimize_run(run)?
        .ok_or_else(|| OptimizeError::Refused(format!("the journal has no run {run}")))?;
    let steps: Vec<Step> = stored
        .changes
        .iter()
        .rev()
        .filter(|change| change.reverted_at.is_none())
        .map(|change| Step {
            row: change.row,
            id: change.id,
            action: backward(change.id, &change.change),
        })
        .collect();
    if steps.is_empty() {
        return Ok(untouched(ChangeOutcome::Done));
    }
    let Ok(executed) = execute(machine, &steps, false, false) else {
        return Ok(untouched(ChangeOutcome::Declined));
    };
    storage.mark_reverted(&executed.done, now)?;
    Ok(outcome_of(&steps, executed, Some(run)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use vitals_ipc::{ChangeOutcome, Effect, ItemResult, RestorePoint};

    use super::super::plan::tests::{FakeReader, olegpc};
    use super::super::plan::{BALANCED, HIGH_PERFORMANCE};
    use super::super::{Data, Hive, KeyAbsent, Location, Put};
    use super::*;

    const ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
    const MOUSE: &str = r"Control Panel\Mouse";
    const CONTEXT_MENU: &str =
        r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Prompt {
        Yes,
        No,
        Broken,
        Lost,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Event {
        Write(Hive, String),
        Scheme(String),
        Elevate(Vec<RegWrite>),
        Refresh(TweakId),
    }

    /// A machine in memory that checks, at every write, that the journal already holds it.
    struct FakeMachine<'a> {
        reader: FakeReader,
        storage: &'a Storage,
        prompt: Prompt,
        events: Vec<Event>,
        failing: Option<&'static str>,
    }

    impl<'a> FakeMachine<'a> {
        fn new(reader: FakeReader, storage: &'a Storage) -> Self {
            Self {
                reader,
                storage,
                prompt: Prompt::Yes,
                events: Vec::new(),
                failing: None,
            }
        }

        fn journaled(&self, key: &str, name: &str) -> bool {
            self.storage.optimize_runs().unwrap().iter().any(|run| {
                run.changes.iter().any(|one| {
                    matches!(&one.change, super::super::Change::Registry { key: k, name: n, .. }
                        if k.eq_ignore_ascii_case(key) && n.eq_ignore_ascii_case(name))
                })
            })
        }

        fn put(&mut self, write: &RegWrite) {
            let key = write.key.to_ascii_lowercase();
            match &write.put {
                Put::Data { data } => {
                    self.reader
                        .set(write.hive, &write.key, &write.name, data.clone());
                }
                Put::DeleteValue => {
                    if let Some(values) = self.reader.keys.get_mut(&(write.hive, key)) {
                        values.remove(&write.name.to_ascii_lowercase());
                    }
                }
                Put::DeleteKey { key: created } => {
                    let created = created.to_ascii_lowercase();
                    self.reader.keys.retain(|(hive, key), _| {
                        *hive != write.hive
                            || !(*key == created || key.starts_with(&format!("{created}\\")))
                    });
                }
            }
        }

        fn writes(&self) -> usize {
            self.events
                .iter()
                .filter(|event| !matches!(event, Event::Refresh(_)))
                .count()
        }
    }

    impl Reader for FakeMachine<'_> {
        fn value(&self, at: &Location) -> Result<Option<Data>, KeyAbsent> {
            self.reader.value(at)
        }

        fn scheme(&self) -> Option<String> {
            self.reader.scheme()
        }

        fn scheme_name(&self, guid: &str) -> Option<String> {
            self.reader.scheme_name(guid)
        }
    }

    impl Machine for FakeMachine<'_> {
        fn write(&mut self, write: &RegWrite) -> Result<(), String> {
            assert_eq!(
                write.hive,
                Hive::User,
                "the normal process writes HKCU only"
            );
            assert!(
                self.journaled(&write.key, &write.name),
                "{} written before it was journaled",
                write.name
            );
            if self.failing == Some(write.name.as_str()) {
                return Err("access denied".to_owned());
            }
            self.events
                .push(Event::Write(write.hive, write.name.clone()));
            self.put(write);
            Ok(())
        }

        fn set_scheme(&mut self, guid: &str) -> Result<(), String> {
            self.events.push(Event::Scheme(guid.to_owned()));
            self.reader.scheme = Some(guid.to_owned());
            Ok(())
        }

        fn elevate(&mut self, request: &ElevatedRequest) -> Result<ElevatedReport, Elevation> {
            for write in &request.writes {
                assert!(
                    self.journaled(&write.key, &write.name),
                    "elevated before the journal"
                );
            }
            self.events.push(Event::Elevate(request.writes.clone()));
            match self.prompt {
                Prompt::No => return Err(Elevation::Declined),
                Prompt::Broken => return Err(Elevation::Failed("could not start".to_owned())),
                Prompt::Lost => return Err(Elevation::Unknown("exit code 1".to_owned())),
                Prompt::Yes => {}
            }
            for write in &request.writes {
                self.put(write);
            }
            Ok(ElevatedReport {
                restore_point: request
                    .restore_point
                    .then_some(RestorePoint::Created { time: 9 }),
                results: request.writes.iter().map(|_| Ok(())).collect(),
            })
        }

        fn refresh(&mut self, id: TweakId) {
            self.events.push(Event::Refresh(id));
        }
    }

    fn checked(ids: &[TweakId]) -> PreviewArgs {
        PreviewArgs {
            checked: ids.to_vec(),
            enable_protection: false,
        }
    }

    /// OLEGPC before: extensions hidden, the mouse accelerated, no context menu key, no long paths, High performance
    fn before() -> FakeReader {
        let mut reader = FakeReader::with_catalog_keys();
        reader.set(Hive::User, ADVANCED, "HideFileExt", Data::Dword(1));
        for name in ["MouseSpeed", "MouseThreshold1", "MouseThreshold2"] {
            reader.set(Hive::User, MOUSE, name, Data::Text("6".to_owned()));
        }
        reader
            .keys
            .retain(|(_, key), _| *key != CONTEXT_MENU.to_ascii_lowercase());
        reader.scheme = Some(HIGH_PERFORMANCE.to_owned());
        reader
    }

    type Snapshot = (
        HashMap<(Hive, String), HashMap<String, Data>>,
        Option<String>,
    );

    fn snapshot(machine: &FakeMachine) -> Snapshot {
        (machine.reader.keys.clone(), machine.reader.scheme.clone())
    }

    const MIXED: [TweakId; 6] = [
        TweakId::FileExtensions,
        TweakId::GameMode,
        TweakId::ClassicContextMenu,
        TweakId::PointerPrecisionOff,
        TweakId::PowerScheme,
        TweakId::LongPaths,
    ];

    #[test]
    fn a_run_without_an_admin_item_never_elevates() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        let outcome = apply(
            &mut machine,
            &storage,
            &olegpc(),
            &checked(&[TweakId::FileExtensions, TweakId::GameMode]),
            100,
        )
        .unwrap();
        assert_eq!(outcome.outcome, ChangeOutcome::Done);
        assert!(
            !machine
                .events
                .iter()
                .any(|event| matches!(event, Event::Elevate(_)))
        );
        assert_eq!(outcome.restore_point, None);
        assert_eq!(outcome.effects, [Effect::ExplorerRestart]);
        assert!(outcome.run.is_some());
    }

    #[test]
    fn one_elevation_holds_every_admin_write_and_no_hkcu_one() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        let outcome = apply(
            &mut machine,
            &storage,
            &olegpc(),
            &checked(&[TweakId::LongPaths, TweakId::Hags, TweakId::GameMode]),
            100,
        )
        .unwrap();
        let elevations: Vec<_> = machine
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Elevate(writes) => Some(writes.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(elevations.len(), 1, "one UAC for the run");
        assert_eq!(elevations[0].len(), 2);
        assert!(
            elevations[0]
                .iter()
                .all(|write| write.hive == Hive::Machine)
        );
        assert!(
            matches!(machine.events[0], Event::Elevate(_)),
            "the admin half comes before any user write"
        );
        assert_eq!(
            outcome.restore_point,
            Some(RestorePoint::Created { time: 9 })
        );
        assert!(
            outcome
                .items
                .iter()
                .all(|item| item.result == ItemResult::Done)
        );
    }

    #[test]
    fn no_at_the_prompt_changes_nothing_at_all() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        machine.prompt = Prompt::No;
        let was = snapshot(&machine);
        let outcome = apply(&mut machine, &storage, &olegpc(), &checked(&MIXED), 100).unwrap();
        assert_eq!(outcome.outcome, ChangeOutcome::Declined);
        assert_eq!(outcome.run, None);
        assert_eq!(snapshot(&machine), was, "user items included");
        assert_eq!(machine.writes(), 1, "only the declined elevation");
        assert_eq!(storage.optimize_runs().unwrap(), [], "the run is forgotten");
    }

    #[test]
    fn a_revert_brings_back_every_value_and_a_second_one_changes_nothing() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        let was = snapshot(&machine);
        let outcome = apply(&mut machine, &storage, &olegpc(), &checked(&MIXED), 100).unwrap();
        assert_eq!(outcome.items.len(), MIXED.len());
        assert_ne!(snapshot(&machine), was);
        assert_eq!(machine.reader.scheme.as_deref(), Some(BALANCED));
        assert!(
            machine
                .events
                .contains(&Event::Refresh(TweakId::PointerPrecisionOff))
        );
        let run = outcome.run.unwrap();

        machine.events.clear();
        let back = revert(&mut machine, &storage, run, 200).unwrap();
        assert_eq!(back.outcome, ChangeOutcome::Done);
        assert_eq!(
            snapshot(&machine),
            was,
            "values, types, absent values and created keys"
        );
        assert!(
            machine
                .events
                .contains(&Event::Refresh(TweakId::PointerPrecisionOff))
        );
        assert!(
            storage
                .optimize_run(run)
                .unwrap()
                .unwrap()
                .changes
                .iter()
                .all(|one| one.reverted_at == Some(200))
        );

        machine.events.clear();
        let again = revert(&mut machine, &storage, run, 300).unwrap();
        assert_eq!(again.items, []);
        assert_eq!(machine.writes(), 0, "a second revert writes nothing");
        assert_eq!(snapshot(&machine), was);
    }

    #[test]
    fn a_revert_goes_backwards() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        let run = apply(
            &mut machine,
            &storage,
            &olegpc(),
            &checked(&[TweakId::StartRecommendations]),
            100,
        )
        .unwrap()
        .run
        .unwrap();
        machine.events.clear();
        revert(&mut machine, &storage, run, 200).unwrap();
        let names: Vec<_> = machine
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Write(_, name) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, ["Start_TrackDocs", "Start_IrisRecommendations"]);
    }

    #[test]
    fn a_failed_write_fails_its_item_and_leaves_no_journal_row() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        machine.failing = Some("AutoGameModeEnabled");
        let outcome = apply(
            &mut machine,
            &storage,
            &olegpc(),
            &checked(&[TweakId::FileExtensions, TweakId::GameMode]),
            100,
        )
        .unwrap();
        let result_of = |id| {
            outcome
                .items
                .iter()
                .find(|item| item.id == id)
                .unwrap()
                .result
                .clone()
        };
        assert_eq!(result_of(TweakId::FileExtensions), ItemResult::Done);
        assert!(matches!(
            result_of(TweakId::GameMode),
            ItemResult::Failed { .. }
        ));
        let run = storage.optimize_run(outcome.run.unwrap()).unwrap().unwrap();
        assert!(
            run.changes
                .iter()
                .all(|one| one.id == TweakId::FileExtensions)
        );
    }

    #[test]
    fn a_broken_elevation_fails_the_admin_items_and_keeps_the_user_ones() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        machine.prompt = Prompt::Broken;
        let outcome = apply(
            &mut machine,
            &storage,
            &olegpc(),
            &checked(&[TweakId::LongPaths, TweakId::GameMode]),
            100,
        )
        .unwrap();
        assert_eq!(outcome.outcome, ChangeOutcome::Done);
        let run = storage.optimize_run(outcome.run.unwrap()).unwrap().unwrap();
        assert!(run.changes.iter().all(|one| one.id == TweakId::GameMode));
        assert!(matches!(
            outcome
                .items
                .iter()
                .find(|item| item.id == TweakId::LongPaths)
                .unwrap()
                .result,
            ItemResult::Failed { .. }
        ));
    }

    #[test]
    fn a_lost_elevation_keeps_its_journal_rows() {
        let storage = Storage::in_memory().unwrap();
        let mut machine = FakeMachine::new(before(), &storage);
        machine.prompt = Prompt::Lost;
        let outcome = apply(
            &mut machine,
            &storage,
            &olegpc(),
            &checked(&[TweakId::LongPaths, TweakId::GameMode]),
            100,
        )
        .unwrap();
        let run = storage.optimize_run(outcome.run.unwrap()).unwrap().unwrap();
        assert!(
            run.changes.iter().any(|one| one.id == TweakId::LongPaths),
            "the admin write may have happened: its revert must stay possible"
        );
        assert!(matches!(
            outcome
                .items
                .iter()
                .find(|item| item.id == TweakId::LongPaths)
                .unwrap()
                .result,
            ItemResult::Failed { .. }
        ));
    }

    #[test]
    fn nothing_to_do_opens_no_run() {
        let storage = Storage::in_memory().unwrap();
        let mut reader = before();
        reader.set(Hive::User, ADVANCED, "HideFileExt", Data::Dword(0));
        let mut machine = FakeMachine::new(reader, &storage);
        let outcome = apply(
            &mut machine,
            &storage,
            &olegpc(),
            &checked(&[TweakId::FileExtensions]),
            100,
        )
        .unwrap();
        assert_eq!(outcome.run, None);
        assert_eq!(machine.writes(), 0);
        assert_eq!(storage.optimize_runs().unwrap(), []);
    }
}
