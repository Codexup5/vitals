import { APP_LOCALE } from '../config/locale';

export type PluralForms = readonly [one: string, few: string, many: string];

const pluralRules = new Intl.PluralRules(APP_LOCALE);

export const pluralize = (count: number, [one, few, many]: PluralForms) => {
  const category = pluralRules.select(count);
  if (category === 'one') return one;
  if (category === 'few') return few;
  return many;
};
