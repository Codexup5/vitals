//! The catalog of 11a: every setting the optimizer may change, with the values it writes. Nothing outside it is ever
//! written, and the elevated side checks each write against it again.

use vitals_ipc::{Condition, Effect, Focus, TweakId};

use super::{DataOf, Hive, Location, Tweak, Write};

const ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const CONTENT_DELIVERY: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";

const fn user(key: &'static str, name: &'static str, data: DataOf) -> Write {
    Write::Value(
        Location {
            hive: Hive::User,
            key,
            name,
        },
        data,
    )
}

const fn machine(key: &'static str, name: &'static str, data: DataOf) -> Write {
    Write::Value(
        Location {
            hive: Hive::Machine,
            key,
            name,
        },
        data,
    )
}

const fn off(key: &'static str, name: &'static str) -> Write {
    user(key, name, DataOf::Dword(0))
}

const fn item(id: TweakId, focus: Focus, effect: Effect, writes: &'static [Write]) -> Tweak {
    Tweak {
        id,
        focus,
        effect,
        is_default: true,
        condition: None,
        writes,
    }
}

const fn unchecked(tweak: Tweak) -> Tweak {
    Tweak {
        is_default: false,
        ..tweak
    }
}

const fn only_if(condition: Condition, tweak: Tweak) -> Tweak {
    Tweak {
        condition: Some(condition),
        ..tweak
    }
}

