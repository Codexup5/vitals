const LOCALES = ['ru', 'en'] as const;

export type Locale = (typeof LOCALES)[number];

export const INTL_LOCALES: Readonly<Record<Locale, string>> = { ru: 'ru-RU', en: 'en-GB' };

type Widen<T> = T extends string
  ? string
  : T extends (...args: infer A) => infer R
    ? (...args: A) => Widen<R>
    : { readonly [K in keyof T]: Widen<T[K]> };

// The ru catalog is the shape: every locale holds exactly its keys, so a missing or extra key fails tsc
export type Localized<T> = Readonly<Record<Locale, Widen<T>>>;

export const isLocale = (value: unknown): value is Locale => LOCALES.some((locale) => locale === value);

export const detectLocale = (language: string): Locale => (language.toLowerCase().startsWith('ru') ? 'ru' : 'en');
