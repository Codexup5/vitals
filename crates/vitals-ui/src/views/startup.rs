//! The startup screen: the counters, and a row per program with its state, cost, publisher, signature and place,
//! and what can be done with it.

use vitals_ipc::{
    ImpactLevel, SignatureState, StartupEntry, StartupImpact, StartupOverview, StartupSource,
};

use crate::format;
use crate::i18n::Locale;
use crate::views::disks::Tone;
use crate::views::network::Stat;
use crate::views::runners::Badge;

/// What the button of a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowAction {
    Toggle {
        enable: bool,
        label: &'static str,
        /// Said next to the button: UAC will ask
        admin_note: Option<&'static str>,
    },
    /// A part of Windows: no button, the reason instead
    Protected(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupRow {
    /// Unique across the sources
    pub key: String,
    pub source: StartupSource,
    pub name: String,
    /// The name without the `.lnk` of a shortcut
    pub title: String,
    pub command: String,
    pub place: &'static str,
    pub state: Badge,
    pub off_since: Option<String>,
    /// None for a program that is off: it does not start, so it costs nothing
    pub impact: Option<Badge>,
    pub impact_detail: Option<String>,
    pub signature: Badge,
    pub publisher: String,
    pub action: RowAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupReady {
    pub stats: Vec<Stat>,
    pub rows: Vec<StartupRow>,
    pub boots_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupView {
    Loading,
    Error(&'static str),
    Empty,
    Ready(StartupReady),
}

fn source_index(source: StartupSource) -> usize {
    StartupSource::ALL
        .iter()
        .position(|one| *one == source)
        .unwrap_or(0)
}

fn impact_badge(impact: Option<&StartupImpact>, locale: Locale) -> Badge {
    let texts = &locale.texts().startup;
    match impact.map(|impact| impact.level) {
        Some(ImpactLevel::High) => (texts.impact_high, Tone::Danger),
        Some(ImpactLevel::Medium) => (texts.impact_medium, Tone::Warn),
        Some(ImpactLevel::Low) => (texts.impact_low, Tone::Ok),
        None => (texts.impact_unmeasured, Tone::Accent),
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "milliseconds of CPU in a sign-in are far below 2^53"
)]
fn seconds(ms: u64) -> f64 {
    ms as f64 / 1000.0
}

/// Below a second in milliseconds, below a megabyte in kilobytes: a calm program does not read as zero.
fn impact_detail(impact: &StartupImpact, locale: Locale) -> String {
    let texts = &locale.texts().startup;
    let cpu = if impact.cpu_ms < 1000 {
        (texts.milliseconds)(&format::count(impact.cpu_ms, locale))
    } else {
        (texts.seconds)(&format::decimal(seconds(impact.cpu_ms), locale))
    };
    let disk = if impact.disk_bytes < 1024 * 1024 {
        (texts.kilobytes)(&format::count(impact.disk_bytes.div_ceil(1024), locale))
    } else {
        format::bytes(impact.disk_bytes, locale)
    };
    (texts.impact_detail)(&cpu, &disk, impact.boots)
}

fn signature_badge(state: SignatureState, locale: Locale) -> Badge {
    let texts = &locale.texts().startup;
    match state {
        SignatureState::Valid => (texts.signature_valid, Tone::Ok),
        SignatureState::Invalid => (texts.signature_invalid, Tone::Danger),
        SignatureState::Unsigned => (texts.signature_unsigned, Tone::Warn),
        SignatureState::StoreApp => (texts.signature_store, Tone::Accent),
        SignatureState::NoFile => (texts.signature_no_file, Tone::Warn),
    }
}

/// The signer when the signature holds; else what the file says of itself, said to be unverified.
fn publisher_of(entry: &StartupEntry, locale: Locale) -> String {
    let texts = &locale.texts().startup;
    match (&entry.signer, entry.signature, &entry.company) {
        (Some(signer), SignatureState::Valid, _) => signer.clone(),
        (_, _, Some(company)) => (texts.publisher_unverified)(company),
        _ => texts.publisher_unknown.to_owned(),
    }
}

fn title_of(name: &str) -> String {
    name.strip_suffix(".lnk")
        .or_else(|| name.strip_suffix(".LNK"))
        .unwrap_or(name)
        .to_owned()
}

fn row_of(entry: &StartupEntry, locale: Locale) -> StartupRow {
    let texts = &locale.texts().startup;
    let action = if entry.is_protected {
        RowAction::Protected(texts.protected)
    } else {
        RowAction::Toggle {
            enable: !entry.is_enabled,
            label: if entry.is_enabled {
                texts.turn_off
            } else {
                texts.turn_on
            },
            admin_note: entry.source.is_machine().then_some(texts.needs_admin),
        }
    };
    StartupRow {
        key: format!("{}:{}", source_index(entry.source), entry.name),
        source: entry.source,
        name: entry.name.clone(),
        title: title_of(&entry.name),
        command: entry.command.clone(),
        place: texts.sources[source_index(entry.source)],
        state: if entry.is_enabled {
            (texts.on, Tone::Ok)
        } else {
            (texts.off, Tone::Accent)
        },
        off_since: entry
            .disabled_at
            .and_then(format::local_time)
            .map(|time| (texts.off_since)(&format::date(time.date(), locale))),
        impact: entry
            .is_enabled
            .then(|| impact_badge(entry.impact.as_ref(), locale)),
        impact_detail: entry
            .impact
            .as_ref()
            .map(|impact| impact_detail(impact, locale)),
        signature: signature_badge(entry.signature, locale),
        publisher: publisher_of(entry, locale),
        action,
    }
}

fn stat(label: &str, value: usize, tone: Option<Tone>) -> Stat {
    Stat {
        label: label.to_owned(),
        value: value.to_string(),
        hint: None,
        tone: tone.filter(|_| value > 0),
    }
}

pub fn startup_view(
    answer: Option<Result<&StartupOverview, &'static str>>,
    locale: Locale,
) -> StartupView {
    let overview = match answer {
        None => return StartupView::Loading,
        Some(Err(message)) => return StartupView::Error(message),
        Some(Ok(overview)) => overview,
    };
    if overview.entries.is_empty() {
        return StartupView::Empty;
    }
    let texts = &locale.texts().startup;
    let entries = &overview.entries;
    let count =
        |test: fn(&StartupEntry) -> bool| entries.iter().filter(|entry| test(entry)).count();
    let stats = vec![
        stat(texts.stat_on, count(|entry| entry.is_enabled), None),
        stat(
            texts.stat_high,
            count(|entry| {
                entry
                    .impact
                    .is_some_and(|impact| impact.level == ImpactLevel::High)
            }),
            Some(Tone::Warn),
        ),
        stat(texts.stat_off, count(|entry| !entry.is_enabled), None),
        stat(
            texts.stat_unsigned,
            count(|entry| {
                matches!(
                    entry.signature,
                    SignatureState::Unsigned | SignatureState::Invalid
                )
            }),
            Some(Tone::Warn),
        ),
    ];
    StartupView::Ready(StartupReady {
        stats,
        rows: entries.iter().map(|entry| row_of(entry, locale)).collect(),
        boots_note: if overview.measured_boots == 0 {
            texts.no_boots.to_owned()
        } else {
            (texts.boots)(overview.measured_boots)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, source: StartupSource) -> StartupEntry {
        StartupEntry {
            source,
            name: name.to_owned(),
            command: format!(r#""C:\Apps\{name}.exe""#),
            image: None,
            is_enabled: true,
            disabled_at: None,
            signature: SignatureState::Valid,
            signer: Some("Maker Inc.".to_owned()),
            company: Some("Maker".to_owned()),
            is_protected: false,
            impact: Some(StartupImpact {
                level: ImpactLevel::High,
                cpu_ms: 2361,
                disk_bytes: 72_855_552,
                boots: 5,
            }),
        }
    }

    fn ready(entries: Vec<StartupEntry>) -> StartupReady {
        let overview = StartupOverview {
            entries,
            measured_boots: 5,
        };
        match startup_view(Some(Ok(&overview)), Locale::Ru) {
            StartupView::Ready(ready) => ready,
            other => panic!("not ready: {other:?}"),
        }
    }

    #[test]
    fn the_states_before_the_list() {
        assert_eq!(startup_view(None, Locale::Ru), StartupView::Loading);
        assert_eq!(
            startup_view(Some(Err("x")), Locale::Ru),
            StartupView::Error("x")
        );
        let empty = StartupOverview {
            entries: Vec::new(),
            measured_boots: 0,
        };
        assert_eq!(
            startup_view(Some(Ok(&empty)), Locale::Ru),
            StartupView::Empty
        );
    }

    #[test]
    fn a_row_says_its_cost_and_who_signed_it() {
        let ready = ready(vec![entry("LGHUB", StartupSource::UserRun)]);
        let row = &ready.rows[0];
        assert_eq!(row.impact, Some(("Влияние высокое", Tone::Danger)));
        assert_eq!(
            row.impact_detail.as_deref(),
            Some("CPU 2,4 с · диск 69,5 МБ · входов: 5")
        );
        assert_eq!(row.publisher, "Maker Inc.");
        assert_eq!(
            row.action,
            RowAction::Toggle {
                enable: false,
                label: "Выключить",
                admin_note: None
            }
        );
        assert_eq!(ready.stats[1].value, "1");
    }

    #[test]
    fn an_entry_of_everyone_warns_of_uac_and_windows_has_no_button() {
        let mut machine = entry("Everything", StartupSource::MachineRun);
        machine.is_enabled = false;
        let mut windows = entry("SecurityHealth", StartupSource::MachineRun);
        windows.is_protected = true;
        let ready = ready(vec![machine, windows]);
        assert_eq!(
            ready.rows[0].action,
            RowAction::Toggle {
                enable: true,
                label: "Включить",
                admin_note: Some("Windows спросит права администратора")
            }
        );
        assert_eq!(
            ready.rows[1].action,
            RowAction::Protected("Часть Windows — только просмотр")
        );
    }

    #[test]
    fn a_calm_program_does_not_read_as_zero_and_an_off_one_has_no_impact() {
        let mut calm = entry("SecurityHealth", StartupSource::MachineRun);
        calm.impact = Some(StartupImpact {
            level: ImpactLevel::Low,
            cpu_ms: 38,
            disk_bytes: 10_240,
            boots: 5,
        });
        let mut off = entry("Steam", StartupSource::UserRun);
        off.is_enabled = false;
        let ready = ready(vec![calm, off]);
        assert_eq!(
            ready.rows[0].impact_detail.as_deref(),
            Some("CPU 38 мс · диск 10 КБ · входов: 5")
        );
        assert_eq!(ready.rows[1].impact, None);
    }

    #[test]
    fn an_unsigned_file_is_published_by_its_own_word_only() {
        let mut share = entry("ShareX.lnk", StartupSource::UserFolder);
        share.signature = SignatureState::Unsigned;
        share.signer = None;
        share.company = Some("ShareX Team".to_owned());
        share.impact = None;
        let ready = ready(vec![share]);
        let row = &ready.rows[0];
        assert_eq!(row.title, "ShareX");
        assert_eq!(row.publisher, "ShareX Team — со слов файла, подписи нет");
        assert_eq!(row.impact, Some(("Влияние не измерено", Tone::Accent)));
        assert_eq!(ready.stats[3].value, "1");
        assert_eq!(ready.stats[3].tone, Some(Tone::Warn));
    }
}
