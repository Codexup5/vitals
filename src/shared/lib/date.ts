import { COMMON_TEXTS } from '../config/texts';
import { INTL_LOCALES, type Locale } from './i18n';

const DATE_FORMATS: Readonly<Record<Locale, Intl.DateTimeFormat>> = {
  ru: new Intl.DateTimeFormat(INTL_LOCALES.ru, { dateStyle: 'long' }),
  en: new Intl.DateTimeFormat(INTL_LOCALES.en, { dateStyle: 'long' }),
};

export const parseIsoDateTime = (iso: string) => {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) throw new RangeError(`Invalid ISO date: ${iso}`);
  return date;
};

export const formatDate = (date: Date | null, locale: Locale) =>
  date === null ? COMMON_TEXTS[locale].missingValue : DATE_FORMATS[locale].format(date);
