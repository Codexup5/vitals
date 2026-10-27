import type { CANDIDATE_KINDS, CATEGORIES } from '../config/disk-kinds';

export type Category = (typeof CATEGORIES)[number];

type CandidateKind = (typeof CANDIDATE_KINDS)[number];

export type CandidateGroup = 'safe' | 'review';

export type DiskHealth = 'healthy' | 'warning' | 'unhealthy' | 'unknown';

interface Volume {
  letter: string;
  label: string;
  totalBytes: number;
  freeBytes: number;
}

interface PhysicalDisk {
  name: string;
  media: string;
  sizeBytes: number;
  health: DiskHealth;
  temperatureC: number | null;
  wearPercent: number | null;
  powerOnHours: number | null;
}

export interface Tile {
  path: string;
  label: string;
  bytes: number;
  category: Category;
  isRest: boolean;
}

export interface Candidate {
  id: string;
  group: CandidateGroup;
  kind: CandidateKind;
  path: string | null;
  bytes: number;
  isUpperBound: boolean;
  lastModified: Date | null;
  needsAdmin: boolean;
}

interface Scan {
  finishedAt: Date;
  home: string;
  totalBytes: number;
  files: number;
  links: number;
  unreadable: number;
  tiles: readonly Tile[];
  candidates: readonly Candidate[];
}

export interface DisksOverview {
  volumes: readonly Volume[];
  physicalDisks: readonly PhysicalDisk[];
  scan: Scan | null;
}

export interface ScanStatus {
  isRunning: boolean;
  files: number;
  bytes: number;
}
