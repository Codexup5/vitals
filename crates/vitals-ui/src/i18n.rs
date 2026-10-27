//! The two languages of the panel. Every text lives in `texts.rs`: `RU` and `EN` are values of one struct, so a
//! missing text in either language is a compile error.

use crate::texts::{EN, RU, Texts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Ru,
    En,
}

impl Locale {
    /// Russian for any `ru*` language of Windows, English otherwise.
    pub fn detect(language: &str) -> Self {
        if language.to_lowercase().starts_with("ru") {
            Self::Ru
        } else {
            Self::En
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "ru" => Some(Self::Ru),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }

    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Ru => Self::En,
            Self::En => Self::Ru,
        }
    }

    pub const fn texts(self) -> &'static Texts {
        match self {
            Self::Ru => &RU,
            Self::En => &EN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_windows_gets_russian() {
        assert_eq!(Locale::detect("ru-RU"), Locale::Ru);
        assert_eq!(Locale::detect("RU"), Locale::Ru);
        assert_eq!(Locale::detect("en-US"), Locale::En);
        assert_eq!(Locale::detect("kk-KZ"), Locale::En);
    }

    #[test]
    fn a_code_reads_back() {
        for locale in [Locale::Ru, Locale::En] {
            assert_eq!(Locale::parse(locale.code()), Some(locale));
        }
        assert_eq!(Locale::parse("de"), None);
    }
}
