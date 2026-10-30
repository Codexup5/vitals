//! The Optimization section: the focus chips, the settings of the chosen focuses with their state, the preview
//! "was → becomes" of a run, its outcome and the journal of runs with their revert.

use vitals_ipc::{
    ApplyOutcome, ChangeOutcome, Condition, CpuVendor, Effect, Focus, ItemResult, JournalRun,
    OptimizeOverview, OptimizePlan, PlanItem, RestorePoint, Rights, TweakId, TweakRow, TweakState,
    ValueChange,
};

use crate::format;
use crate::i18n::Locale;
use crate::views::disks::Tone;
use crate::views::runners::Badge;

/// The focuses the user chooses; the base is always on.
pub const CHOOSABLE: [Focus; 3] = [Focus::Games, Focus::Development, Focus::Work];

pub fn focus_word(focus: Focus, locale: Locale) -> &'static str {
    let texts = &locale.texts().optimize;
    match focus {
        Focus::Base => texts.focus_base,
        Focus::Games => texts.focus_games,
        Focus::Development => texts.focus_development,
        Focus::Work => texts.focus_work,
    }
}

pub fn effect_word(effect: Effect, locale: Locale) -> &'static str {
    let texts = &locale.texts().optimize;
    match effect {
        Effect::Now => texts.effect_now,
        Effect::ExplorerRestart => texts.effect_explorer,
        Effect::NextGameStart => texts.effect_game,
        Effect::NewPrograms => texts.effect_programs,
        Effect::NextShutdown => texts.effect_shutdown,
        Effect::Reboot => texts.effect_reboot,
    }
}

