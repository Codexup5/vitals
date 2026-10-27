import { COMMON_TEXTS } from '../config/texts';
import type { Locale } from '../lib/i18n';
import { isIpcError } from './ipc-error';

/** What happened and what to do, by the kind of the failure. */
export const getErrorMessage = (error: unknown, locale: Locale) => {
  const texts = COMMON_TEXTS[locale].errors;
  if (!isIpcError(error)) return texts.unexpected;
  switch (error.kind) {
    case 'storage':
      return texts.storage;
    case 'timeout':
      return texts.timeout;
    case 'unavailable':
      return texts.unavailable;
    case 'schema':
    case 'invalid':
    case 'io':
      return texts.unexpected;
  }
};
