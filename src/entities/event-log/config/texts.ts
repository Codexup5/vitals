import type { Localized } from '@/shared/lib/i18n';

const ru = {
  kinds: {
    crash: 'Аварийная перезагрузка',
    bsod: 'Синий экран (BSOD)',
    whea: 'Аппаратная ошибка (WHEA)',
    gpuReset: 'Сброс видеодрайвера',
    appCrash: 'Падение программы',
    nvContainer: 'Падение службы NVIDIA',
    iommu: 'Предупреждение IOMMU',
    error: 'Прочая ошибка System',
  },
} as const;

export const HEALTH_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    kinds: {
      crash: 'Unexpected restart',
      bsod: 'Blue screen (BSOD)',
      whea: 'Hardware error (WHEA)',
      gpuReset: 'GPU driver reset',
      appCrash: 'Program crash',
      nvContainer: 'NVIDIA service crash',
      iommu: 'IOMMU warning',
      error: 'Other System error',
    },
  },
};