fn reason_of(condition: Condition, locale: Locale) -> &'static str {
    let texts = &locale.texts().optimize;
    match condition {
        Condition::HagsSupported => texts.needs_hags,
        Condition::Hibernation => texts.needs_hibernation,
        Condition::HighPerformanceScheme => texts.needs_high_performance,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusChip {
    pub focus: Focus,
    pub label: &'static str,
    pub is_on: bool,
}

pub fn focus_chips(selected: &[Focus], locale: Locale) -> Vec<FocusChip> {
    CHOOSABLE
        .into_iter()
        .map(|focus| FocusChip {
            focus,
            label: focus_word(focus, locale),
            is_on: selected.contains(&focus),
        })
        .collect()
}

/// A row of the base or of a chosen focus, offered on this machine.
fn is_offered(row: &TweakRow, focuses: &[Focus]) -> bool {
    (row.focus == Focus::Base || focuses.contains(&row.focus)) && row.unmet.is_none()
}

/// The checks the section opens with: the settings checked by default that are not done yet.
pub fn default_checks(overview: &OptimizeOverview, focuses: &[Focus]) -> Vec<TweakId> {
    overview
        .rows
        .iter()
        .filter(|row| is_offered(row, focuses) && row.is_default && row.state == TweakState::Todo)
        .map(|row| row.id)
        .collect()
}

/// "Maximum": every offered setting that is not done yet.
pub fn maximum_checks(overview: &OptimizeOverview, focuses: &[Focus]) -> Vec<TweakId> {
    overview
        .rows
        .iter()
        .filter(|row| is_offered(row, focuses) && row.state == TweakState::Todo)
        .map(|row| row.id)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueLine {
    pub name: String,
    pub was: String,
    pub becomes: String,
}

fn value_line(change: &ValueChange, locale: Locale) -> ValueLine {
    let absent = locale.texts().optimize.absent;
    ValueLine {
        name: change.name.clone(),
        was: change.was.clone().unwrap_or_else(|| absent.to_owned()),
        becomes: change.becomes.clone().unwrap_or_else(|| absent.to_owned()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TweakLine {
    pub id: TweakId,
    pub title: &'static str,
    pub hint: &'static str,
    pub state: Badge,
    /// The admin mark of an HKLM setting
    pub admin: Option<&'static str>,
    pub effect: &'static str,
    pub is_checked: bool,
    pub is_done: bool,
    /// Nothing to apply: done, or a value that cannot be read
    pub is_locked: bool,
    pub values: Vec<ValueLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusGroup {
    pub title: &'static str,
    pub lines: Vec<TweakLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenLine {
    pub title: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizeReady {
    /// The machine as the optimizer sees it: vendor, kind, edition, build
    pub machine: String,
    pub summary: String,
    pub groups: Vec<FocusGroup>,
    /// Settings of the chosen focuses this machine cannot have, with the reason
    pub hidden: Vec<HiddenLine>,
    /// Checked settings that are not done yet: what "Preview" plans
    pub to_apply: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizeView {
    Loading,
    Error(&'static str),
    Ready(OptimizeReady),
}

fn line_of(row: &TweakRow, checked: &[TweakId], locale: Locale) -> TweakLine {
    let texts = &locale.texts().optimize;
    let tweak = texts.tweaks.of(row.id);
    let is_done = row.state == TweakState::Done;
    TweakLine {
        id: row.id,
        title: tweak.title,
        hint: tweak.hint,
        state: match row.state {
            TweakState::Done => (texts.done, Tone::Ok),
            TweakState::Todo => (texts.todo, Tone::Accent),
            TweakState::Unreadable => (texts.unreadable, Tone::Warn),
        },
        admin: (row.rights == Rights::Admin).then_some(texts.admin),
        effect: effect_word(row.effect, locale),
        is_checked: checked.contains(&row.id),
        is_done,
        is_locked: row.state != TweakState::Todo,
        values: row
            .values
            .iter()
            .map(|change| value_line(change, locale))
            .collect(),
    }
}

pub fn optimize_view(
    answer: Option<Result<&OptimizeOverview, &'static str>>,
    focuses: &[Focus],
    checked: &[TweakId],
    locale: Locale,
) -> OptimizeView {
    let overview = match answer {
        None => return OptimizeView::Loading,
        Some(Err(message)) => return OptimizeView::Error(message),
        Some(Ok(overview)) => overview,
    };
    let texts = &locale.texts().optimize;
    let groups: Vec<FocusGroup> = std::iter::once(Focus::Base)
        .chain(
            CHOOSABLE
                .into_iter()
                .filter(|focus| focuses.contains(focus)),
        )
        .map(|focus| FocusGroup {
            title: focus_word(focus, locale),
            lines: overview
                .rows
                .iter()
                .filter(|row| row.focus == focus && row.unmet.is_none())
                .map(|row| line_of(row, checked, locale))
                .collect(),
        })
        .filter(|group| !group.lines.is_empty())
        .collect();
    let hidden = overview
        .rows
        .iter()
        .filter(|row| row.focus == Focus::Base || focuses.contains(&row.focus))
        .filter_map(|row| {
            row.unmet.map(|condition| HiddenLine {
                title: texts.tweaks.of(row.id).title,
                reason: reason_of(condition, locale),
            })
        })
        .collect();
    let offered: Vec<&TweakLine> = groups.iter().flat_map(|group| &group.lines).collect();
    let done = offered.iter().filter(|line| line.is_done).count();
    let to_apply = offered
        .iter()
        .filter(|line| line.is_checked && !line.is_locked)
        .count();
    let facts = &overview.facts;
    let vendor = match facts.cpu_vendor {
        CpuVendor::Amd => "AMD",
        CpuVendor::Intel => "Intel",
        CpuVendor::Other => texts.other_cpu,
    };
    let kind = if facts.is_laptop {
        texts.laptop
    } else {
        texts.desktop
    };
    OptimizeView::Ready(OptimizeReady {
        machine: (texts.machine)(vendor, kind, &facts.edition, facts.build),
        summary: (texts.summary)(offered.len() - done, done),
        groups,
        hidden,
        to_apply,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLine {
    pub title: &'static str,
    pub effect: &'static str,
    pub values: Vec<ValueLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanGroup {
    pub title: &'static str,
    pub lines: Vec<PlanLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewView {
    pub groups: Vec<PlanGroup>,
    /// Checked settings that already hold
    pub already: Option<String>,
    /// One UAC with a restore point, or neither
    pub rights_line: &'static str,
    pub needs_uac: bool,
    pub apply_label: String,
    pub is_empty: bool,
}

fn plan_lines(items: &[PlanItem], locale: Locale) -> Vec<PlanLine> {
    items
        .iter()
        .map(|item| PlanLine {
            title: locale.texts().optimize.tweaks.of(item.id).title,
            effect: effect_word(item.effect, locale),
            values: item
                .changes
                .iter()
                .map(|change| value_line(change, locale))
                .collect(),
        })
        .collect()
}

pub fn preview_view(plan: &OptimizePlan, locale: Locale) -> PreviewView {
    let texts = &locale.texts().optimize;
    let groups: Vec<PlanGroup> = [
        (texts.admin_group, &plan.admin),
        (texts.user_group, &plan.user),
    ]
    .into_iter()
    .filter(|(_, items)| !items.is_empty())
    .map(|(title, items)| PlanGroup {
        title,
        lines: plan_lines(items, locale),
    })
    .collect();
    let count = plan.admin.len() + plan.user.len();
    let needs_uac = plan.needs_uac();
    PreviewView {
        already: (!plan.already_done.is_empty()).then(|| {
            let titles: Vec<&str> = plan
                .already_done
                .iter()
                .map(|id| texts.tweaks.of(*id).title)
                .collect();
            (texts.already)(&titles.join(", "))
        }),
        rights_line: if needs_uac {
            texts.uac_line
        } else {
            texts.no_uac_line
        },
        needs_uac,
        apply_label: (texts.apply)(count),
        is_empty: groups.is_empty(),
        groups,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeView {
    pub headline: String,
    pub tone: Tone,
    pub failures: Vec<(&'static str, String)>,
    pub restore: Option<String>,
    /// What the user does for every change to show
    pub then: Vec<&'static str>,
    /// "Restart Explorer" is offered
    pub offers_explorer: bool,
}

fn when(seconds: i64, locale: Locale) -> String {
    format::local_time(seconds).map_or_else(String::new, |time| format::date_time(time, locale))
}

/// The outcome of an apply, or of a revert when `is_revert`.
pub fn outcome_view(outcome: &ApplyOutcome, is_revert: bool, locale: Locale) -> OutcomeView {
    let texts = &locale.texts().optimize;
    if outcome.outcome == ChangeOutcome::Declined {
        return OutcomeView {
            headline: texts.declined.to_owned(),
            tone: Tone::Warn,
            failures: Vec::new(),
            restore: None,
            then: Vec::new(),
            offers_explorer: false,
        };
    }
    let failures: Vec<(&'static str, String)> = outcome
        .items
        .iter()
        .filter_map(|item| match &item.result {
            ItemResult::Done => None,
            ItemResult::Failed { message } => {
                Some((texts.tweaks.of(item.id).title, message.clone()))
            }
        })
        .collect();
    let done = outcome.items.len() - failures.len();
    let restore = outcome.restore_point.as_ref().map(|point| match point {
        RestorePoint::Created { time } => (texts.restore_created)(&when(*time, locale)),
        RestorePoint::Kept { time } => (texts.restore_kept)(&when(*time, locale)),
        RestorePoint::ProtectionOff => texts.restore_off.to_owned(),
        RestorePoint::Failed { message } => (texts.restore_failed)(message),
    });
    let then = outcome
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Now => None,
            Effect::ExplorerRestart => Some(texts.then_explorer),
            Effect::NextGameStart => Some(texts.then_game),
            Effect::NewPrograms => Some(texts.then_programs),
            Effect::NextShutdown => Some(texts.then_shutdown),
            Effect::Reboot => Some(texts.then_reboot),
        })
        .collect();
    OutcomeView {
        headline: if outcome.items.is_empty() {
            texts.nothing_changed.to_owned()
        } else {
            let count = if is_revert {
                texts.reverted_count
            } else {
                texts.applied
            };
            count(done, failures.len())
        },
        tone: if failures.is_empty() {
            Tone::Ok
        } else {
            Tone::Warn
        },
        offers_explorer: outcome.effects.contains(&Effect::ExplorerRestart),
        failures,
        restore,
        then,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalLine {
    pub run: i64,
    pub label: String,
    pub is_reverted: bool,
    pub changes: Vec<(&'static str, ValueLine)>,
}

pub fn journal_view(runs: &[JournalRun], locale: Locale) -> Vec<JournalLine> {
    let texts = &locale.texts().optimize;
    runs.iter()
        .map(|run| JournalLine {
            run: run.run,
            label: (texts.journal_run)(&when(run.time, locale), run.changes.len()),
            is_reverted: run.is_reverted(),
            changes: run
                .changes
                .iter()
                .map(|change| {
                    (
                        texts.tweaks.of(change.id).title,
                        value_line(&change.change, locale),
                    )
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use vitals_ipc::{HardwareFacts, ItemOutcome, JournalChange};

    use super::*;

    fn row(id: TweakId, focus: Focus, state: TweakState) -> TweakRow {
        TweakRow {
            id,
            focus,
            rights: Rights::User,
            effect: Effect::Now,
            state,
            is_default: true,
            unmet: None,
            values: vec![ValueChange {
                name: "HKCU\\X\\Y".to_owned(),
                was: None,
                becomes: Some("1".to_owned()),
            }],
        }
    }

    fn overview() -> OptimizeOverview {
        let mut hags = row(TweakId::Hags, Focus::Games, TweakState::Todo);
        hags.unmet = Some(Condition::HagsSupported);
        hags.rights = Rights::Admin;
        let mut menu = row(TweakId::ClassicContextMenu, Focus::Base, TweakState::Todo);
        menu.is_default = false;
        OptimizeOverview {
            facts: HardwareFacts {
                cpu_vendor: CpuVendor::Amd,
                is_laptop: false,
                met: Vec::new(),
                edition: "Professional".to_owned(),
                build: 26_100,
            },
            rows: vec![
                row(TweakId::FileExtensions, Focus::Base, TweakState::Done),
                row(TweakId::GameMode, Focus::Games, TweakState::Todo),
                hags,
                menu,
                row(TweakId::EndTask, Focus::Development, TweakState::Todo),
            ],
        }
    }

    #[test]
    fn the_defaults_skip_done_hidden_and_unchosen_settings() {
        let overview = overview();
        assert_eq!(
            default_checks(&overview, &[Focus::Games]),
            [TweakId::GameMode]
        );
        assert_eq!(
            maximum_checks(&overview, &[Focus::Games]),
            [TweakId::GameMode, TweakId::ClassicContextMenu]
        );
    }

    #[test]
    fn the_view_groups_by_focus_and_names_the_hidden_with_the_reason() {
        let OptimizeView::Ready(ready) = optimize_view(
            Some(Ok(&overview())),
            &[Focus::Games],
            &[TweakId::GameMode],
            Locale::En,
        ) else {
            panic!("not ready");
        };
        let titles: Vec<_> = ready.groups.iter().map(|group| group.title).collect();
        assert_eq!(titles, ["Base", "Games"]);
        assert_eq!(ready.hidden.len(), 1);
        assert_eq!(
            ready.hidden[0].reason,
            Locale::En.texts().optimize.needs_hags
        );
        assert_eq!(ready.to_apply, 1);
        assert!(ready.groups[0].lines[0].is_done);
        assert_eq!(ready.groups[0].lines[0].values[0].was, "absent");
        assert!(ready.machine.contains("AMD"));
    }

    #[test]
    fn loading_and_errors_are_their_own_states() {
        assert_eq!(
            optimize_view(None, &[], &[], Locale::Ru),
            OptimizeView::Loading
        );
        assert_eq!(
            optimize_view(Some(Err("x")), &[], &[], Locale::Ru),
            OptimizeView::Error("x")
        );
    }

    #[test]
    fn the_preview_says_whether_it_asks_for_rights() {
        let item = |id| PlanItem {
            id,
            effect: Effect::Reboot,
            changes: vec![ValueChange {
                name: "HKLM\\X\\Y".to_owned(),
                was: Some("0".to_owned()),
                becomes: Some("2".to_owned()),
            }],
        };
        let plan = OptimizePlan {
            admin: vec![item(TweakId::Hags)],
            user: vec![item(TweakId::GameMode)],
            already_done: vec![TweakId::FileExtensions],
        };
        let texts = &Locale::Ru.texts().optimize;
        let view = preview_view(&plan, Locale::Ru);
        assert!(view.needs_uac);
        assert_eq!(view.rights_line, texts.uac_line);
        assert_eq!(view.groups.len(), 2);
        assert_eq!(view.groups[0].lines[0].values[0].becomes, "2");
        assert!(view.already.is_some());
        let user_only = OptimizePlan {
            admin: Vec::new(),
            ..plan
        };
        assert_eq!(
            preview_view(&user_only, Locale::Ru).rights_line,
            texts.no_uac_line
        );
    }

    #[test]
    fn an_outcome_names_failures_the_point_and_what_to_do_next() {
        let outcome = ApplyOutcome {
            outcome: ChangeOutcome::Done,
            run: Some(1),
            items: vec![
                ItemOutcome {
                    id: TweakId::FileExtensions,
                    result: ItemResult::Done,
                },
                ItemOutcome {
                    id: TweakId::GameMode,
                    result: ItemResult::Failed {
                        message: "denied".to_owned(),
                    },
                },
            ],
            restore_point: Some(RestorePoint::ProtectionOff),
            effects: vec![Effect::ExplorerRestart, Effect::Reboot],
        };
        let texts = &Locale::En.texts().optimize;
        let view = outcome_view(&outcome, false, Locale::En);
        assert_eq!(view.tone, Tone::Warn);
        assert_eq!(view.failures.len(), 1);
        assert_eq!(view.restore.as_deref(), Some(texts.restore_off));
        assert_eq!(view.then, [texts.then_explorer, texts.then_reboot]);
        assert!(view.offers_explorer);
        let declined = ApplyOutcome {
            outcome: ChangeOutcome::Declined,
            ..outcome
        };
        assert_eq!(
            outcome_view(&declined, false, Locale::En).headline,
            texts.declined
        );
        let back = ApplyOutcome {
            outcome: ChangeOutcome::Done,
            ..declined
        };
        assert_eq!(
            outcome_view(&back, true, Locale::En).headline,
            "Reverted: 1, failed: 1."
        );
    }

    #[test]
    fn a_journal_run_reads_as_reverted_when_every_change_is() {
        let change = |reverted_at| JournalChange {
            id: TweakId::GameMode,
            change: ValueChange {
                name: "HKCU\\X\\Y".to_owned(),
                was: None,
                becomes: Some("1".to_owned()),
            },
            reverted_at,
        };
        let runs = [
            JournalRun {
                run: 2,
                time: 0,
                changes: vec![change(None)],
            },
            JournalRun {
                run: 1,
                time: 0,
                changes: vec![change(Some(5))],
            },
        ];
        let lines = journal_view(&runs, Locale::Ru);
        assert!(!lines[0].is_reverted);
        assert!(lines[1].is_reverted);
        assert_eq!(
            lines[0].changes[0].1.was,
            Locale::Ru.texts().optimize.absent
        );
    }

    #[test]
    fn every_setting_has_its_text_in_both_languages() {
        for locale in [Locale::Ru, Locale::En] {
            for id in TweakId::ALL {
                let text = locale.texts().optimize.tweaks.of(id);
                assert!(!text.title.is_empty() && !text.hint.is_empty(), "{id:?}");
            }
        }
    }
}
