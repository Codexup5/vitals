import { describe, expect, it } from 'vitest';

import { COMMON_TEXTS } from '../config/texts';
import { formatDate, parseIsoDateTime } from './date';
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
