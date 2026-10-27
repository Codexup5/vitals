import type { Minidump } from '@/entities/health';
import { getErrorMessage } from '@/shared/api/error-message';
import { formatDateTime } from '@/shared/lib/date';
import { INTL_LOCALES, type Locale } from '@/shared/lib/i18n';

import { ERRORS_TEXTS } from '../config/texts';

const BYTES_IN_MEGABYTE = 1024 * 1024;

interface MinidumpRow {
  name: string;
  timeLabel: string;
  code: string;
  sizeLabel: string;
}

export type MinidumpView =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'empty'; message: string }
  | { status: 'ready'; rows: readonly MinidumpRow[] };

interface MinidumpState {
  minidumps: readonly Minidump[] | undefined;
  error: unknown;
  locale: Locale;
}

export const toMinidumpView = ({ minidumps, error, locale }: MinidumpState): MinidumpView => {
  if (!minidumps) return error ? { status: 'error', message: getErrorMessage(error, locale) } : { status: 'loading' };
  const texts = ERRORS_TEXTS[locale].dumps;
  if (minidumps.length === 0) return { status: 'empty', message: texts.empty };
  const megabytes = new Intl.NumberFormat(INTL_LOCALES[locale], { maximumFractionDigits: 1 });
  return {
    status: 'ready',
    rows: minidumps.map((dump) => ({
      name: dump.name,
      timeLabel: formatDateTime(dump.time, locale),
      code: dump.code ?? texts.unknownCode,
      sizeLabel: texts.megabytes(megabytes.format(dump.sizeBytes / BYTES_IN_MEGABYTE)),
    })),
  };
};
