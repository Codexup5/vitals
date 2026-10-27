import { COMMON_TEXTS } from '../config/texts';
import { INTL_LOCALES, type Locale } from './i18n';

const GIGABYTE = 1024 ** 3;
const MEGABYTE = 1024 ** 2;

const FORMATS: Readonly<Record<Locale, Intl.NumberFormat>> = {
  ru: new Intl.NumberFormat(INTL_LOCALES.ru, { maximumFractionDigits: 1, minimumFractionDigits: 1 }),
  en: new Intl.NumberFormat(INTL_LOCALES.en, { maximumFractionDigits: 1, minimumFractionDigits: 1 }),
};

/** `207,6 ГБ` / `207.6 GB`; below a gigabyte in megabytes, as Explorer shows it (binary units). */
export const formatBytes = (bytes: number, locale: Locale) =>
  bytes >= GIGABYTE
    ? `${FORMATS[locale].format(bytes / GIGABYTE)} ${COMMON_TEXTS[locale].units.gigabytes}`
    : `${FORMATS[locale].format(bytes / MEGABYTE)} ${COMMON_TEXTS[locale].units.megabytes}`;
