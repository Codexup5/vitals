//! The optimizer, stage 11a: a reversible set of Windows settings from a catalog compiled into the exe (the decisions
//! of 2026-10-29). Every change is journaled with the value it replaces before it is written, so a run can be reverted
//! exactly; the admin items of a run go through one UAC, which also makes a restore point first. The rules are in
//! `.claude/rules/rust.md` §3: nothing outside the catalog, HKCU never from `--elevated`.

// Until the commands of step 11 call it, the domain is reached only from its tests
#![allow(dead_code, reason = "wired to the commands in step 11 of the 11a plan")]

pub mod catalog;
pub mod plan;

use serde::{Deserialize, Serialize};
use vitals_ipc::{Condition, Effect, Focus, Rights, TweakId};

use crate::storage::StorageError;

#[derive(Debug, thiserror::Error)]
pub enum OptimizeError {
    #[error("{0:?} is not in the catalog")]
    Unknown(String),
    #[error("{0}")]
    Refused(String),
    #[error("the registry refused: {0}")]
    Registry(String),
    #[error("{0}")]
    Elevated(String),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// The registry hive of a value: the user's own or the machine's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Hive {
    /// `HKEY_CURRENT_USER`, written by the normal process
    User,
    /// `HKEY_LOCAL_MACHINE`, written by `--elevated` only
    Machine,
}

impl Hive {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::User => "HKCU",
            Self::Machine => "HKLM",
        }
    }

    pub const fn rights(self) -> Rights {
        match self {
            Self::User => Rights::User,
            Self::Machine => Rights::Admin,
        }
    }
}

/// The data of a registry value, of the two types the catalog writes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "value")]
pub enum Data {
    /// `REG_DWORD`
    Dword(u32),
    /// `REG_SZ`
    Text(String),
}

impl Data {
    /// Both are of the same registry type
    pub const fn same_type(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Dword(_), Self::Dword(_)) | (Self::Text(_), Self::Text(_))
        )
    }

    pub fn display(&self) -> String {
        match self {
            Self::Dword(value) => value.to_string(),
            Self::Text(text) => format!("\"{text}\""),
        }
    }
}

/// A registry value; an empty name is the default value of the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub hive: Hive,
    /// From the root of the hive: `Software\…` or `SYSTEM\…`
    pub key: &'static str,
    pub name: &'static str,
}

impl Location {
    pub fn display(&self) -> String {
        let name = if self.name.is_empty() { "@" } else { self.name };
        format!("{}\\{}\\{name}", self.hive.prefix(), self.key)
    }
}

/// What a catalog item writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    /// A value set to fixed data
    Value(Location, DataOf),
    /// One `name=value;` pair merged into a `REG_SZ` of such pairs, the other pairs kept
    Pair {
        at: Location,
        pair: &'static str,
        value: &'static str,
    },
    /// The active power scheme, chosen by the hardware
    PowerScheme,
}

/// The fixed data of a catalog value, `'static` so the catalog is a constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataOf {
    Dword(u32),
    Text(&'static str),
}

impl DataOf {
    pub fn data(self) -> Data {
        match self {
            Self::Dword(value) => Data::Dword(value),
            Self::Text(text) => Data::Text(text.to_owned()),
        }
    }
}

impl Write {
    /// The registry value it writes; none for the power scheme
    pub const fn location(&self) -> Option<Location> {
        match self {
            Self::Value(at, _) | Self::Pair { at, .. } => Some(*at),
            Self::PowerScheme => None,
        }
    }
}

/// A setting of the catalog.
#[derive(Debug, Clone, Copy)]
pub struct Tweak {
    pub id: TweakId,
    pub focus: Focus,
    pub effect: Effect,
    /// Checked when the section opens
    pub is_default: bool,
    pub condition: Option<Condition>,
    pub writes: &'static [Write],
}

impl Tweak {
    /// An item that writes HKLM is an admin item; the catalog never mixes hives in one item
    pub fn rights(&self) -> Rights {
        let is_machine = self
            .writes
            .iter()
            .filter_map(Write::location)
            .any(|at| at.hive == Hive::Machine);
        if is_machine {
            Rights::Admin
        } else {
            Rights::User
        }
    }
}

/// What a registry value was before a change, as the journal keeps it to put it back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Old {
    Value {
        data: Data,
    },
    /// The key was there, the value was not
    Absent,
    /// The key was not there: `created` is the first key of its path the write created, deleted again by a revert
    KeyAbsent {
        created: String,
    },
}

/// What a write does to one registry value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Put {
    Data {
        data: Data,
    },
    DeleteValue,
    /// Deletes this key with what is under it: only a key a change created
    DeleteKey {
        key: String,
    },
}

impl Put {
    /// The write that brings the old state back
    pub fn restoring(old: &Old) -> Self {
        match old {
            Old::Value { data } => Self::Data { data: data.clone() },
            Old::Absent => Self::DeleteValue,
            Old::KeyAbsent { created } => Self::DeleteKey {
                key: created.clone(),
            },
        }
    }
}

/// One registry write, for the normal process or the elevated one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegWrite {
    pub id: TweakId,
    pub hive: Hive,
    pub key: String,
    pub name: String,
    pub put: Put,
}

/// One change a run makes, with what it replaces: the unit of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Change {
    Registry {
        hive: Hive,
        key: String,
        name: String,
        old: Old,
        new: Data,
    },
    Scheme {
        old: String,
        new: String,
    },
}

/// What the optimizer reads of the machine: values and the power scheme.
pub trait Reader {
    /// `Err` when the key is absent; `Ok(None)` when the key is there and the value is not
    fn value(&self, at: &Location) -> Result<Option<Data>, KeyAbsent>;
    /// The GUID of the active scheme, lower case
    fn scheme(&self) -> Option<String>;
    /// The scheme's name as Windows shows it
    fn scheme_name(&self, guid: &str) -> Option<String>;
}

/// The key of a value does not exist; `created` is the first key of the path a write would create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyAbsent {
    pub created: String,
}
