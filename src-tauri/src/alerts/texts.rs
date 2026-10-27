//! The words of the Telegram messages, in Russian and English as Windows is set, like the tray. `RU` and `EN` are
//! values of one struct: a text missing in either does not compile.

use chrono::{Local, TimeZone};

use super::{Alert, DISK_MIN_GB, RUNNER_GRACE_MINUTES};

#[derive(Debug)]
pub struct AlertTexts {
    pub crashes: fn(usize, &str) -> String,
    pub bsod: fn(&str) -> String,
    pub no_code: &'static str,
    pub whea: fn(u32) -> String,
    pub gpu_resets: fn(u32) -> String,
    pub low_disk: fn(f64, u32) -> String,
    pub runners_offline: fn(u32, &str) -> String,
    pub runners_back: &'static str,
    pub test: &'static str,
}

const RU: AlertTexts = AlertTexts {
    crashes: |count, last| format!("⚠️ Аварийная перезагрузка: {count}, последняя {last}"),
    bsod: |codes| format!("🟥 Синий экран: {codes}"),
    no_code: "код не найден",
    whea: |count| format!("🟧 Аппаратные ошибки WHEA: {count} — возможна память или процессор"),
    gpu_resets: |count| format!("🟨 Сбросы видеодрайвера NVIDIA: {count}"),
    low_disk: |free, min| {
        let free = format!("{free:.1}").replace('.', ",");
        format!("💾 На C: свободно {free} ГБ (порог {min} ГБ)")
    },
    runners_offline: |minutes, names| format!("🔌 Раннеры offline дольше {minutes} мин: {names}"),
    runners_back: "✅ Раннеры снова online",
    test: "✅ Vitals: тестовое сообщение. Оповещения будут приходить сюда.",
};

const EN: AlertTexts = AlertTexts {
    crashes: |count, last| format!("⚠️ Unexpected restarts: {count}, the last at {last}"),
    bsod: |codes| format!("🟥 Blue screen: {codes}"),
    no_code: "no code found",
    whea: |count| format!("🟧 WHEA hardware errors: {count}: memory or the CPU may be failing"),
    gpu_resets: |count| format!("🟨 NVIDIA driver resets: {count}"),
    low_disk: |free, min| format!("💾 C: has {free:.1} GB free (the bar is {min} GB)"),
    runners_offline: |minutes, names| format!("🔌 Runners offline for over {minutes} min: {names}"),
    runners_back: "✅ The runners are online again",
    test: "✅ Vitals: a test message. The alerts will come here.",
};

impl AlertTexts {
    pub const fn of(is_russian: bool) -> &'static Self {
        if is_russian { &RU } else { &EN }
    }
}

/// `28.10 14:28` in the time of this PC, as the old alerts wrote it.
fn short_time(seconds: i64) -> String {
    Local
        .timestamp_opt(seconds, 0)
        .single()
        .map_or_else(String::new, |time| time.format("%d.%m %H:%M").to_string())
}

pub fn lines(alerts: &[Alert], texts: &AlertTexts) -> Vec<String> {
    alerts
        .iter()
        .map(|alert| match alert {
            Alert::Crashes { count, last } => (texts.crashes)(*count, &short_time(*last)),
            Alert::Bsod { codes } => (texts.bsod)(
                &codes
                    .iter()
                    .map(|code| code.as_deref().unwrap_or(texts.no_code))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            Alert::Whea { count } => (texts.whea)(*count),
            Alert::GpuResets { count } => (texts.gpu_resets)(*count),
            Alert::LowDisk { free_gb } => (texts.low_disk)(*free_gb, DISK_MIN_GB),
            Alert::RunnersOffline { names } => {
                (texts.runners_offline)(RUNNER_GRACE_MINUTES, &names.join(", "))
            }
            Alert::RunnersBack => texts.runners_back.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_read_as_the_old_alerts_did() {
        let alerts = [
            Alert::Bsod {
                codes: vec![Some("0x0000004e".to_owned()), None],
            },
            Alert::LowDisk { free_gb: 42.5 },
            Alert::RunnersOffline {
                names: vec!["kit-linux-1".to_owned(), "kit-linux-2".to_owned()],
            },
        ];
        assert_eq!(
            lines(&alerts, AlertTexts::of(true)),
            [
                "🟥 Синий экран: 0x0000004e, код не найден",
                "💾 На C: свободно 42,5 ГБ (порог 50 ГБ)",
                "🔌 Раннеры offline дольше 30 мин: kit-linux-1, kit-linux-2",
            ]
        );
        assert_eq!(
            lines(&[Alert::RunnersBack], AlertTexts::of(false)),
            ["✅ The runners are online again"]
        );
    }
}
