import type { DiskHealth, DisksOverview, ScanStatus } from '@/entities/disk';
import { getErrorMessage } from '@/shared/api/error-message';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { formatDateTime } from '@/shared/lib/date';
import { formatBytes } from '@/shared/lib/format-bytes';
import { INTL_LOCALES, type Locale } from '@/shared/lib/i18n';
import type { TagTone } from '@/shared/ui/tag/Tag';

import { DISKS_TEXTS } from '../config/texts';
import { sumOf, toRecommendationGroups, type RecommendationGroup } from './cleanup-view';

interface Kpi {
  key: string;
  label: string;
  value: string;
  hint: string | null;
}

interface VolumeRow {
  letter: string;
  name: string;
  usedLabel: string;
  freeLabel: string;
  usedPercent: number;
}

const HEALTH_TONES: Record<DiskHealth, TagTone> = {
  healthy: 'ok',
  warning: 'warn',
  unhealthy: 'danger',
  unknown: 'accent',
};

interface DriveRow {
  key: string;
  name: string;
  healthTone: TagTone;
  healthLabel: string;
  temperature: string;
  wear: string;
  hours: string;
}

export type DisksView =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | {
      status: 'ready';
      kpis: readonly Kpi[];
      volumes: readonly VolumeRow[];
      drives: readonly DriveRow[];
      scanLabel: string;
      hasScan: boolean;
      recommendations: readonly RecommendationGroup[];
    };

interface DisksState {
  overview: DisksOverview | undefined;
  error: unknown;
  scanStatus: ScanStatus | undefined;
  systemDrive: string;
  locale: Locale;
}

const scanLabelOf = ({ overview, scanStatus, locale }: Omit<DisksState, 'error' | 'systemDrive'>) => {
  const texts = DISKS_TEXTS[locale].scan;
  const count = new Intl.NumberFormat(INTL_LOCALES[locale]);
  if (scanStatus?.isRunning)
    return texts.running(count.format(scanStatus.files), formatBytes(scanStatus.bytes, locale));
  if (!overview?.scan) return texts.never;
  const last = texts.last(formatDateTime(overview.scan.finishedAt, locale));
  return overview.scan.links > 0 ? `${last} · ${texts.links(count.format(overview.scan.links))}` : last;
};

export const toDisksView = ({ overview, error, scanStatus, systemDrive, locale }: DisksState): DisksView => {
  if (!overview) return error ? { status: 'error', message: getErrorMessage(error, locale) } : { status: 'loading' };
  const texts = DISKS_TEXTS[locale];
  const missing = COMMON_TEXTS[locale].missingValue;
  const count = new Intl.NumberFormat(INTL_LOCALES[locale]);
  const system = overview.volumes.find((volume) => volume.letter === systemDrive);
  const candidates = overview.scan?.candidates ?? [];
  const safe = sumOf(candidates, 'safe');
  const review = sumOf(candidates, 'review');
  const scanned = overview.scan?.totalBytes;
  const notScanned =
    system && scanned !== undefined ? Math.max(0, system.totalBytes - system.freeBytes - scanned) : null;
  const bytes = (value: number | null | undefined) =>
    value === null || value === undefined ? missing : formatBytes(value, locale);

  return {
    status: 'ready',
    kpis: [
      { key: 'free', label: texts.kpis.freeNow(`${systemDrive}:`), value: bytes(system?.freeBytes), hint: null },
      {
        key: 'after',
        label: texts.kpis.freeAfter,
        value: system && overview.scan ? bytes(system.freeBytes + safe) : missing,
        hint: null,
      },
      { key: 'safe', label: texts.kpis.safe, value: overview.scan ? bytes(safe) : missing, hint: null },
      { key: 'review', label: texts.kpis.review, value: overview.scan ? bytes(review) : missing, hint: null },
      { key: 'notScanned', label: texts.kpis.notScanned, value: bytes(notScanned), hint: texts.kpis.notScannedHint },
    ],
    volumes: overview.volumes.map((volume) => {
      const used = volume.totalBytes - volume.freeBytes;
      return {
        letter: volume.letter,
        name: volume.label ? `${volume.letter}: ${volume.label}` : `${volume.letter}:`,
        usedLabel: texts.volumes.used(formatBytes(used, locale), formatBytes(volume.totalBytes, locale)),
        freeLabel: texts.volumes.free(formatBytes(volume.freeBytes, locale)),
        usedPercent: volume.totalBytes > 0 ? Math.round((used / volume.totalBytes) * 100) : 0,
      };
    }),
    drives: overview.physicalDisks.map((drive, index) => ({
      key: `${drive.name}-${String(index)}`,
      name: `${drive.name} · ${drive.media} · ${formatBytes(drive.sizeBytes, locale)}`,
      healthTone: HEALTH_TONES[drive.health],
      healthLabel: texts.drives[drive.health],
      temperature: drive.temperatureC === null ? missing : texts.drives.celsius(drive.temperatureC),
      wear: drive.wearPercent === null ? missing : texts.drives.percent(drive.wearPercent),
      hours: drive.powerOnHours === null ? missing : texts.drives.hoursValue(count.format(drive.powerOnHours)),
    })),
    scanLabel: scanLabelOf({ overview, scanStatus, locale }),
    hasScan: overview.scan !== null,
    recommendations: toRecommendationGroups(candidates, locale),
  };
};