const CATALOG: [Tweak; 19] = [
    item(
        TweakId::AdvertisingId,
        Focus::Base,
        Effect::Now,
        &[off(
            r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
            "Enabled",
        )],
    ),
    item(
        TweakId::Tips,
        Focus::Base,
        Effect::Now,
        &[
            off(CONTENT_DELIVERY, "SubscribedContent-338389Enabled"),
            off(CONTENT_DELIVERY, "SubscribedContent-310093Enabled"),
            off(CONTENT_DELIVERY, "SubscribedContent-338393Enabled"),
            off(CONTENT_DELIVERY, "SubscribedContent-353694Enabled"),
            off(CONTENT_DELIVERY, "SubscribedContent-353696Enabled"),
            off(CONTENT_DELIVERY, "SystemPaneSuggestionsEnabled"),
            off(CONTENT_DELIVERY, "SoftLandingEnabled"),
        ],
    ),
    item(
        TweakId::StartRecommendations,
        Focus::Base,
        Effect::ExplorerRestart,
        &[
            off(ADVANCED, "Start_IrisRecommendations"),
            off(ADVANCED, "Start_TrackDocs"),
        ],
    ),
    item(
        TweakId::FileExtensions,
        Focus::Base,
        Effect::ExplorerRestart,
        &[off(ADVANCED, "HideFileExt")],
    ),
    item(
        TweakId::ExplorerThisPc,
        Focus::Base,
        Effect::Now,
        &[user(ADVANCED, "LaunchTo", DataOf::Dword(1))],
    ),
    item(
        TweakId::StartWebSearch,
        Focus::Base,
        Effect::ExplorerRestart,
        &[machine(
            r"SOFTWARE\Policies\Microsoft\Windows\Explorer",
            "DisableSearchBoxSuggestions",
            DataOf::Dword(1),
        )],
    ),
    item(
        TweakId::TailoredExperiences,
        Focus::Base,
        Effect::Now,
        &[off(
            r"Software\Microsoft\Windows\CurrentVersion\Privacy",
            "TailoredExperiencesWithDiagnosticDataEnabled",
        )],
    ),
    item(
        TweakId::DiagnosticData,
        Focus::Base,
        Effect::Now,
        &[machine(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection",
            "AllowTelemetry",
            DataOf::Dword(1),
        )],
    ),
    item(
        TweakId::ActivityHistory,
        Focus::Base,
        Effect::Now,
        &[machine(
            r"SOFTWARE\Policies\Microsoft\Windows\System",
            "PublishUserActivities",
            DataOf::Dword(0),
        )],
    ),
    // The key is created; a revert deletes what the write created
    unchecked(item(
        TweakId::ClassicContextMenu,
        Focus::Base,
        Effect::ExplorerRestart,
        &[user(
            r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32",
            "",
            DataOf::Text(""),
        )],
    )),
    only_if(
        Condition::Hibernation,
        unchecked(item(
            TweakId::FastStartupOff,
            Focus::Base,
            Effect::NextShutdown,
            &[machine(
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Power",
                "HiberbootEnabled",
                DataOf::Dword(0),
            )],
        )),
    ),
    item(
        TweakId::GameMode,
        Focus::Games,
        Effect::Now,
        &[user(
            r"Software\Microsoft\GameBar",
            "AutoGameModeEnabled",
            DataOf::Dword(1),
        )],
    ),
    only_if(
        Condition::HagsSupported,
        item(
            TweakId::Hags,
            Focus::Games,
            Effect::Reboot,
            &[machine(
                r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers",
                "HwSchMode",
                DataOf::Dword(2),
            )],
        ),
    ),
    item(
        TweakId::WindowedOptimizations,
        Focus::Games,
        Effect::NextGameStart,
        &[Write::Pair {
            at: Location {
                hive: Hive::User,
                key: r"Software\Microsoft\DirectX\UserGpuPreferences",
                name: "DirectXUserGlobalSettings",
            },
            pair: "SwapEffectUpgradeEnable",
            value: "1",
        }],
    ),
    // `SPI_SETMOUSE` after the write makes it hold without a sign-in
    item(
        TweakId::PointerPrecisionOff,
        Focus::Games,
        Effect::Now,
        &[
            user(r"Control Panel\Mouse", "MouseSpeed", DataOf::Text("0")),
            user(r"Control Panel\Mouse", "MouseThreshold1", DataOf::Text("0")),
            user(r"Control Panel\Mouse", "MouseThreshold2", DataOf::Text("0")),
        ],
    ),
    unchecked(item(
        TweakId::PowerScheme,
        Focus::Games,
        Effect::Now,
        &[Write::PowerScheme],
    )),
    item(
        TweakId::EndTask,
        Focus::Development,
        Effect::Now,
        &[user(
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\TaskbarDeveloperSettings",
            "TaskbarEndTask",
            DataOf::Dword(1),
        )],
    ),
    item(
        TweakId::LongPaths,
        Focus::Development,
        Effect::NewPrograms,
        &[machine(
            r"SYSTEM\CurrentControlSet\Control\FileSystem",
            "LongPathsEnabled",
            DataOf::Dword(1),
        )],
    ),
    item(
        TweakId::ClipboardHistory,
        Focus::Work,
        Effect::Now,
        &[user(
            r"Software\Microsoft\Clipboard",
            "EnableClipboardHistory",
            DataOf::Dword(1),
        )],
    ),
];

/// Every setting, in the order the section lists them.
pub const fn catalog() -> &'static [Tweak] {
    &CATALOG
}

