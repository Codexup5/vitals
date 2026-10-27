import type { Localized } from '../lib/i18n';

const ru = {
  errors: 'Ошибки и падения',
  disks: 'Диски и очистка',
  runners: 'Раннеры и Docker',
  sensors: 'Датчики',
  startup: 'Автозагрузка и службы',
  updates: 'Обновления',
  network: 'Сеть',
} as const;

export const MODULE_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    errors: 'Errors and crashes',
    disks: 'Disks and cleanup',
    runners: 'Runners and Docker',
    sensors: 'Sensors',
    startup: 'Startup and services',
    updates: 'Updates',
    network: 'Network',
  },
};
