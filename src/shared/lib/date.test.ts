import { describe, expect, it } from 'vitest';

import { APP_LOCALE } from '../config/locale';
import { COMMON_TEXTS } from '../config/texts';
import { formatDate, parseIsoDateTime } from './date';

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
  it('gives the missing-value text for null', () => {
    expect(formatDate(null)).toBe(COMMON_TEXTS.missingValue);
  });

  it('gives a long date in the app locale otherwise', () => {
    const date = parseIsoDateTime('2024-03-05T12:00:00Z');
    const expected = new Intl.DateTimeFormat(APP_LOCALE, { dateStyle: 'long' }).format(date);

    expect(formatDate(date)).toBe(expected);
  });
});