/// The setting of an id; `None` for an id the catalog no longer has.
pub fn tweak(id: TweakId) -> Option<&'static Tweak> {
    catalog().iter().find(|tweak| tweak.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use vitals_ipc::{Condition, Effect, Focus, Rights};

    use super::super::{DataOf, Hive, Location, Write};
    use super::*;

    /// The keys brief §7 and `rust.md` §3 put out of reach: Defender, HVCI, the pagefile, services, Windows Update.
    const NEVER: [&str; 5] = [
        r"SOFTWARE\Microsoft\Windows Defender",
        r"SOFTWARE\Policies\Microsoft\Windows Defender",
        r"SYSTEM\CurrentControlSet\Control\DeviceGuard",
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management",
        r"SYSTEM\CurrentControlSet\Services",
    ];

    fn is_under(key: &str, root: &str) -> bool {
        let key = key.to_ascii_lowercase();
        let root = root.to_ascii_lowercase();
        key == root || key.starts_with(&format!("{root}\\"))
    }

    #[test]
    fn every_id_is_in_the_catalog_once() {
        let ids: Vec<_> = catalog().iter().map(|tweak| tweak.id).collect();
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "an id appears twice");
        assert_eq!(ids, TweakId::ALL, "the catalog and TweakId::ALL differ");
    }

    #[test]
    fn rights_follow_the_hive_and_an_item_never_mixes_hives() {
        for tweak in catalog() {
            assert!(!tweak.writes.is_empty(), "{:?} writes nothing", tweak.id);
            let hives: HashSet<_> = tweak
                .writes
                .iter()
                .filter_map(Write::location)
                .map(|at| at.hive)
                .collect();
            assert!(hives.len() <= 1, "{:?} mixes hives", tweak.id);
            let expected = if hives.contains(&Hive::Machine) {
                Rights::Admin
            } else {
                Rights::User
            };
            assert_eq!(tweak.rights(), expected, "{:?}", tweak.id);
        }
    }

    #[test]
    fn only_registry_values_go_to_the_machine() {
        // A pair merge and the power scheme have no exact old value to hand the elevated side
        for tweak in catalog() {
            for write in tweak.writes {
                if let Write::Pair { at, .. } = write {
                    assert_eq!(at.hive, Hive::User, "{:?}", tweak.id);
                }
            }
        }
    }

    #[test]
    fn nothing_is_written_under_the_never_list() {
        for tweak in catalog() {
            for at in tweak.writes.iter().filter_map(Write::location) {
                assert!(
                    !at.key.to_ascii_lowercase().contains("windowsupdate"),
                    "{:?} writes {}",
                    tweak.id,
                    at.key
                );
                for root in NEVER {
                    assert!(!is_under(at.key, root), "{:?} writes {}", tweak.id, at.key);
                }
            }
        }
    }

    #[test]
    fn the_never_check_matches_by_key_not_by_prefix_text() {
        assert!(is_under(
            r"system\currentcontrolset\services\x",
            r"SYSTEM\CurrentControlSet\Services"
        ));
        assert!(!is_under(
            r"SYSTEM\CurrentControlSet\ServicesExtra",
            r"SYSTEM\CurrentControlSet\Services"
        ));
    }

    #[test]
    fn the_slugs_are_pinned() {
        // The journal keeps these: renaming one needs a serde alias, or old runs cannot be reverted
        let slugs = [
            "base.advertising-id",
            "base.tips",
            "base.start-recommendations",
            "base.file-extensions",
            "base.explorer-this-pc",
            "base.start-web-search",
            "base.tailored-experiences",
            "base.diagnostic-data",
            "base.activity-history",
            "base.classic-context-menu",
            "base.fast-startup-off",
            "games.game-mode",
            "games.hags",
            "games.windowed-optimizations",
            "games.pointer-precision-off",
            "games.power-scheme",
            "dev.end-task",
            "dev.long-paths",
            "work.clipboard-history",
        ];
        for (slug, id) in slugs.iter().zip(TweakId::ALL) {
            let read: TweakId = serde_json::from_str(&format!("\"{slug}\"")).unwrap();
            assert_eq!(read, id);
        }
    }

    /// The Catalog table of the plan: focus, rights, effect, checked by default, condition.
    #[test]
    fn the_catalog_matches_the_plan_table() {
        use Effect::{ExplorerRestart, NewPrograms, NextGameStart, NextShutdown, Now, Reboot};
        use Focus::{Base, Development, Games, Work};
        use Rights::{Admin, User};
        let table = [
            (TweakId::AdvertisingId, Base, User, Now, true, None),
            (TweakId::Tips, Base, User, Now, true, None),
            (
                TweakId::StartRecommendations,
                Base,
                User,
                ExplorerRestart,
                true,
                None,
            ),
            (
                TweakId::FileExtensions,
                Base,
                User,
                ExplorerRestart,
                true,
                None,
            ),
            (TweakId::ExplorerThisPc, Base, User, Now, true, None),
            (
                TweakId::StartWebSearch,
                Base,
                Admin,
                ExplorerRestart,
                true,
                None,
            ),
            (TweakId::TailoredExperiences, Base, User, Now, true, None),
            (TweakId::DiagnosticData, Base, Admin, Now, true, None),
            (TweakId::ActivityHistory, Base, Admin, Now, true, None),
            (
                TweakId::ClassicContextMenu,
                Base,
                User,
                ExplorerRestart,
                false,
                None,
            ),
            (
                TweakId::FastStartupOff,
                Base,
                Admin,
                NextShutdown,
                false,
                Some(Condition::Hibernation),
            ),
            (TweakId::GameMode, Games, User, Now, true, None),
            (
                TweakId::Hags,
                Games,
                Admin,
                Reboot,
                true,
                Some(Condition::HagsSupported),
            ),
            (
                TweakId::WindowedOptimizations,
                Games,
                User,
                NextGameStart,
                true,
                None,
            ),
            (TweakId::PointerPrecisionOff, Games, User, Now, true, None),
            (TweakId::PowerScheme, Games, User, Now, false, None),
            (TweakId::EndTask, Development, User, Now, true, None),
            (
                TweakId::LongPaths,
                Development,
                Admin,
                NewPrograms,
                true,
                None,
            ),
            (TweakId::ClipboardHistory, Work, User, Now, true, None),
        ];
        assert_eq!(catalog().len(), table.len());
        for (tweak, (id, focus, rights, effect, is_default, condition)) in
            catalog().iter().zip(table)
        {
            assert_eq!(tweak.id, id);
            assert_eq!(tweak.focus, focus, "{id:?}");
            assert_eq!(tweak.rights(), rights, "{id:?}");
            assert_eq!(tweak.effect, effect, "{id:?}");
            assert_eq!(tweak.is_default, is_default, "{id:?}");
            assert_eq!(tweak.condition, condition, "{id:?}");
        }
    }

    #[test]
    fn the_admin_values_are_the_documented_ones() {
        let value = |id| match tweak(id).unwrap().writes {
            [Write::Value(at, data)] => (*at, *data),
            other => panic!("{id:?} writes {other:?}"),
        };
        let machine = |key, name| Location {
            hive: Hive::Machine,
            key,
            name,
        };
        assert_eq!(
            value(TweakId::Hags),
            (
                machine(
                    r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers",
                    "HwSchMode"
                ),
                DataOf::Dword(2)
            )
        );
        assert_eq!(
            value(TweakId::LongPaths),
            (
                machine(
                    r"SYSTEM\CurrentControlSet\Control\FileSystem",
                    "LongPathsEnabled"
                ),
                DataOf::Dword(1)
            )
        );
        assert_eq!(
            value(TweakId::FastStartupOff),
            (
                machine(
                    r"SYSTEM\CurrentControlSet\Control\Session Manager\Power",
                    "HiberbootEnabled"
                ),
                DataOf::Dword(0)
            )
        );
        assert_eq!(
            value(TweakId::DiagnosticData),
            (
                machine(
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection",
                    "AllowTelemetry"
                ),
                DataOf::Dword(1)
            )
        );
        assert_eq!(
            value(TweakId::StartWebSearch),
            (
                machine(
                    r"SOFTWARE\Policies\Microsoft\Windows\Explorer",
                    "DisableSearchBoxSuggestions"
                ),
                DataOf::Dword(1)
            )
        );
        assert_eq!(
            value(TweakId::ActivityHistory),
            (
                machine(
                    r"SOFTWARE\Policies\Microsoft\Windows\System",
                    "PublishUserActivities"
                ),
                DataOf::Dword(0)
            )
        );
    }
}
