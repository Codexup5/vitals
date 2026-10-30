//! The admin action of the optimizer: `vitals.exe --elevated optimize-apply`, one per run. It makes the restore point
//! first, then writes the HKLM values of the run. The request comes from the normal process, its revert values from
//! the user's journal, so every write is checked again against the catalog: a known id of an admin item, one of its
//! values, of the type the catalog declares; a key it deletes only one its write could have created. HKCU is never
//! written here (`rust.md` §3).

use serde::{Deserialize, Serialize};
use vitals_ipc::{RestorePoint, Rights};

use super::catalog::tweak;
use super::{Hive, OptimizeError, Put, RegWrite, Write};

pub const ACTION: &str = "optimize-apply";

/// A key a revert deletes has at least this many parts: never `SOFTWARE`, `SOFTWARE\Policies` or the like.
const MIN_DELETED_DEPTH: usize = 4;

/// What the elevated process is asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevatedRequest {
    pub writes: Vec<RegWrite>,
    /// An apply makes a point; a revert does not
    pub restore_point: bool,
    /// Turn System Protection on for C: when it is off
    pub enable_protection: bool,
}

/// What it did: the point, and the result of each write in the order of the request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevatedReport {
    pub restore_point: Option<RestorePoint>,
    pub results: Vec<Result<(), String>>,
}

/// What the elevated process does to the machine.
pub trait AdminMachine {
    fn write(&mut self, write: &RegWrite) -> Result<(), String>;
    fn restore_point(&mut self, enable_protection: bool) -> RestorePoint;
}

/// `key` is the value's key or one above it, deep enough to be one a write created.
pub fn is_on_path(key: &str, value_key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    let value_key = value_key.to_ascii_lowercase();
    let is_ancestor = value_key == key || value_key.starts_with(&format!("{key}\\"));
    is_ancestor && key.split('\\').count() >= MIN_DELETED_DEPTH
}

fn check_write(write: &RegWrite) -> Result<(), OptimizeError> {
    let refuse = |why: &str| Err(OptimizeError::Refused(format!("{:?}: {why}", write.id)));
    if write.hive != Hive::Machine {
        return refuse("the elevated side never writes HKCU");
    }
    let tweak = tweak(write.id).ok_or_else(|| OptimizeError::Unknown(format!("{:?}", write.id)))?;
    if tweak.rights() != Rights::Admin {
        return refuse("a user item is written by the normal process");
    }
    let declared = tweak.writes.iter().find_map(|declared| match declared {
        Write::Value(at, data)
            if at.hive == Hive::Machine
                && at.key.eq_ignore_ascii_case(&write.key)
                && at.name.eq_ignore_ascii_case(&write.name) =>
        {
            Some((at, data.data()))
        }
        _ => None,
    });
    let Some((at, data)) = declared else {
        return refuse("not a value of this setting");
    };
    match &write.put {
        Put::Data { data: given } if !given.same_type(&data) => {
            refuse("another type than the catalog's")
        }
        Put::DeleteKey { key } if !is_on_path(key, at.key) => refuse("a key off the value's path"),
        Put::Data { .. } | Put::DeleteValue | Put::DeleteKey { .. } => Ok(()),
    }
}

/// Checks every write of a request against the catalog; one bad write refuses the whole request.
pub fn check(request: &ElevatedRequest) -> Result<(), OptimizeError> {
    request.writes.iter().try_for_each(check_write)
}

/// The checked request on a machine: the point first, then the writes.
pub fn perform_with(
    machine: &mut dyn AdminMachine,
    request: &ElevatedRequest,
) -> Result<ElevatedReport, OptimizeError> {
    check(request)?;
    let restore_point = request
        .restore_point
        .then(|| machine.restore_point(request.enable_protection));
    let results = request
        .writes
        .iter()
        .map(|write| machine.write(write))
        .collect();
    Ok(ElevatedReport {
        restore_point,
        results,
    })
}

