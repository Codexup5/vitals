import { describe, expect, it } from 'vitest';

import { parsePreferences } from './preferences';

describe('parsePreferences', () => {
  it('takes the Windows language and the dark theme when nothing is stored', () => {
    expect(parsePreferences(null, 'ru-RU')).toEqual({ locale: 'ru', theme: 'dark' });
    expect(parsePreferences(null, 'en-GB')).toEqual({ locale: 'en', theme: 'dark' });
  });

  it('keeps stored values', () => {
    expect(parsePreferences('{"locale":"en","theme":"light"}', 'ru-RU')).toEqual({ locale: 'en', theme: 'light' });
  });

  it('replaces an unknown value field by field', () => {
    expect(parsePreferences('{"locale":"de","theme":"light"}', 'ru-RU')).toEqual({ locale: 'ru', theme: 'light' });
  });

  it.each(['not json', '42', 'null', '[]'])('falls back on %s', (stored) => {
    expect(parsePreferences(stored, 'en-US')).toEqual({ locale: 'en', theme: 'dark' });
  });
});
