//! Sending to the user's own bot through the `curl` that ships with Windows. The token never stands on a command
//! line, where any process could read it: it goes in a curl config file in `%APPDATA%\Vitals`, removed after the call,
//! and the message in a UTF-8 file next to it (curl's arguments mangle Cyrillic, the brief §5 warns).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "telegram-request.cfg";
const BODY_FILE: &str = "telegram-body.json";
const DEADLINE: Duration = Duration::from_secs(30);

/// The bot the user made, and the chat it writes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bot {
    pub token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    #[error("Telegram refused the message: a wrong token or chat")]
    Refused,
    #[error("Telegram did not answer: {0}")]
    Unreachable(String),
}

/// `123456:AAH…`: digits, a colon, then letters, digits, `_` and `-`. Anything else could break out of the config.
pub fn is_token(text: &str) -> bool {
    text.split_once(':').is_some_and(|(id, secret)| {
        !id.is_empty()
            && id.bytes().all(|byte| byte.is_ascii_digit())
            && !secret.is_empty()
            && secret
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    })
}

/// A numeric chat id (negative for a group) or a `@channel` name.
pub fn is_chat_id(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let is_number = !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit());
    let is_channel = text.strip_prefix('@').is_some_and(|name| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    });
    is_number || is_channel
}

impl Bot {
    /// A bot only when both values are of the right shape.
    pub fn new(token: &str, chat_id: &str) -> Option<Self> {
        let (token, chat_id) = (token.trim(), chat_id.trim());
        (is_token(token) && is_chat_id(chat_id)).then(|| Self {
            token: token.to_owned(),
            chat_id: chat_id.to_owned(),
        })
    }

    /// The last four characters of the chat: enough to tell which one it is.
    pub fn chat_hint(&self) -> String {
        let count = self.chat_id.chars().count();
        let tail: String = self.chat_id.chars().skip(count.saturating_sub(4)).collect();
        format!("…{tail}")
    }
}

/// `TELEGRAM_BOT_TOKEN=…` and `TELEGRAM_CHAT_ID=…` of the old `telegram.env`.
pub fn parse_env(text: &str) -> Option<Bot> {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| value.trim().trim_matches('"').to_owned())
        })
    };
    Bot::new(&value("TELEGRAM_BOT_TOKEN")?, &value("TELEGRAM_CHAT_ID")?)
}

/// The old alerts' bot file of OLEGPC, when it is there.
pub fn legacy_env() -> Option<std::path::PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    let path = Path::new(&profile).join(".olegpc").join("telegram.env");
    path.is_file().then_some(path)
}

/// Telegram's answer says `"ok": true` when the message went.
pub fn is_accepted(answer: &str) -> bool {
    #[derive(Deserialize)]
    struct Answer {
        ok: bool,
    }
    serde_json::from_str::<Answer>(answer).is_ok_and(|answer| answer.ok)
}

fn config_text(bot: &Bot, body: &Path) -> String {
    // Forward slashes: a backslash in a quoted curl config value starts an escape
    let body = body.display().to_string().replace('\\', "/");
    format!(
        "url = \"https://api.telegram.org/bot{}/sendMessage\"\nheader = \"Content-Type: application/json; charset=utf-8\"\ndata-binary = \"@{body}\"\nsilent\nmax-time = 20\n",
        bot.token
    )
}

pub fn send(bot: &Bot, text: &str, work_dir: &Path) -> Result<(), SendError> {
    let unreachable = |error: std::io::Error| SendError::Unreachable(error.to_string());
    let body_path = work_dir.join(BODY_FILE);
    let config_path = work_dir.join(CONFIG_FILE);
    let body = serde_json::json!({ "chat_id": bot.chat_id, "text": text }).to_string();
    std::fs::write(&body_path, body).map_err(unreachable)?;
    std::fs::write(&config_path, config_text(bot, &body_path)).map_err(unreachable)?;
    let config = config_path.display().to_string();
    let outcome = crate::process::output("curl", &["--config", &config], DEADLINE);
    let _ = std::fs::remove_file(&config_path);
    let _ = std::fs::remove_file(&body_path);
    let done = outcome.map_err(|error| SendError::Unreachable(error.to_string()))?;
    if is_accepted(&done.stdout) {
        Ok(())
    } else if done.stdout.trim().is_empty() {
        Err(SendError::Unreachable(format!(
            "curl exited with {}",
            done.code
        )))
    } else {
        Err(SendError::Refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_well_formed_tokens_and_chats_make_a_bot() {
        assert!(Bot::new("123456:AAH-x_9", "987654").is_some());
        assert!(Bot::new(" 123456:AAH ", "-100123").is_some());
        assert!(Bot::new("123456:AAH", "@my_channel").is_some());
        assert!(Bot::new("123456:AA\"H", "1").is_none());
        assert!(Bot::new("abc:AAH", "1").is_none());
        assert!(Bot::new("1:x\nurl = \"http://evil\"", "1").is_none());
        assert!(Bot::new("1:x", "12a").is_none());
    }

    #[test]
    fn the_old_env_file_gives_the_bot() {
        let text = "TELEGRAM_BOT_TOKEN=123:abc\r\nTELEGRAM_CHAT_ID = 4567890\r\n";
        let bot = parse_env(text).unwrap();
        assert_eq!(bot.chat_id, "4567890");
        assert_eq!(bot.chat_hint(), "…7890");
        assert!(parse_env("TELEGRAM_BOT_TOKEN=123:abc").is_none());
    }

    #[test]
    fn telegrams_answer_is_read_by_its_ok() {
        assert!(is_accepted(r#"{"ok":true,"result":{"message_id":1}}"#));
        assert!(!is_accepted(
            r#"{"ok":false,"error_code":401,"description":"Unauthorized"}"#
        ));
        assert!(!is_accepted(""));
    }

    #[test]
    fn the_config_names_the_body_with_forward_slashes() {
        let bot = Bot::new("1:x", "2").unwrap();
        let text = config_text(
            &bot,
            Path::new(r"C:\Users\someone\AppData\Roaming\Vitals\telegram-body.json"),
        );
        assert!(text.contains(
            r#"data-binary = "@C:/Users/someone/AppData/Roaming/Vitals/telegram-body.json""#
        ));
        assert!(text.contains("/bot1:x/sendMessage"));
    }
}
