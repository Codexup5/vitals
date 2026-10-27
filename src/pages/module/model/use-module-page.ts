import { getRouteApi } from '@tanstack/react-router';

import { MODULE_TEXTS } from '@/shared/config/modules-texts';
import { useTexts } from '@/shared/model/use-preferences';

import { MODULE_STAGES } from '../config/module-stages';
import { MODULE_PAGE_TEXTS } from '../config/texts';

const moduleRoute = getRouteApi('/$moduleId');

export const useModulePage = () => {
  const { moduleId } = moduleRoute.useParams();
  const moduleTexts = useTexts(MODULE_TEXTS);
  const pageTexts = useTexts(MODULE_PAGE_TEXTS);

  return {
    eyebrow: pageTexts.eyebrow,
    title: moduleTexts[moduleId],
    note: pageTexts.plannedStage(MODULE_STAGES[moduleId]),
  };
};
