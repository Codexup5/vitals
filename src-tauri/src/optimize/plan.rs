//! The plan of a run: the catalog, the machine's facts and its current values, the user's checks → every value a run
//! would change with what it replaces, admin items apart from user items, settings that already hold apart.

use vitals_ipc::{
    Condition, CpuVendor, Effect, Focus, HardwareFacts, OptimizePlan, PlanItem, Rights, TweakId,
    TweakRow, TweakState, ValueChange,
};

use super::catalog::catalog;
use super::{Change, Data, KeyAbsent, Location, Old, Reader, Tweak, Write};

/// `powercfg /list`: the schemes every Windows has, by GUID.
pub const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub const HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

/// The name a power scheme change shows under
const SCHEME_NAME: &str = "powercfg";

/// The scheme for this hardware: Balanced on AMD and on a laptop, High performance on an Intel desktop.
pub fn power_target(facts: &HardwareFacts) -> &'static str {
    if facts.cpu_vendor == CpuVendor::Intel && !facts.is_laptop {
        HIGH_PERFORMANCE
    } else {
        BALANCED
    }
}

/// The condition this machine does not meet for a setting.
pub fn unmet(tweak: &Tweak, facts: &HardwareFacts) -> Option<Condition> {
    let needs_high_performance =
        tweak.writes.contains(&Write::PowerScheme) && power_target(facts) == HIGH_PERFORMANCE;
    tweak
        .condition
        .or_else(|| needs_high_performance.then_some(Condition::HighPerformanceScheme))
        .filter(|condition| !facts.met.contains(condition))
}

/// Merges `name=value;` into a string of such pairs, keeping the other pairs and their order.
pub fn merge_pair(current: &str, name: &str, value: &str) -> String {
    let mut pairs: Vec<String> = current
        .split(';')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .map(str::to_owned)
        .collect();
    let entry = format!("{name}={value}");
    match pairs
        .iter_mut()
        .find(|pair| pair.split('=').next() == Some(name))
    {
        Some(pair) => *pair = entry,
        None => pairs.push(entry),
    }
    pairs.iter().flat_map(|pair| [pair.as_str(), ";"]).collect()
}

/// The settings a machine is offered for the chosen focuses (the base always), and the ones hidden with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Offered {
    pub shown: Vec<TweakId>,
    pub hidden: Vec<(TweakId, Condition)>,
}

pub fn offered(facts: &HardwareFacts, focuses: &[Focus]) -> Offered {
    let mut offered = Offered::default();
    let chosen = catalog()
        .iter()
        .filter(|tweak| tweak.focus == Focus::Base || focuses.contains(&tweak.focus));
    for tweak in chosen {
        match unmet(tweak, facts) {
            Some(condition) => offered.hidden.push((tweak.id, condition)),
            None => offered.shown.push(tweak.id),
        }
    }
    offered
}

/// What each write of a setting would do on this machine, holding or not. An unknown active scheme is an empty
/// `old`: such a change cannot be reverted, so it is never planned.
fn changes_of(tweak: &Tweak, reader: &dyn Reader, facts: &HardwareFacts) -> Vec<Change> {
    tweak
        .writes
        .iter()
        .map(|write| match *write {
            Write::Value(at, data) => registry(&at, old_of(reader.value(&at)), data.data()),
            Write::Pair { at, pair, value } => {
                let old = old_of(reader.value(&at));
                let current = match &old {
                    Old::Value {
                        data: Data::Text(text),
                    } => text.as_str(),
                    _ => "",
                };
                let new = Data::Text(merge_pair(current, pair, value));
                registry(&at, old, new)
            }
            Write::PowerScheme => Change::Scheme {
                old: reader.scheme().unwrap_or_default(),
                new: power_target(facts).to_owned(),
            },
        })
        .collect()
}

fn old_of(read: Result<Option<Data>, KeyAbsent>) -> Old {
    match read {
        Err(KeyAbsent { created }) => Old::KeyAbsent { created },
        Ok(None) => Old::Absent,
        Ok(Some(data)) => Old::Value { data },
    }
}

fn registry(at: &Location, old: Old, new: Data) -> Change {
    Change::Registry {
        hive: at.hive,
        key: at.key.to_owned(),
        name: at.name.to_owned(),
        old,
        new,
    }
}

/// The machine already has what the change would write
fn holds(change: &Change) -> bool {
    match change {
        Change::Registry { old, new, .. } => matches!(old, Old::Value { data } if data == new),
        Change::Scheme { old, new } => old == new,
    }
}

