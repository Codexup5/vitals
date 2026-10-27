//! Numbers, sizes and dates as each language writes them: `1 234,5` and `27.10.2026, 22:44` in Russian,
//! `1,234.5 GB` and `27/10/2026, 22:44` in English. Pure functions: the window and the tests format the same way.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};

use crate::i18n::Locale;

const GIGABYTE: f64 = 1024.0 * 1024.0 * 1024.0;
const MEGABYTE: f64 = 1024.0 * 1024.0;
/// Russian groups thousands with a no-break space, as `Intl` does
const RU_GROUP: char = '\u{a0}';

fn group_digits(digits: &str, separator: char) -> String {
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(separator);
        }
        grouped.push(digit);
    }
    grouped
}

/// A whole number with the thousands grouped: `35 733` / `35,733`.
pub fn count(value: u64, locale: Locale) -> String {
    let separator = match locale {
        Locale::Ru => RU_GROUP,
        Locale::En => ',',
    };
    group_digits(&value.to_string(), separator)
}

/// One digit after the point, always shown: `207,6` / `207.6`.
pub fn decimal(value: f64, locale: Locale) -> String {
    let text = format!("{:.1}", value.max(0.0));
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, "0"));
    let (group, point) = match locale {
        Locale::Ru => (RU_GROUP, ','),
        Locale::En => (',', '.'),
    };
    format!("{}{point}{fraction}", group_digits(whole, group))
}

#[allow(
    clippy::cast_precision_loss,
    reason = "a size is shown with one decimal; a byte count past 2^53 is beyond any disk"
)]
fn as_float(bytes: u64) -> f64 {
    bytes as f64
}

/// `207,6` in Russian, `207.6 GB` in English; below a gigabyte in megabytes, as Explorer shows it (binary units).
pub fn bytes(bytes: u64, locale: Locale) -> String {
    let texts = &locale.texts().common;
    let value = as_float(bytes);
    if value >= GIGABYTE {
        format!("{} {}", decimal(value / GIGABYTE, locale), texts.gigabytes)
    } else {
        format!("{} {}", decimal(value / MEGABYTE, locale), texts.megabytes)
    }
}

/// Unix seconds from the core as the local time of this PC.
pub fn local_time(seconds: i64) -> Option<NaiveDateTime> {
    DateTime::from_timestamp(seconds, 0)
        .map(|utc| Local.from_utc_datetime(&utc.naive_utc()).naive_local())
}

/// `27.10.2026, 22:44` / `27/10/2026, 22:44`
pub fn date_time(time: NaiveDateTime, locale: Locale) -> String {
    format!(
        "{}, {:02}:{:02}",
        date(time.date(), locale),
        time.hour(),
        time.minute()
    )
}

/// `27.10.2026` / `27/10/2026`
pub fn date(day: NaiveDate, locale: Locale) -> String {
    let separator = separator_of(locale);
    format!(
        "{:02}{separator}{:02}{separator}{}",
        day.day(),
        day.month(),
        day.year()
    )
}

const fn separator_of(locale: Locale) -> char {
    match locale {
        Locale::Ru => '.',
        Locale::En => '/',
    }
}

/// `27.10` from `2026-10-27`; the text itself when it is not a day.
pub fn calendar_day(day: &str, locale: Locale) -> String {
    NaiveDate::parse_from_str(day, "%Y-%m-%d").map_or_else(
        |_| day.to_owned(),
        |day| format!("{:02}{}{:02}", day.day(), separator_of(locale), day.month()),
    )
}

/// The short month and the year, `Oct 2026` from `2026-10`; the text itself when it is not a month.
pub fn calendar_month(month: &str, locale: Locale) -> String {
    NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").map_or_else(
        |_| month.to_owned(),
        |day| {
            let index = usize::try_from(day.month0()).unwrap_or_default();
            format!("{} {}", locale.texts().months[index], day.year())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_group_thousands_by_language() {
        assert_eq!(count(35_733, Locale::Ru), "35\u{a0}733");
        assert_eq!(count(35_733, Locale::En), "35,733");
        assert_eq!(count(999, Locale::En), "999");
        assert_eq!(count(1_234_567, Locale::En), "1,234,567");
    }

    #[test]
    fn sizes_use_binary_units_and_one_decimal() {
        let gigabytes = Locale::Ru.texts().common.gigabytes;
        assert_eq!(
            bytes(287_384_174_592, Locale::Ru),
            format!("267,6 {gigabytes}")
        );
        assert_eq!(bytes(287_384_174_592, Locale::En), "267.6 GB");
        assert_eq!(bytes(3 * 1024 * 1024, Locale::En), "3.0 MB");
        assert_eq!(
            bytes(1_500 * 1024 * 1024 * 1024, Locale::Ru),
            format!("1\u{a0}500,0 {gigabytes}")
        );
    }

    #[test]
    fn dates_follow_the_language() {
        let time = NaiveDate::from_ymd_opt(2026, 10, 7)
            .and_then(|day| day.and_hms_opt(9, 5, 0))
            .unwrap();
        assert_eq!(date_time(time, Locale::Ru), "07.10.2026, 09:05");
        assert_eq!(date_time(time, Locale::En), "07/10/2026, 09:05");
        assert_eq!(calendar_day("2026-10-27", Locale::Ru), "27.10");
        assert_eq!(
            calendar_month("2026-10", Locale::Ru),
            format!("{} 2026", Locale::Ru.texts().months[9])
        );
        assert_eq!(calendar_month("2026-10", Locale::En), "Oct 2026");
        assert_eq!(calendar_month("broken", Locale::En), "broken");
    }
}
