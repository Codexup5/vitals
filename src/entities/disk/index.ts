export { CATEGORIES } from './config/disk-kinds';
export { DISK_TEXTS } from './config/texts';
export type { Candidate, CandidateGroup, Category, DiskHealth, DisksOverview, ScanStatus, Tile } from './model/disk';
export { disksOverviewQuery, scanStatusQuery } from './model/disk-queries';
export { useRevealPath, useScanFinished, useStartScan } from './model/disk-mutations';
