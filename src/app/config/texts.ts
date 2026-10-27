import type { Localized } from '@/shared/lib/i18n';

const ru = {
  brand: 'Vitals',
  fatalTitle: 'Панель упала',
  reload: 'Перезагрузить окно',
  modules: 'Модули',
  preferences: 'Настройки окна',
  switchLocale: 'English',
  switchLocaleLabel: 'Switch the interface to English',
  themeDark: 'Тёмная тема',
  themeLight: 'Светлая тема',
} as const;

export const APP_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    brand: 'Vitals',
    fatalTitle: 'The panel crashed',
    reload: 'Reload the window',
    modules: 'Modules',
    preferences: 'Window settings',
    switchLocale: 'Русский',
    switchLocaleLabel: 'Переключить интерфейс на русский',
    themeDark: 'Dark theme',
    themeLight: 'Light theme',
  },
};
