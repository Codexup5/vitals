import type { Localized } from '../lib/i18n';

const ru = {
  actions: {
    retry: 'Повторить',
    previous: 'Назад',
    next: 'Вперёд',
  },
  errors: {
    unexpected: 'Что-то пошло не так. Перезапустите Vitals.',
    notFound: 'Такого раздела нет.',
    screenTitle: 'Не удалось загрузить',
    storage: 'Не удалось прочитать базу Vitals. Повторите через минуту; если не поможет — перезапустите Vitals.',
    timeout: 'Vitals долго не отвечает. Повторите через минуту.',
    unavailable: 'Данные доступны только в окне Vitals, а не в браузере.',
  },
  pagination: {
    label: 'Страницы',
    position: (page: number, pageCount: number) => `Страница ${page} из ${pageCount}`,
  },
  units: {
    gigabytes: 'ГБ',
    megabytes: 'МБ',
  },
  missingValue: '—',
} as const;

export const COMMON_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    actions: {
      retry: 'Retry',
      previous: 'Previous',
      next: 'Next',
    },
    errors: {
      unexpected: 'Something went wrong. Restart Vitals.',
      notFound: 'There is no such section.',
      screenTitle: 'Could not load',
      storage: 'Could not read the Vitals database. Retry in a minute; if that does not help, restart Vitals.',
      timeout: 'Vitals takes too long to answer. Retry in a minute.',
      unavailable: 'The data is available only in the Vitals window, not in a browser.',
    },
    pagination: {
      label: 'Pages',
      position: (page: number, pageCount: number) => `Page ${page} of ${pageCount}`,
    },
    units: {
      gigabytes: 'GB',
      megabytes: 'MB',
    },
    missingValue: '—',
  },
};
