import type { Localized } from '@/shared/lib/i18n';

const ru = {
  title: 'Раздел не найден',
  home: 'К первому модулю',
} as const;

export const NOT_FOUND_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    title: 'Section not found',
    home: 'To the first module',
  },
};
