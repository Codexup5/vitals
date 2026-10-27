//! The XML of one rendered event (`EvtRender` with `EvtRenderEventXml`) into a [`RawEvent`].

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{Log, RawEvent};

#[derive(Debug, thiserror::Error)]
pub enum EventXmlError {
    #[error("event XML does not parse: {0}")]
    Parse(#[from] quick_xml::DeError),
    #[error("event time {0:?} is not RFC 3339")]
    Time(String),
    #[error("event channel {0:?} is not one the panel reads")]
    Channel(String),
}

#[derive(Deserialize)]
struct EventXml {
    #[serde(rename = "System")]
    system: SystemXml,
    #[serde(rename = "EventData", default)]
    event_data: Option<EventDataXml>,
}

#[derive(Deserialize)]
struct SystemXml {
    #[serde(rename = "Provider")]
    provider: ProviderXml,
    #[serde(rename = "EventID")]
    event_id: TextXml<u32>,
    #[serde(rename = "Level", default)]
    level: u8,
    #[serde(rename = "TimeCreated")]
    time_created: TimeCreatedXml,
    #[serde(rename = "EventRecordID")]
    record_id: u64,
    #[serde(rename = "Channel")]
    channel: String,
}

#[derive(Deserialize)]
struct ProviderXml {
    #[serde(rename = "@Name")]
    name: String,
}

#[derive(Deserialize)]
struct TextXml<T> {
    #[serde(rename = "$text")]
    value: T,
}

#[derive(Deserialize)]
struct TimeCreatedXml {
    #[serde(rename = "@SystemTime")]
    system_time: String,
}

#[derive(Deserialize)]
struct EventDataXml {
    #[serde(rename = "Data", default)]
    data: Vec<DataXml>,
}

#[derive(Deserialize)]
struct DataXml {
    #[serde(rename = "@Name", default)]
    name: Option<String>,
    #[serde(rename = "$text", default)]
    value: String,
}

pub fn parse_event(xml: &str) -> Result<RawEvent, EventXmlError> {
    let event: EventXml = quick_xml::de::from_str(xml)?;
    let system = event.system;
    let log = match system.channel.as_str() {
        "System" => Log::System,
        "Application" => Log::Application,
        _ => return Err(EventXmlError::Channel(system.channel)),
    };
    let time = DateTime::parse_from_rfc3339(&system.time_created.system_time)
        .map_err(|_| EventXmlError::Time(system.time_created.system_time.clone()))?
        .with_timezone(&Utc);
    Ok(RawEvent {
        log,
        record_id: system.record_id,
        time,
        provider: system.provider.name,
        event_id: system.event_id.value,
        level: system.level,
        data: event
            .event_data
            .map(|event_data| {
                event_data
                    .data
                    .into_iter()
                    .map(|data| (data.name, data.value))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EventKind, classify};

    #[test]
    fn parses_a_service_crash_with_qualifiers_and_binary() {
        let event = parse_event(include_str!("fixtures/service-7031.xml")).unwrap();
        assert_eq!(event.log, Log::System);
        assert_eq!(event.event_id, 7031);
        assert_eq!(event.level, 2);
        assert_eq!(event.record_id, 84405);
        assert_eq!(
            event.time.to_rfc3339(),
            "2026-10-26T21:29:40.262774300+00:00"
        );
        assert_eq!(
            classify(&event),
            Some((
                EventKind::NvContainer,
                Some("NVIDIA LocalSystem Container".to_owned())
            ))
        );
    }

    #[test]
    fn parses_an_application_crash() {
        let event = parse_event(include_str!("fixtures/app-crash-1000.xml")).unwrap();
        assert_eq!(event.log, Log::Application);
        assert_eq!(
            classify(&event),
            Some((EventKind::AppCrash, Some("Steam.exe".to_owned())))
        );
    }

    #[test]
    fn parses_a_bsod_and_a_kernel_power_crash() {
        let bsod = parse_event(include_str!("fixtures/wer-1001.xml")).unwrap();
        assert_eq!(
            classify(&bsod),
            Some((EventKind::Bsod, Some("0x0000004e".to_owned())))
        );
        let crash = parse_event(include_str!("fixtures/kernel-power-41.xml")).unwrap();
        assert_eq!(classify(&crash), Some((EventKind::Crash, None)));
    }

    #[test]
    fn keeps_unnamed_data_and_escaped_text() {
        let event = parse_event(include_str!("fixtures/classic-unnamed.xml")).unwrap();
        assert_eq!(
            event.data,
            vec![(None, "a & b".to_owned()), (None, String::new())]
        );
    }

    #[test]
    fn rejects_another_channel_and_broken_xml() {
        let security = include_str!("fixtures/classic-unnamed.xml")
            .replace(">System</Channel>", ">Security</Channel>");
        assert!(matches!(
            parse_event(&security),
            Err(EventXmlError::Channel(_))
        ));
        assert!(matches!(
            parse_event("<Event><System>"),
            Err(EventXmlError::Parse(_))
        ));
    }
}
