import { describe, expect, it } from 'vitest';

import { formatBytes } from './format-bytes';

describe('formatBytes', () => {
  it('shows gigabytes with one decimal in the locale', () => {
    expect(formatBytes(222_906_920_960, 'ru')).toBe('207,6 ГБ');
    expect(formatBytes(222_906_920_960, 'en')).toBe('207.6 GB');
  });

  it('falls to megabytes below a gigabyte', () => {
    expect(formatBytes(512 * 1024 * 1024, 'en')).toBe('512.0 MB');
    expect(formatBytes(0, 'ru')).toBe('0,0 МБ');
  });
});
