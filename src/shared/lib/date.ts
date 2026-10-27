import { COMMON_TEXTS } from '../config/texts';
import { INTL_LOCALES, type Locale } from './i18n';

const formats = (options: Intl.DateTimeFormatOptions): Readonly<Record<Locale, Intl.DateTimeFormat>> => ({
  ru: new Intl.DateTimeFormat(INTL_LOCALES.ru, options),
  en: new Intl.DateTimeFormat(INTL_LOCALES.en, options),
});

const DATE_FORMATS = formats({ dateStyle: 'long' });
const DAY_FORMATS = formats({ day: '2-digit', month: '2-digit' });
const MONTH_FORMATS = formats({ month: 'short', year: 'numeric' });
const DATE_TIME_FORMATS = formats({
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
});

export const parseIsoDateTime = (iso: string) => {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) throw new RangeError(`Invalid ISO date: ${iso}`);
  return date;
};

/** The Rust side sends times as Unix seconds. */
export const fromUnixSeconds = (seconds: number) => new Date(seconds * 1000);

/** A calendar day `2026-10-27` as local midnight, so the label never shifts by the time zone. */
const fromCalendarDay = (day: string) => {
  const [year = 0, month = 1, date = 1] = day.split('-').map(Number);
  return new Date(year, month - 1, date);
};

export const formatDate = (date: Date | null, locale: Locale) =>
  date === null ? COMMON_TEXTS[locale].missingValue : DATE_FORMATS[locale].format(date);

export const formatDateTime = (date: Date, locale: Locale) => DATE_TIME_FORMATS[locale].format(date);

/** `27.10` from `2026-10-27` */
export const formatCalendarDay = (day: string, locale: Locale) => DAY_FORMATS[locale].format(fromCalendarDay(day));

/** `окт. 2026` from `2026-10` */
export const formatCalendarMonth = (month: string, locale: Locale) =>
  MONTH_FORMATS[locale].format(fromCalendarDay(`${month}-01`));