/// Every setting of the catalog as it stands on this machine.
pub fn rows_of(reader: &dyn Reader, facts: &HardwareFacts) -> Vec<TweakRow> {
    catalog()
        .iter()
        .map(|tweak| {
            let changes = changes_of(tweak, reader, facts);
            let state = if changes.iter().all(holds) {
                TweakState::Done
            } else {
                TweakState::Todo
            };
            TweakRow {
                id: tweak.id,
                focus: tweak.focus,
                rights: tweak.rights(),
                effect: tweak.effect,
                state,
                is_default: tweak.is_default,
                unmet: unmet(tweak, facts),
                values: changes.iter().map(|change| shown(change, reader)).collect(),
            }
        })
        .collect()
}

/// A setting a run changes and the changes it makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub id: TweakId,
    pub rights: Rights,
    pub effect: Effect,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// Admin items first, then user items, each in catalog order
    pub items: Vec<Planned>,
    pub already_done: Vec<TweakId>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// What the window shows: "was → becomes" for every value, admin and user apart
    pub fn to_ipc(&self, reader: &dyn Reader) -> OptimizePlan {
        let of = |rights| {
            self.items
                .iter()
                .filter(|item| item.rights == rights)
                .map(|item| PlanItem {
                    id: item.id,
                    effect: item.effect,
                    changes: item
                        .changes
                        .iter()
                        .map(|change| shown(change, reader))
                        .collect(),
                })
                .collect()
        };
        OptimizePlan {
            admin: of(Rights::Admin),
            user: of(Rights::User),
            already_done: self.already_done.clone(),
        }
    }
}

/// The plan of the checked settings; an unknown, hidden or unchecked setting is not in it.
pub fn plan_of(reader: &dyn Reader, facts: &HardwareFacts, checked: &[TweakId]) -> Plan {
    let mut plan = Plan::default();
    let offered = catalog()
        .iter()
        .filter(|tweak| checked.contains(&tweak.id) && unmet(tweak, facts).is_none());
    for tweak in offered {
        let changes: Vec<_> = changes_of(tweak, reader, facts)
            .into_iter()
            .filter(|change| !holds(change))
            .collect();
        let is_revertible = changes
            .iter()
            .all(|change| !matches!(change, Change::Scheme { old, .. } if old.is_empty()));
        if changes.is_empty() {
            plan.already_done.push(tweak.id);
        } else if is_revertible {
            plan.items.push(Planned {
                id: tweak.id,
                rights: tweak.rights(),
                effect: tweak.effect,
                changes,
            });
        }
    }
    plan.items.sort_by_key(|item| item.rights != Rights::Admin);
    plan
}

