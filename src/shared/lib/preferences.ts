import { detectLocale, isLocale, type Locale } from './i18n';

const THEMES = ['dark', 'light'] as const;

type Theme = (typeof THEMES)[number];

export interface Preferences {
  locale: Locale;
  theme: Theme;
}

const STORAGE_KEY = 'vitals.preferences';

const isTheme = (value: unknown): value is Theme => THEMES.some((theme) => theme === value);

const fieldOf = (value: object, key: string): unknown => (key in value ? Reflect.get(value, key) : undefined);

export const parsePreferences = (stored: string | null, language: string): Preferences => {
  const fallback: Preferences = { locale: detectLocale(language), theme: 'dark' };
  if (stored === null) return fallback;
  try {
    const parsed: unknown = JSON.parse(stored);
    if (typeof parsed !== 'object' || parsed === null) return fallback;
    const locale = fieldOf(parsed, 'locale');
    const theme = fieldOf(parsed, 'theme');
    return {
      locale: isLocale(locale) ? locale : fallback.locale,
      theme: isTheme(theme) ? theme : fallback.theme,
    };
  } catch (error) {
    if (error instanceof SyntaxError) return fallback;
    throw error;
  }
};

// WebView storage can be blocked: the panel then keeps the preferences for the session only
const readStored = () => {
  try {
    return window.localStorage.getItem(STORAGE_KEY);
  } catch (error) {
    if (error instanceof DOMException) return null;
    throw error;
  }
};

const writeStored = (preferences: Preferences) => {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
  } catch (error) {
    if (!(error instanceof DOMException)) throw error;
  }
};

const applyToDocument = ({ locale, theme }: Preferences) => {
  document.documentElement.lang = locale;
  document.documentElement.dataset.theme = theme;
};

let current = parsePreferences(readStored(), navigator.language);
const listeners = new Set<() => void>();

export const getPreferences = () => current;

export const subscribePreferences = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

export const updatePreferences = (patch: Partial<Preferences>) => {
  current = { ...current, ...patch };
  writeStored(current);
  applyToDocument(current);
  for (const listener of listeners) listener();
};

export const initPreferences = () => {
  applyToDocument(current);
};
