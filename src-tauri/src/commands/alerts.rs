//! The alerts settings: the bot, a test message, sending on or off, and what the recent checks found.

use tauri::State;
use vitals_ipc::{AlertsStatus, TestOutcome};

use super::{AppState, CommandError, open};
use crate::alerts::{self, telegram};

#[cfg(windows)]
fn load_bot(state: &AppState) -> Option<telegram::Bot> {
    alerts::windows::load_bot(&state.data_dir)
}

#[cfg(not(windows))]
fn load_bot(_: &AppState) -> Option<telegram::Bot> {
    None
}

#[cfg(windows)]
fn save_bot(state: &AppState, bot: &telegram::Bot) -> Result<(), CommandError> {
    Ok(alerts::windows::save_bot(&state.data_dir, bot)?)
}

#[cfg(not(windows))]
fn save_bot(_: &AppState, _: &telegram::Bot) -> Result<(), CommandError> {
    Err(CommandError::Invalid("only on Windows".to_owned()))
}

#[tauri::command(async)]
pub fn alerts_status(state: State<'_, AppState>) -> Result<AlertsStatus, CommandError> {
    let storage = open(&state)?;
    let bot = load_bot(&state);
    let now = chrono::Utc::now().timestamp();
    let last_check = storage
        .setting("alerts.state")?
        .is_some()
        .then(|| alerts::read_state(&storage, now).ok())
        .flatten()
        .and_then(|memory| memory.last_check);
    Ok(AlertsStatus {
        has_bot: bot.is_some(),
        chat_hint: bot.as_ref().map(telegram::Bot::chat_hint),
        is_sending: alerts::is_sending(&storage)?,
        has_legacy_bot: telegram::legacy_env().is_some(),
        #[cfg(windows)]
        old_tasks_on: crate::legacy::tasks_on(),
        #[cfg(not(windows))]
        old_tasks_on: Vec::new(),
        disk_min_gb: alerts::DISK_MIN_GB,
        runner_grace_minutes: alerts::RUNNER_GRACE_MINUTES,
        last_check,
        recent: alerts::recent(&state.data_dir),
    })
}

/// Keeps the bot when both values have the right shape; the token is never logged.
#[tauri::command]
pub fn alerts_save_bot(
    state: State<'_, AppState>,
    token: &str,
    chat_id: &str,
) -> Result<(), CommandError> {
    let bot = telegram::Bot::new(token, chat_id)
        .ok_or_else(|| CommandError::Invalid("not a bot token and chat".to_owned()))?;
    save_bot(&state, &bot)
}

/// Takes the bot of the old OLEGPC alerts; false when the file is missing or malformed.
#[tauri::command]
pub fn alerts_import_legacy(state: State<'_, AppState>) -> Result<bool, CommandError> {
    let Some(bot) = telegram::legacy_env()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| telegram::parse_env(&text))
    else {
        return Ok(false);
    };
    save_bot(&state, &bot)?;
    Ok(true)
}

#[tauri::command]
pub fn alerts_forget_bot(state: State<'_, AppState>) -> Result<(), CommandError> {
    #[cfg(windows)]
    alerts::windows::forget_bot(&state.data_dir)?;
    open(&state)?.set_setting(alerts::SENDING_KEY, "false")?;
    Ok(())
}

/// Sends the test message now, whether sending is on or off.
#[tauri::command(async)]
pub fn alerts_test(state: State<'_, AppState>, is_russian: bool) -> TestOutcome {
    let Some(bot) = load_bot(&state) else {
        return TestOutcome::NoBot;
    };
    let text = alerts::message(
        &alerts::computer_name(),
        &[alerts::AlertTexts::of(is_russian).test.to_owned()],
    );
    match telegram::send(&bot, &text, &state.data_dir) {
        Ok(()) => TestOutcome::Sent,
        Err(telegram::SendError::Refused) => TestOutcome::Refused,
        Err(error) => {
            tracing::warn!("the test message did not go: {error}");
            TestOutcome::Unreachable
        }
    }
}

#[tauri::command]
pub fn alerts_set_sending(
    state: State<'_, AppState>,
    is_sending: bool,
) -> Result<(), CommandError> {
    open(&state)?.set_setting(
        alerts::SENDING_KEY,
        if is_sending { "true" } else { "false" },
    )?;
    tracing::info!("alerts sending: {is_sending}");
    Ok(())
}
