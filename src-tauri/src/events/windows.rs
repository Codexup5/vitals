//! Reads the Windows event log through `wevtapi` (`EvtQuery`, `EvtNext`, `EvtRender`).
#![allow(
    unsafe_code,
    reason = "the event log API is C: every call of it lives in this file"
)]

use chrono::{DateTime, Utc};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS};
use windows::Win32::System::EventLog::{
    EVT_HANDLE, EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryForwardDirection,
    EvtRender, EvtRenderEventXml,
};
use windows::core::HSTRING;

use super::xml::parse_event;
use super::{Log, RawEvent};

const BATCH: usize = 64;
const WAIT_FOREVER: u32 = u32::MAX;

#[derive(Debug, thiserror::Error)]
pub enum EventLogError {
    #[error("the event log refused the query: {0}")]
    Windows(#[from] windows::core::Error),
}

/// An `EVT_HANDLE` closed once, when it goes out of scope.
struct Handle(EVT_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from EvtQuery or EvtNext, is owned by this value only and is closed here once.
        if let Err(error) = unsafe { EvtClose(self.0) } {
            tracing::warn!("EvtClose failed: {error}");
        }
    }
}

/// UTF-16 buffer reused across the events of one read: most render without a new allocation.
struct Renderer {
    buffer: Vec<u16>,
}

impl Renderer {
    fn render(&mut self, event: &Handle) -> Result<String, EventLogError> {
        let mut used = 0u32;
        let mut properties = 0u32;
        let capacity = u32::try_from(self.buffer.len() * 2).unwrap_or(u32::MAX);
        // SAFETY: the buffer pointer and its byte size describe `self.buffer`, which outlives the call.
        let first = unsafe {
            EvtRender(
                None,
                event.0,
                EvtRenderEventXml.0,
                capacity,
                Some(self.buffer.as_mut_ptr().cast()),
                &raw mut used,
                &raw mut properties,
            )
        };
        match first {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {
                self.buffer.resize((used as usize).div_ceil(2), 0);
                let capacity = u32::try_from(self.buffer.len() * 2).unwrap_or(u32::MAX);
                // SAFETY: as above, with the buffer grown to the size the first call asked for.
                unsafe {
                    EvtRender(
                        None,
                        event.0,
                        EvtRenderEventXml.0,
                        capacity,
                        Some(self.buffer.as_mut_ptr().cast()),
                        &raw mut used,
                        &raw mut properties,
                    )
                }?;
            }
            Err(error) => return Err(error.into()),
        }
        let chars = (used as usize / 2).min(self.buffer.len());
        let text = &self.buffer[..chars];
        let end = text
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(text.len());
        Ok(String::from_utf16_lossy(&text[..end]))
    }
}

fn query(log: Log, xpath: &str) -> Result<Handle, EventLogError> {
    let flags = EvtQueryChannelPath.0 | EvtQueryForwardDirection.0;
    // SAFETY: both strings are valid null-terminated UTF-16 for the duration of the call.
    let handle = unsafe {
        EvtQuery(
            None,
            &HSTRING::from(log.as_str()),
            &HSTRING::from(xpath),
            flags,
        )
    }?;
    Ok(Handle(handle))
}

/// Reads every entry of `log` from `limit` on, oldest first, at most `limit` entries when given.
fn read(log: Log, xpath: &str, limit: Option<usize>) -> Result<Vec<RawEvent>, EventLogError> {
    let results = query(log, xpath)?;
    let mut renderer = Renderer {
        buffer: vec![0; 8 * 1024],
    };
    let mut events = Vec::new();
    loop {
        let mut handles = [0isize; BATCH];
        let mut returned = 0u32;
        // SAFETY: `handles` has room for BATCH handles and `returned` tells how many EvtNext wrote.
        match unsafe { EvtNext(results.0, &mut handles, WAIT_FOREVER, 0, &raw mut returned) } {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
            Err(error) => return Err(error.into()),
        }
        let batch: Vec<Handle> = handles[..returned as usize]
            .iter()
            .map(|&raw| Handle(EVT_HANDLE(raw)))
            .collect();
        for event in &batch {
            match parse_event(&renderer.render(event)?) {
                Ok(parsed) => events.push(parsed),
                Err(error) => tracing::warn!("skipped an event of {}: {error}", log.as_str()),
            }
        }
        if limit.is_some_and(|limit| events.len() >= limit) {
            events.truncate(limit.unwrap_or(events.len()));
            break;
        }
    }
    Ok(events)
}

fn xpath_time(since: DateTime<Utc>) -> String {
    since.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// System: every entry since `since` (the panel decides what to keep). Application: only crashes (1000).
pub fn read_since(log: Log, since: DateTime<Utc>) -> Result<Vec<RawEvent>, EventLogError> {
    let time = xpath_time(since);
    let xpath = match log {
        Log::System => format!("*[System[TimeCreated[@SystemTime>='{time}']]]"),
        Log::Application => {
            format!("*[System[(EventID=1000) and TimeCreated[@SystemTime>='{time}']]]")
        }
    };
    read(log, &xpath, None)
}

/// The time of the oldest entry the log still holds: days before it cannot be counted from the log.
pub fn oldest_time(log: Log) -> Result<Option<DateTime<Utc>>, EventLogError> {
    Ok(read(log, "*", Some(1))?.first().map(|event| event.time))
}
