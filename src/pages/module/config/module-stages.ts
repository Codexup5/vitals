import type { ModuleId } from '@/shared/config/modules';

// The stage of the work plan (the brief, §8) that brings each module
export const MODULE_STAGES: Readonly<Record<ModuleId, number>> = {
  errors: 2,
  disks: 3,
  updates: 4,
  runners: 5,
  network: 6,
  startup: 7,
  sensors: 9,
};
