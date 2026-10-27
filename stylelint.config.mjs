import fs from 'node:fs';
import path from 'node:path';

import { classNameRule } from './tooling/class-case.mjs';

// 'kebab' or 'camel'; a project that sets camel records it in docs/deviations.md.
const CLASS_CASE = 'kebab';

// tokens.scss is built from the DTCG data and judged there once the token config exists; a hand-written one is linted
const GENERATED_TOKENS = fs.existsSync(path.join(import.meta.dirname, 'src/shared/styles/tokens/config.json'))
  ? ['src/shared/styles/tokens.scss']
  : [];

const SPACING_PROPERTIES = [
  '/^margin/',
  '/^padding/',
  'gap',
  'row-gap',
  'column-gap',
  'font-size',
  '/^border-radius/',
  '/^inset/',
];

const RAW_SIZE = /\b(?:[4-9]|\d{2,})(?:\.\d+)?px\b|\d(?:\.\d+)?r?em\b/;

export default {
  extends: ['stylelint-config-standard-scss'],
  reportDescriptionlessDisables: true,
  reportNeedlessDisables: true,
  reportInvalidScopeDisables: true,
  reportUnscopedDisables: true,
  rules: {
    'selector-class-pattern': classNameRule(CLASS_CASE),
    'declaration-property-value-disallowed-list': [
      Object.fromEntries(SPACING_PROPERTIES.map((property) => [property, [RAW_SIZE]])),
      {
        message:
          'Spacing, font size and radius are tokens only, `var(--…)`; 1–3 px is allowed as an optical correction.',
        reportDisables: true,
      },
    ],
    'color-no-hex': [
      true,
      { message: 'Color is a token only, `var(--color-…)` from `shared/styles/tokens.scss`.', reportDisables: true },
    ],
    'color-named': ['never', { message: 'Color is a token only, `var(--color-…)`.', reportDisables: true }],
    'function-disallowed-list': [
      ['rgb', 'rgba', 'hsl', 'hsla'],
      { message: 'Color is a token only, `var(--color-…)`.', reportDisables: true },
    ],
    'declaration-property-unit-allowed-list': [
      { 'line-height': [] },
      { message: 'Line height is unitless: a number or a token (design/lh-unitless).', reportDisables: true },
    ],
    'declaration-no-important': [true, { reportDisables: true }],
  },
  overrides: [
    {
      files: ['src/shared/styles/*.scss', 'tooling/design/fixtures/*/src/shared/styles/*.scss'],
      rules: { 'color-no-hex': null, 'declaration-property-value-disallowed-list': null },
    },
    {
      files: ['src/app/styles/global.scss'],
      rules: { 'declaration-no-important': null },
    },
  ],
  ignoreFiles: ['dist/**', 'coverage/**', ...GENERATED_TOKENS],
};