/// A change as the window shows it.
pub fn shown(change: &Change, reader: &dyn Reader) -> ValueChange {
    match change {
        Change::Registry {
            hive,
            key,
            name,
            old,
            new,
        } => ValueChange {
            name: format!(
                "{}\\{key}\\{}",
                hive.prefix(),
                if name.is_empty() { "@" } else { name }
            ),
            was: match old {
                Old::Value { data } => Some(data.display()),
                Old::Absent | Old::KeyAbsent { .. } => None,
            },
            becomes: Some(new.display()),
        },
        Change::Scheme { old, new } => {
            let name_of = |guid: &str| {
                (!guid.is_empty())
                    .then(|| reader.scheme_name(guid).unwrap_or_else(|| guid.to_owned()))
            };
            ValueChange {
                name: SCHEME_NAME.to_owned(),
                was: name_of(old),
                becomes: name_of(new),
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashMap;

    use vitals_ipc::{CpuVendor, TweakState};

    use super::super::catalog::catalog;
    use super::super::{Data, Hive, KeyAbsent, Location, Old};
    use super::*;

    /// A machine in memory: values by hive, key and name; a key not listed is absent.
    #[derive(Debug, Clone, Default)]
    pub struct FakeReader {
        pub keys: HashMap<(Hive, String), HashMap<String, Data>>,
        pub scheme: Option<String>,
    }

    impl FakeReader {
        pub fn set(&mut self, hive: Hive, key: &str, name: &str, data: Data) {
            self.keys
                .entry((hive, key.to_ascii_lowercase()))
                .or_default()
                .insert(name.to_ascii_lowercase(), data);
        }

        pub fn add_key(&mut self, hive: Hive, key: &str) {
            self.keys
                .entry((hive, key.to_ascii_lowercase()))
                .or_default();
        }

        /// Every catalog key exists, so a missing value reads as absent, not as a missing key
        pub fn with_catalog_keys() -> Self {
            let mut reader = Self::default();
            for tweak in catalog() {
                for at in tweak
                    .writes
                    .iter()
                    .filter_map(super::super::Write::location)
                {
                    reader.add_key(at.hive, at.key);
                }
            }
            reader
        }
    }

    impl Reader for FakeReader {
        fn value(&self, at: &Location) -> Result<Option<Data>, KeyAbsent> {
            let Some(values) = self.keys.get(&(at.hive, at.key.to_ascii_lowercase())) else {
                // The fake creates the whole path: the first missing part is the key itself
                return Err(KeyAbsent {
                    created: at.key.to_owned(),
                });
            };
            Ok(values.get(&at.name.to_ascii_lowercase()).cloned())
        }

        fn scheme(&self) -> Option<String> {
            self.scheme.clone()
        }

        fn scheme_name(&self, guid: &str) -> Option<String> {
            match guid {
                BALANCED => Some("Balanced".to_owned()),
                HIGH_PERFORMANCE => Some("High performance".to_owned()),
                _ => None,
            }
        }
    }

    pub fn facts(cpu_vendor: CpuVendor, is_laptop: bool, met: &[Condition]) -> HardwareFacts {
        HardwareFacts {
            cpu_vendor,
            is_laptop,
            met: met.to_vec(),
            edition: "Professional".to_owned(),
            build: 26_100,
        }
    }

    pub fn olegpc() -> HardwareFacts {
        facts(
            CpuVendor::Amd,
            false,
            &[Condition::HagsSupported, Condition::Hibernation],
        )
    }

    fn base_ids() -> Vec<TweakId> {
        catalog()
            .iter()
            .filter(|tweak| tweak.focus == Focus::Base)
            .map(|tweak| tweak.id)
            .collect()
    }

    fn of_focus(focus: Focus) -> Vec<TweakId> {
        catalog()
            .iter()
            .filter(|tweak| tweak.focus == focus)
            .map(|tweak| tweak.id)
            .collect()
    }

    /// The Hardware cases table of the plan.
    #[test]
    fn the_four_machines_get_their_lists() {
        use TweakId::{FastStartupOff, Hags};

        // OLEGPC: every Games item, every Development item, nothing hidden
        let mut expected = base_ids();
        expected.extend(of_focus(Focus::Games));
        expected.extend(of_focus(Focus::Development));
        let got = offered(&olegpc(), &[Focus::Games, Focus::Development]);
        assert_eq!(got.shown, expected);
        assert_eq!(got.hidden, []);
        assert_eq!(power_target(&olegpc()), BALANCED);

        // An Intel laptop without HAGS: Games but HAGS, Balanced
        let laptop = facts(CpuVendor::Intel, true, &[Condition::Hibernation]);
        let got = offered(&laptop, &[Focus::Games]);
        let mut expected = base_ids();
        expected.extend(of_focus(Focus::Games).into_iter().filter(|id| *id != Hags));
        assert_eq!(got.shown, expected);
        assert_eq!(got.hidden, [(Hags, Condition::HagsSupported)]);
        assert_eq!(power_target(&laptop), BALANCED);

        // An Intel desktop without HAGS: Games but HAGS, High performance, clipboard history
        let desktop = facts(
            CpuVendor::Intel,
            false,
            &[Condition::Hibernation, Condition::HighPerformanceScheme],
        );
        let got = offered(&desktop, &[Focus::Games, Focus::Work]);
        let mut expected = base_ids();
        expected.extend(of_focus(Focus::Games).into_iter().filter(|id| *id != Hags));
        expected.push(TweakId::ClipboardHistory);
        assert_eq!(got.shown, expected);
        assert_eq!(got.hidden, [(Hags, Condition::HagsSupported)]);
        assert_eq!(power_target(&desktop), HIGH_PERFORMANCE);

        // An AMD desktop with HAGS, Home, hibernation off: fast startup hidden
        let home = facts(CpuVendor::Amd, false, &[Condition::HagsSupported]);
        let got = offered(&home, &[Focus::Development]);
        let mut expected: Vec<_> = base_ids()
            .into_iter()
            .filter(|id| *id != FastStartupOff)
            .collect();
        expected.extend([TweakId::EndTask, TweakId::LongPaths]);
        assert_eq!(got.shown, expected);
        assert_eq!(got.hidden, [(FastStartupOff, Condition::Hibernation)]);
    }

    #[test]
    fn high_performance_needs_its_scheme() {
        let without = facts(CpuVendor::Intel, false, &[]);
        let power = super::super::catalog::tweak(TweakId::PowerScheme).unwrap();
        assert_eq!(
            unmet(power, &without),
            Some(Condition::HighPerformanceScheme)
        );
        let amd = facts(CpuVendor::Amd, false, &[]);
        assert_eq!(unmet(power, &amd), None);
    }

    #[test]
    fn a_pair_is_merged_and_the_others_are_kept() {
        let name = "SwapEffectUpgradeEnable";
        assert_eq!(merge_pair("", name, "1"), "SwapEffectUpgradeEnable=1;");
        assert_eq!(
            merge_pair("VRROptimizeEnable=0;", name, "1"),
            "VRROptimizeEnable=0;SwapEffectUpgradeEnable=1;"
        );
        assert_eq!(
            merge_pair("SwapEffectUpgradeEnable=0;AutoHDREnable=1;", name, "1"),
            "SwapEffectUpgradeEnable=1;AutoHDREnable=1;"
        );
        assert_eq!(
            merge_pair("AutoHDREnable=1", name, "1"),
            "AutoHDREnable=1;SwapEffectUpgradeEnable=1;"
        );
    }

    #[test]
    fn rows_read_done_and_todo_with_the_default_checks() {
        let mut reader = FakeReader::with_catalog_keys();
        reader.set(
            Hive::User,
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            "HideFileExt",
            Data::Dword(0),
        );
        reader.scheme = Some(BALANCED.to_owned());
        let rows = rows_of(&reader, &olegpc());
        assert_eq!(rows.len(), catalog().len());
        let row = |id| rows.iter().find(|row| row.id == id).unwrap();
        assert_eq!(row(TweakId::FileExtensions).state, TweakState::Done);
        assert_eq!(row(TweakId::GameMode).state, TweakState::Todo);
        assert_eq!(row(TweakId::PowerScheme).state, TweakState::Done);
        assert!(row(TweakId::GameMode).is_default);
        assert!(!row(TweakId::ClassicContextMenu).is_default);
        assert_eq!(row(TweakId::Hags).unmet, None);
        assert_eq!(
            row(TweakId::Tips).values.len(),
            7,
            "every value of an item is listed"
        );
    }

    #[test]
    fn a_plan_keeps_what_each_value_replaces() {
        let mut reader = FakeReader::with_catalog_keys();
        let advanced = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
        reader.set(Hive::User, advanced, "HideFileExt", Data::Dword(1));
        reader.set(Hive::User, advanced, "LaunchTo", Data::Dword(1));
        reader
            .keys
            .retain(|(_, key), _| !key.contains("inprocserver32"));
        let plan = plan_of(
            &reader,
            &olegpc(),
            &[
                TweakId::FileExtensions,
                TweakId::ExplorerThisPc,
                TweakId::ClassicContextMenu,
                TweakId::GameMode,
                TweakId::LongPaths,
            ],
        );
        assert_eq!(plan.already_done, [TweakId::ExplorerThisPc]);
        let ids: Vec<_> = plan.items.iter().map(|item| item.id).collect();
        assert_eq!(
            ids,
            [
                TweakId::LongPaths,
                TweakId::FileExtensions,
                TweakId::ClassicContextMenu,
                TweakId::GameMode,
            ],
            "admin items first, then user items, in catalog order"
        );
        let old_of = |id| match &plan
            .items
            .iter()
            .find(|item| item.id == id)
            .unwrap()
            .changes[..]
        {
            [Change::Registry { old, .. }] => old.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            old_of(TweakId::FileExtensions),
            Old::Value {
                data: Data::Dword(1)
            }
        );
        assert_eq!(old_of(TweakId::GameMode), Old::Absent);
        assert!(matches!(
            old_of(TweakId::ClassicContextMenu),
            Old::KeyAbsent { .. }
        ));
        let shown = plan.to_ipc(&reader);
        assert_eq!(shown.admin.len(), 1);
        assert_eq!(shown.user.len(), 3);
        assert_eq!(shown.user[0].changes[0].was.as_deref(), Some("1"));
        assert_eq!(shown.user[0].changes[0].becomes.as_deref(), Some("0"));
        assert!(shown.needs_uac());
    }

    #[test]
    fn a_hidden_setting_is_never_planned() {
        let reader = FakeReader::with_catalog_keys();
        let no_hags = facts(CpuVendor::Amd, false, &[]);
        let plan = plan_of(&reader, &no_hags, &[TweakId::Hags]);
        assert!(plan.is_empty());
        assert_eq!(plan.already_done, []);
    }

    #[test]
    fn the_power_scheme_change_names_both_schemes() {
        let mut reader = FakeReader::with_catalog_keys();
        reader.scheme = Some(HIGH_PERFORMANCE.to_owned());
        let plan = plan_of(&reader, &olegpc(), &[TweakId::PowerScheme]);
        assert_eq!(
            plan.items[0].changes,
            [Change::Scheme {
                old: HIGH_PERFORMANCE.to_owned(),
                new: BALANCED.to_owned(),
            }]
        );
        let shown = shown(&plan.items[0].changes[0], &reader);
        assert_eq!(shown.was.as_deref(), Some("High performance"));
        assert_eq!(shown.becomes.as_deref(), Some("Balanced"));
    }
}