/// The elevated process: the request in, the report or the reason out.
pub fn perform(request: &str) -> Result<String, String> {
    let request: ElevatedRequest =
        serde_json::from_str(request).map_err(|error| error.to_string())?;
    let report = run(&request).map_err(|error| error.to_string());
    serde_json::to_string(&report).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn run(request: &ElevatedRequest) -> Result<ElevatedReport, OptimizeError> {
    perform_with(&mut super::windows::Admin, request)
}

#[cfg(not(windows))]
fn run(_: &ElevatedRequest) -> Result<ElevatedReport, OptimizeError> {
    Err(OptimizeError::Registry("only on Windows".to_owned()))
}

#[cfg(test)]
mod tests {
    use vitals_ipc::TweakId;

    use super::super::{Data, Hive, Put};
    use super::*;

    const GRAPHICS: &str = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers";
    const POLICIES_EXPLORER: &str = r"SOFTWARE\Policies\Microsoft\Windows\Explorer";

    fn write(id: TweakId, hive: Hive, key: &str, name: &str, put: Put) -> RegWrite {
        RegWrite {
            id,
            hive,
            key: key.to_owned(),
            name: name.to_owned(),
            put,
        }
    }

    fn hags(put: Put) -> RegWrite {
        write(TweakId::Hags, Hive::Machine, GRAPHICS, "HwSchMode", put)
    }

    fn request(writes: Vec<RegWrite>) -> ElevatedRequest {
        ElevatedRequest {
            writes,
            restore_point: true,
            enable_protection: false,
        }
    }

    #[derive(Default)]
    struct FakeAdmin {
        writes: Vec<RegWrite>,
        points: usize,
    }

    impl AdminMachine for FakeAdmin {
        fn write(&mut self, write: &RegWrite) -> Result<(), String> {
            self.writes.push(write.clone());
            if write.name == "LongPathsEnabled" {
                return Err("access denied".to_owned());
            }
            Ok(())
        }

        fn restore_point(&mut self, _: bool) -> RestorePoint {
            self.points += 1;
            RestorePoint::Created { time: 7 }
        }
    }

    #[test]
    fn a_catalog_write_of_an_admin_item_passes() {
        let ok = request(vec![
            hags(Put::Data {
                data: Data::Dword(2),
            }),
            hags(Put::Data {
                data: Data::Dword(0),
            }),
            hags(Put::DeleteValue),
            write(
                TweakId::StartWebSearch,
                Hive::Machine,
                POLICIES_EXPLORER,
                "DisableSearchBoxSuggestions",
                Put::DeleteKey {
                    key: POLICIES_EXPLORER.to_owned(),
                },
            ),
        ]);
        check(&ok).unwrap();
    }

    #[test]
    fn an_unknown_id_does_not_even_read() {
        let json = r#"{"writes":[{"id":"games.unknown","hive":"machine","key":"SYSTEM","name":"x",
            "put":{"kind":"deleteValue"}}],"restorePoint":false,"enableProtection":false}"#;
        assert!(serde_json::from_str::<ElevatedRequest>(json).is_err());
    }

    #[test]
    fn a_user_item_or_an_hkcu_write_is_refused() {
        let game_mode = write(
            TweakId::GameMode,
            Hive::User,
            r"Software\Microsoft\GameBar",
            "AutoGameModeEnabled",
            Put::Data {
                data: Data::Dword(1),
            },
        );
        assert!(check(&request(vec![game_mode])).is_err());
        let hkcu_hags = write(
            TweakId::Hags,
            Hive::User,
            GRAPHICS,
            "HwSchMode",
            Put::Data {
                data: Data::Dword(2),
            },
        );
        assert!(check(&request(vec![hkcu_hags])).is_err());
    }

    #[test]
    fn a_value_outside_the_item_or_of_another_type_is_refused() {
        let elsewhere = write(
            TweakId::Hags,
            Hive::Machine,
            r"SYSTEM\CurrentControlSet\Services\Tcpip",
            "HwSchMode",
            Put::Data {
                data: Data::Dword(2),
            },
        );
        assert!(check(&request(vec![elsewhere])).is_err());
        let other_name = write(
            TweakId::Hags,
            Hive::Machine,
            GRAPHICS,
            "TdrLevel",
            Put::Data {
                data: Data::Dword(0),
            },
        );
        assert!(check(&request(vec![other_name])).is_err());
        let text = hags(Put::Data {
            data: Data::Text("2".to_owned()),
        });
        assert!(check(&request(vec![text])).is_err());
    }

    #[test]
    fn only_a_key_on_the_path_of_the_value_may_be_deleted() {
        let wider = |key: &str| {
            write(
                TweakId::StartWebSearch,
                Hive::Machine,
                POLICIES_EXPLORER,
                "DisableSearchBoxSuggestions",
                Put::DeleteKey {
                    key: key.to_owned(),
                },
            )
        };
        assert!(
            check(&request(vec![wider(
                r"SOFTWARE\Policies\Microsoft\Windows\System"
            )]))
            .is_err()
        );
        assert!(
            check(&request(vec![wider(
                r"SOFTWARE\Policies\Microsoft\Windows\Explorer2"
            )]))
            .is_err()
        );
        // Never a root a catalog key merely sits under
        assert!(check(&request(vec![wider(r"SOFTWARE\Policies")])).is_err());
        assert!(check(&request(vec![wider("SOFTWARE")])).is_err());
    }

    #[test]
    fn one_bad_write_refuses_the_whole_request() {
        let mut machine = FakeAdmin::default();
        let mixed = request(vec![
            hags(Put::Data {
                data: Data::Dword(2),
            }),
            hags(Put::Data {
                data: Data::Text("2".to_owned()),
            }),
        ]);
        assert!(perform_with(&mut machine, &mixed).is_err());
        assert_eq!(machine.writes, [], "nothing written");
        assert_eq!(machine.points, 0);
    }

    #[test]
    fn the_point_comes_first_and_each_write_reports() {
        let mut machine = FakeAdmin::default();
        let both = request(vec![
            hags(Put::Data {
                data: Data::Dword(2),
            }),
            write(
                TweakId::LongPaths,
                Hive::Machine,
                r"SYSTEM\CurrentControlSet\Control\FileSystem",
                "LongPathsEnabled",
                Put::Data {
                    data: Data::Dword(1),
                },
            ),
        ]);
        let report = perform_with(&mut machine, &both).unwrap();
        assert_eq!(
            report.restore_point,
            Some(RestorePoint::Created { time: 7 })
        );
        assert_eq!(report.results, [Ok(()), Err("access denied".to_owned())]);
        assert_eq!(machine.points, 1);
        let revert = ElevatedRequest {
            restore_point: false,
            ..both
        };
        let report = perform_with(&mut machine, &revert).unwrap();
        assert_eq!(report.restore_point, None);
        assert_eq!(machine.points, 1, "a revert makes no point");
    }
}
