import path from 'node:path';
import stylelint from 'stylelint';
import { describe, expect, it } from 'vitest';

import { classNameRule } from './class-case.mjs';

const PACK_ROOT = path.resolve(import.meta.dirname, '..');
const CONFIG_FILE = path.join(PACK_ROOT, 'stylelint.config.mjs');

const MODULE = 'src/pages/probe/ui/Probe.module.scss';

const HEX = 'color-no-hex';
const NAMED = 'color-named';
const FUNCTIONS = 'function-disallowed-list';
const VALUES = 'declaration-property-value-disallowed-list';
const IMPORTANT = 'declaration-no-important';
const CLASS_PATTERN = 'selector-class-pattern';
const LINE_UNIT = 'declaration-property-unit-allowed-list';

const rule = (declaration, selector = '.probe') => `${selector} {\n  ${declaration}\n}\n`;

// [case, file, code, rule] — the rule must report (CATCHES) or must not report (PASSES).
const CATCHES = [
  ['a hex color', MODULE, rule('color: #fff;'), HEX],
  ['a named color', MODULE, rule('color: red;'), NAMED],
  ['rgb()', MODULE, rule('color: rgb(0 0 0);'), FUNCTIONS],
  ['padding in px', MODULE, rule('padding: 16px;'), VALUES],
  ['margin in rem', MODULE, rule('margin: 1rem;'), VALUES],
  ['!important in a module', MODULE, rule('display: block !important;'), IMPORTANT],
  ['a camelCase class', MODULE, rule('display: block;', '.cardHeader'), CLASS_PATTERN],
  ['line-height in px', MODULE, rule('line-height: 24px;'), LINE_UNIT],
  ['line-height in em', MODULE, rule('line-height: 1.5em;'), LINE_UNIT],
  ['line-height in %', MODULE, rule('line-height: 150%;'), LINE_UNIT],
];

const PASSES = [
  ['a color token', MODULE, rule('color: var(--color-text);'), HEX],
  ['a color token', MODULE, rule('color: var(--color-text);'), NAMED],
  ['a color token', MODULE, rule('color: var(--color-text);'), FUNCTIONS],
  ['an optical padding of 2px', MODULE, rule('padding: 2px;'), VALUES],
  ['a spacing token', MODULE, rule('padding: var(--space-4);'), VALUES],
  ['!important in global.scss', 'src/app/styles/global.scss', rule('display: block !important;'), IMPORTANT],
  ['a hex in tokens.scss', 'src/shared/styles/tokens.scss', rule('color: #fff;'), HEX],
  ['a kebab-case class', MODULE, rule('display: block;', '.card-header'), CLASS_PATTERN],
  ['a unitless line-height', MODULE, rule('line-height: 1.5;'), LINE_UNIT],
  ['a line-height token', MODULE, rule('line-height: var(--line-height-body);'), LINE_UNIT],
  ['line-height normal', MODULE, rule('line-height: normal;'), LINE_UNIT],
];

const lintRules = async (file, code) => {
  const { results } = await stylelint.lint({
    code,
    codeFilename: path.join(PACK_ROOT, file),
    configFile: CONFIG_FILE,
  });
  return (results[0]?.warnings ?? []).map((warning) => warning.rule);
};

describe('stylelint config: token rules', () => {
  it.each(CATCHES)('catches %s', async (_name, file, code, ruleName) => {
    expect(await lintRules(file, code)).toContain(ruleName);
  });

  it.each(PASSES)('does not catch %s', async (_name, file, code, ruleName) => {
    expect(await lintRules(file, code)).not.toContain(ruleName);
  });
});

const REASON = 'a probe reason';
const ZERO_UNIT = 'length-zero-no-unit';

const disableNext = (ruleName, reason = REASON) =>
  `/* stylelint-disable-next-line ${ruleName}${reason ? ` -- ${reason}` : ''} */\n`;

const inRule = (declaration, comment = '') => `.probe {\n  ${comment}${declaration}\n}\n`;

// [rule, code] — a disable of a token rule with a reason is still reported.
const TOKEN_DISABLES = [
  [HEX, inRule('color: #fff;', disableNext(HEX))],
  [NAMED, inRule('color: red;', disableNext(NAMED))],
  [FUNCTIONS, inRule('color: rgb(0 0 0);', disableNext(FUNCTIONS))],
  [VALUES, inRule('padding: 16px;', disableNext(VALUES))],
  [IMPORTANT, inRule('display: block !important;', disableNext(IMPORTANT))],
  [CLASS_PATTERN, `${disableNext(CLASS_PATTERN)}.cardHeader {\n  display: block;\n}\n`],
];

