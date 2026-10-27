/** The treemap categories of the brief, §4, in legend order. */
export const CATEGORIES = [
  'documentsMedia',
  'devTools',
  'codeProjects',
  'sdks',
  'aiModels',
  'packageCaches',
  'system',
  'other',
] as const;

/** The cleanup candidates of the brief, §3: the whitelist first, then the ones to review one by one. */
export const CANDIDATE_KINDS = [
  'userTemp',
  'windowsTemp',
  'pnpmStore',
  'npmCache',
  'dockerDangling',
  'dockerBuildCache',
  'nextCache',
  'turbo',
  'shaderCache',
  'staleNodeModules',
  'dockerUnusedImages',
  'vhdx',
] as const;

/** How often the screen asks how far a running scan got. */
export const SCAN_POLL_MS = 1000;
