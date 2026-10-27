import { useSyncExternalStore } from 'react';

import type { Locale } from '../lib/i18n';
import { getPreferences, subscribePreferences, updatePreferences } from '../lib/preferences';

export const usePreferences = () => {
  const preferences = useSyncExternalStore(subscribePreferences, getPreferences);
  return { ...preferences, updatePreferences };
};

export const useTexts = <C extends Readonly<Record<Locale, unknown>>>(catalog: C): C[Locale] =>
  catalog[usePreferences().locale];