// [case, code, report rule, text of the report] — a problem of the disable comment itself.
const DISABLE_PROBLEMS = [
  [
    'a disable without a reason',
    inRule('margin: 0px;', disableNext(ZERO_UNIT, '')),
    '--report-descriptionless-disables',
    `Disable for "${ZERO_UNIT}" is missing a description`,
  ],
  [
    'a needless disable',
    inRule('margin: 1px;', disableNext(ZERO_UNIT)),
    '--report-needless-disables',
    `Needless disable for "${ZERO_UNIT}"`,
  ],
  [
    'a disable of a rule the config lacks',
    inRule('margin: 1px;', disableNext('selector-max-id')),
    '--report-invalid-scope-disables',
    'Rule "selector-max-id" isn\'t enabled',
  ],
  [
    'a disable without rules',
    `/* stylelint-disable */\n${inRule('margin: 1px;')}`,
    '--report-unscoped-disables',
    'Configuration comment must be scoped',
  ],
];

const lintWarnings = async (file, code) => {
  const { results } = await stylelint.lint({
    code,
    codeFilename: path.join(PACK_ROOT, file),
    configFile: CONFIG_FILE,
  });
  return results[0]?.warnings ?? [];
};

describe('stylelint config: disable comments', () => {
  it.each(TOKEN_DISABLES)('catches a disable of %s with a reason', async (ruleName, code) => {
    expect(await lintWarnings(MODULE, code)).toContainEqual(
      expect.objectContaining({
        rule: 'reportDisables',
        text: expect.stringContaining(`Rule "${ruleName}" may not be disabled`),
      }),
    );
  });

  it.each(DISABLE_PROBLEMS)('catches %s', async (_name, code, reportRule, text) => {
    expect(await lintWarnings(MODULE, code)).toContainEqual(
      expect.objectContaining({ rule: reportRule, text: expect.stringContaining(text) }),
    );
  });

  it('does not catch a disable of length-zero-no-unit with a reason over margin: 0px', async () => {
    const code = inRule('margin: 0px;', disableNext(ZERO_UNIT));
    expect(await lintWarnings(MODULE, code)).toEqual([]);
  });
});

const PALETTE = 'src/shared/styles/_palette.scss';
const TOKENS = 'src/shared/styles/tokens.scss';

// [case, file, code, rule] — the token files lift the hex rule, not the function list.
const TOKEN_FILE_CATCHES = [
  ['a hex in a module', MODULE, rule('color: #fff;'), HEX],
  ['rgb(0 0 0) in a module', MODULE, rule('color: rgb(0 0 0);'), FUNCTIONS],
  ['rgb(0 0 0) in _palette.scss', PALETTE, rule('color: rgb(0 0 0);'), FUNCTIONS],
];

const TOKEN_FILE_PASSES = [
  ['a hex in _palette.scss', PALETTE, rule('color: #fff;'), HEX],
  ['a hex in tokens.scss', TOKENS, rule('color: #fff;'), HEX],
];

const lintWithClassCase = async (classCase, code) => {
  const { default: baseConfig } = await import(CONFIG_FILE);
  const { results } = await stylelint.lint({
    code,
    codeFilename: path.join(PACK_ROOT, MODULE),
    config: {
      ...baseConfig,
      rules: { ...baseConfig.rules, [CLASS_PATTERN]: classNameRule(classCase) },
    },
    configBasedir: PACK_ROOT,
  });
  return (results[0]?.warnings ?? []).map((warning) => warning.rule);
};

describe('stylelint config: token files', () => {
  it.each(TOKEN_FILE_CATCHES)('catches %s', async (_name, file, code, ruleName) => {
    expect(await lintRules(file, code)).toContain(ruleName);
  });

  it.each(TOKEN_FILE_PASSES)('does not catch %s', async (_name, file, code, ruleName) => {
    expect(await lintRules(file, code)).not.toContain(ruleName);
  });
});

describe('stylelint config: class case', () => {
  it('catches a kebab-case class with the camel option', async () => {
    const code = rule('display: block;', '.card-header');
    expect(await lintWithClassCase('camel', code)).toContain(CLASS_PATTERN);
  });

  it('does not catch a camelCase class with the camel option', async () => {
    const code = rule('display: block;', '.cardHeader');
    expect(await lintWithClassCase('camel', code)).not.toContain(CLASS_PATTERN);
  });

  it('still catches a camelCase class with the default config', async () => {
    const code = rule('display: block;', '.cardHeader');
    expect(await lintRules(MODULE, code)).toContain(CLASS_PATTERN);
  });

  it('throws for an unknown case, naming kebab and camel', () => {
    expect(() => classNameRule('snake')).toThrow(/kebab.*camel|camel.*kebab/);
  });
});
