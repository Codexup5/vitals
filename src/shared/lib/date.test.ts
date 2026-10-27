import { describe, expect, it } from 'vitest';

import { COMMON_TEXTS } from '../config/texts';
import { formatCalendarDay, formatCalendarMonth, formatDate, fromUnixSeconds, parseIsoDateTime } from './date';
import { INTL_LOCALES } from './i18n';

describe('parseIsoDateTime', () => {
  it('takes a date with an offset', () => {
    expect(parseIsoDateTime('2024-03-05T10:00:00+03:00').toISOString()).toBe('2024-03-05T07:00:00.000Z');
  });

  it('takes a Z date', () => {
    expect(parseIsoDateTime('2024-03-05T10:00:00Z').toISOString()).toBe('2024-03-05T10:00:00.000Z');
  });

  it.each(['', 'garbage', '2024-13-45T99:00:00Z'])('throws RangeError on "%s"', (iso) => {
    expect(() => parseIsoDateTime(iso)).toThrow(RangeError);
  });
});

describe('formatDate', () => {
  it.each(['ru', 'en'] as const)('gives the missing-value text for null in %s', (locale) => {
    expect(formatDate(null, locale)).toBe(COMMON_TEXTS[locale].missingValue);
  });

  it.each(['ru', 'en'] as const)('gives a long date in %s', (locale) => {
    const date = parseIsoDateTime('2024-03-05T12:00:00Z');
    const expected = new Intl.DateTimeFormat(INTL_LOCALES[locale], { dateStyle: 'long' }).format(date);

    expect(formatDate(date, locale)).toBe(expected);
  });
});

describe('calendar labels', () => {
  it('keeps the calendar day whatever the time zone', () => {
    expect(formatCalendarDay('2026-10-27', 'ru')).toBe('27.10');
    expect(formatCalendarDay('2026-01-01', 'en')).toBe('01/01');
  });

  it('names the month', () => {
    expect(formatCalendarMonth('2026-10', 'en')).toBe('Oct 2026');
  });

  it('reads Unix seconds', () => {
    expect(fromUnixSeconds(1_793_097_600).toISOString()).toBe('2026-10-27T10:40:00.000Z');
  });
});
