//! The services block of the startup screen: other makers' services first, the ones of Microsoft and Windows behind
//! a switch, each with its start, state, publisher and what can be done with it.

use vitals_ipc::{Service, ServiceState, ServicesOverview, StartType};

use crate::i18n::Locale;
use crate::views::disks::Tone;
use crate::views::runners::Badge;
use crate::views::startup::{publisher_of, signature_badge};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceAction {
    ToManual(&'static str),
    Restore(String),
    /// No button; the reason instead
    Note(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRow {
    pub name: String,
    pub title: String,
    pub description: Option<String>,
    pub state: Badge,
    pub start: Badge,
    pub changed_note: Option<String>,
    pub publisher: String,
    pub signature: Badge,
    pub image: Option<String>,
    pub action: ServiceAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicesReady {
    pub summary: String,
    pub rows: Vec<ServiceRow>,
    /// The label of the switch that shows or hides the services of Microsoft and Windows
    pub switch_label: String,
    /// No other maker's service, and Microsoft's hidden
    pub is_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServicesView {
    Loading,
    Error(&'static str),
    Ready(ServicesReady),
}

pub fn start_word(start: StartType, locale: Locale) -> &'static str {
    let texts = &locale.texts().startup;
    match start {
        StartType::Automatic => texts.start_automatic,
        StartType::AutomaticDelayed => texts.start_delayed,
        StartType::Manual => texts.start_manual,
        StartType::Disabled => texts.start_disabled,
    }
}

fn row_of(service: &Service, locale: Locale) -> ServiceRow {
    let texts = &locale.texts().startup;
    let state = match service.state {
        ServiceState::Running => (texts.running, Tone::Ok),
        ServiceState::Stopped => (texts.stopped, Tone::Accent),
        ServiceState::Changing => (texts.changing, Tone::Warn),
    };
    let start_tone = if service.start.is_automatic() {
        Tone::Ok
    } else {
        Tone::Accent
    };
    let action = match (service.is_microsoft, service.changed_from) {
        (true, _) => ServiceAction::Note(texts.microsoft_only),
        (false, Some(original)) => {
            ServiceAction::Restore((texts.restore)(start_word(original, locale)))
        }
        (false, None) if service.start.is_automatic() => ServiceAction::ToManual(texts.to_manual),
        (false, None) => ServiceAction::Note(texts.nothing_to_change),
    };
    ServiceRow {
        name: service.name.clone(),
        title: service.display_name.clone(),
        description: service.description.clone(),
        state,
        start: (start_word(service.start, locale), start_tone),
        changed_note: service
            .changed_from
            .map(|original| (texts.changed_from)(start_word(original, locale))),
        publisher: publisher_of(
            service.signer.as_ref(),
            service.signature,
            service.company.as_ref(),
            locale,
        ),
        signature: signature_badge(service.signature, locale),
        image: service
            .image
            .as_ref()
            .map(|image| image.display().to_string()),
        action,
    }
}

pub fn services_view(
    answer: Option<Result<&ServicesOverview, &'static str>>,
    show_microsoft: bool,
    locale: Locale,
) -> ServicesView {
    let overview = match answer {
        None => return ServicesView::Loading,
        Some(Err(message)) => return ServicesView::Error(message),
        Some(Ok(overview)) => overview,
    };
    let texts = &locale.texts().startup;
    let microsoft = overview
        .services
        .iter()
        .filter(|service| service.is_microsoft)
        .count();
    let other = overview.services.len() - microsoft;
    let rows: Vec<ServiceRow> = overview
        .services
        .iter()
        .filter(|service| show_microsoft || !service.is_microsoft)
        .map(|service| row_of(service, locale))
        .collect();
    ServicesView::Ready(ServicesReady {
        summary: (texts.services_count)(other, microsoft),
        is_empty: rows.is_empty(),
        rows,
        switch_label: if show_microsoft {
            texts.hide_microsoft.to_owned()
        } else {
            (texts.show_microsoft)(microsoft)
        },
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use vitals_ipc::SignatureState;

    use super::*;

    fn service(name: &str, start: StartType, is_microsoft: bool) -> Service {
        Service {
            name: name.to_owned(),
            display_name: name.to_owned(),
            description: None,
            start,
            state: ServiceState::Running,
            image: Some(PathBuf::from(r"C:\Apps\svc.exe")),
            signature: SignatureState::Valid,
            signer: Some("Maker Inc.".to_owned()),
            company: None,
            is_microsoft,
            changed_from: None,
        }
    }

    fn ready(services: Vec<Service>, show_microsoft: bool) -> ServicesReady {
        let overview = ServicesOverview { services };
        match services_view(Some(Ok(&overview)), show_microsoft, Locale::Ru) {
            ServicesView::Ready(ready) => ready,
            other => panic!("not ready: {other:?}"),
        }
    }

    #[test]
    fn microsofts_services_wait_behind_the_switch() {
        let services = vec![
            service("nvidia", StartType::Automatic, false),
            service("wuauserv", StartType::Manual, true),
        ];
        let hidden = ready(services.clone(), false);
        assert_eq!(hidden.rows.len(), 1);
        assert_eq!(
            hidden.switch_label,
            "Показать службы Microsoft и Windows (1)"
        );
        assert_eq!(
            hidden.summary,
            "Других производителей: 1 · Microsoft и Windows: 1"
        );
        let shown = ready(services, true);
        assert_eq!(
            shown.rows[1].action,
            ServiceAction::Note("Служба Microsoft или Windows — только просмотр")
        );
    }

    #[test]
    fn an_automatic_one_goes_manual_and_a_changed_one_restores() {
        let mut changed = service("runner", StartType::Manual, false);
        changed.changed_from = Some(StartType::AutomaticDelayed);
        let ready = ready(
            vec![
                service("nvidia", StartType::Automatic, false),
                changed,
                service("steam", StartType::Manual, false),
            ],
            false,
        );
        assert_eq!(
            ready.rows[0].action,
            ServiceAction::ToManual("Запускать вручную")
        );
        assert_eq!(
            ready.rows[1].action,
            ServiceAction::Restore("Вернуть: Автоматически, отложенно".to_owned())
        );
        assert_eq!(
            ready.rows[1].changed_note.as_deref(),
            Some("Vitals поменял запуск, было: Автоматически, отложенно")
        );
        assert_eq!(
            ready.rows[2].action,
            ServiceAction::Note("Сама с Windows не стартует")
        );
    }

    #[test]
    fn nothing_of_other_makers_reads_as_empty() {
        assert!(ready(vec![service("wuauserv", StartType::Manual, true)], false).is_empty);
    }
}
