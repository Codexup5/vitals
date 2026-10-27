import type { EVENT_KINDS, HEALTH_RANGES } from '../config/event-kinds';

export type EventKind = (typeof EVENT_KINDS)[number];

export type HealthRange = (typeof HEALTH_RANGES)[number];

interface DayCounts {
  crashes: number;
  bsod: number;
  gpuReset: number;
  nvContainer: number;
  whea: number;
  iommu: number;
  appCrashes: number;
  errors: number;
}

/** A calendar day `2026-10-27`, or a month `2026-10` in the monthly series */
export interface HealthPeriod extends DayCounts {
  period: string;
  topApps: readonly string[];
}

export interface HealthOverview {
  daysWithoutCrash: number | null;
  systemDriveFreeGb: number | null;
  firstDay: string | null;
  days: readonly HealthPeriod[];
  months: readonly HealthPeriod[];
}

interface HealthEvent {
  id: string;
  time: Date;
  kind: EventKind;
  provider: string;
  eventId: number;
  detail: string | null;
}

export interface HealthEventPage {
  events: readonly HealthEvent[];
  total: number;
  pageCount: number;
}

export interface HealthEventParams {
  kinds: readonly EventKind[];
  /** 1-based, as in the URL */
  page: number;
}

export interface Minidump {
  name: string;
  sizeBytes: number;
  time: Date;
  code: string | null;
}
