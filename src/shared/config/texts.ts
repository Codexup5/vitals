import type { Localized } from '../lib/i18n';

const ru = {
  actions: {
    retry: 'Повторить',
  },
  errors: {
    unexpected: 'Что-то пошло не так. Перезапустите Vitals.',
    notFound: 'Такого раздела нет.',
    screenTitle: 'Не удалось загрузить',
  },
  missingValue: '—',
} as const;

export const COMMON_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    actions: {
      retry: 'Retry',
    },
    errors: {
      unexpected: 'Something went wrong. Restart Vitals.',
      notFound: 'There is no such section.',
      screenTitle: 'Could not load',
    },
    missingValue: '—',
  },
};
