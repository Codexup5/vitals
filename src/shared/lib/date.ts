import { APP_LOCALE } from '../config/locale';
import { COMMON_TEXTS } from '../config/texts';

const dateFormat = new Intl.DateTimeFormat(APP_LOCALE, { day: 'numeric', month: 'long', year: 'numeric' });

export const parseIsoDateTime = (iso: string) => {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) throw new RangeError(`Invalid ISO date: ${iso}`);
  return date;
};

export const formatDate = (date: Date | null) => (date === null ? COMMON_TEXTS.missingValue : dateFormat.format(date));
