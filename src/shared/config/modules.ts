export const MODULE_IDS = ['errors', 'disks', 'runners', 'sensors', 'startup', 'updates', 'network'] as const;

export type ModuleId = (typeof MODULE_IDS)[number];

export const isModuleId = (value: string): value is ModuleId => MODULE_IDS.some((id) => id === value);

export const DEFAULT_MODULE: ModuleId = 'errors';
