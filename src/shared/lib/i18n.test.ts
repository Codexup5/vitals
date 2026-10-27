import { describe, expect, it } from 'vitest';

import { detectLocale, isLocale } from './i18n';

describe('detectLocale', () => {
  it.each([
    ['ru-RU', 'ru'],
    ['ru', 'ru'],
    ['RU-ru', 'ru'],
    ['en-US', 'en'],
    ['de-DE', 'en'],
    ['', 'en'],
  ])('%s → %s', (language, expected) => {
    expect(detectLocale(language)).toBe(expected);
  });
});

describe('isLocale', () => {
  it.each([
    ['ru', true],
    ['en', true],
    ['de', false],
    [undefined, false],
  ])('%s → %s', (value, expected) => {
    expect(isLocale(value)).toBe(expected);
  });
});
