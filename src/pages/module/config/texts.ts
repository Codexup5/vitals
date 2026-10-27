import type { Localized } from '@/shared/lib/i18n';

const ru = {
  plannedStage: (stage: number) => `Модуль появится на этапе ${stage} плана работ.`,
} as const;

export const MODULE_PAGE_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    plannedStage: (stage: number) => `This module arrives at stage ${stage} of the work plan.`,
  },
};
