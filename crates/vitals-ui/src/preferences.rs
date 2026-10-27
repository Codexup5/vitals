//! The language and the theme of the window: Mocha by default, Latte on `data-theme='light'`. They survive a restart
//! in the `WebView` storage; when the storage is blocked they last for the session only.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::i18n::Locale;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preferences {
    pub locale: Locale,
    pub theme: Theme,
}

/// What the storage holds; a field that does not read falls back on its own
#[derive(Debug, Deserialize)]
struct Stored {
    #[serde(default)]
    locale: Option<String>,
    #[serde(default)]
    theme: Option<serde_json::Value>,
}

/// The stored preferences, field by field; the language of Windows and the dark theme fill what is missing.
pub fn parse(stored: Option<&str>, language: &str) -> Preferences {
    let fallback = Preferences {
        locale: Locale::detect(language),
        theme: Theme::Dark,
    };
    let Some(stored) = stored.and_then(|text| serde_json::from_str::<Stored>(text).ok()) else {
        return fallback;
    };
    Preferences {
        locale: stored
            .locale
            .as_deref()
            .and_then(Locale::parse)
            .unwrap_or(fallback.locale),
        theme: stored
            .theme
            .and_then(|theme| serde_json::from_value(theme).ok())
            .unwrap_or(fallback.theme),
    }
}

fn serialize(preferences: Preferences) -> String {
    let stored = serde_json::json!({ "locale": preferences.locale.code(), "theme": preferences.theme.code() });
    stored.to_string()
}

#[cfg(target_arch = "wasm32")]
mod window {
    use super::{Preferences, parse, serialize};

    const STORAGE_KEY: &str = "vitals.preferences";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub fn load() -> Preferences {
        let language = web_sys::window()
            .and_then(|window| window.navigator().language())
            .unwrap_or_default();
        let stored = storage().and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten());
        parse(stored.as_deref(), &language)
    }

    pub fn save(preferences: Preferences) {
        if let Some(storage) = storage() {
            // A blocked storage keeps the choice for this session only
            let _ = storage.set_item(STORAGE_KEY, &serialize(preferences));
        }
    }

    pub fn apply(preferences: Preferences) {
        let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        else {
            return;
        };
        let _ = root.set_attribute("lang", preferences.locale.code());
        let _ = root.set_attribute("data-theme", preferences.theme.code());
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod window {
    use super::{Preferences, parse};

    pub fn load() -> Preferences {
        parse(None, "en")
    }

    pub fn save(preferences: Preferences) {
        let _ = super::serialize(preferences);
    }

    pub const fn apply(_: Preferences) {}
}

/// The preferences of the window, shared through the Leptos context.
#[derive(Debug, Clone, Copy)]
pub struct PreferencesStore(RwSignal<Preferences>);

impl PreferencesStore {
    pub fn provide() -> Self {
        let preferences = window::load();
        window::apply(preferences);
        let store = Self(RwSignal::new(preferences));
        provide_context(store);
        store
    }

    pub fn get() -> Self {
        use_context::<Self>().unwrap_or_else(Self::provide)
    }

    pub fn locale(self) -> Locale {
        self.0.get().locale
    }

    pub fn theme(self) -> Theme {
        self.0.get().theme
    }

    pub fn update(self, change: impl FnOnce(&mut Preferences)) {
        self.0.update(change);
        let preferences = self.0.get_untracked();
        window::save(preferences);
        window::apply(preferences);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_windows_language_and_the_dark_theme_when_nothing_is_stored() {
        assert_eq!(
            parse(None, "ru-RU"),
            Preferences {
                locale: Locale::Ru,
                theme: Theme::Dark
            }
        );
    }

    #[test]
    fn keeps_stored_values() {
        let stored = serialize(Preferences {
            locale: Locale::En,
            theme: Theme::Light,
        });
        assert_eq!(
            parse(Some(&stored), "ru-RU"),
            Preferences {
                locale: Locale::En,
                theme: Theme::Light
            }
        );
    }

    #[test]
    fn replaces_an_unknown_value_field_by_field() {
        let parsed = parse(Some(r#"{"locale":"de","theme":"light"}"#), "ru-RU");
        assert_eq!(parsed.locale, Locale::Ru);
        assert_eq!(parsed.theme, Theme::Light);
        assert_eq!(parse(Some("not json"), "en-US").locale, Locale::En);
        assert_eq!(parse(Some(r#"{"theme":42}"#), "en").theme, Theme::Dark);
    }
}
