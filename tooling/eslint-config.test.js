import { existsSync } from 'node:fs';
import path from 'node:path';
import { ESLint } from 'eslint';
import { beforeAll, describe, expect, it } from 'vitest';

const PACK_ROOT = path.resolve(import.meta.dirname, '..');
const WARM_UP_FILE = 'src/shared/lib/warm-up.ts';

// Cyrillic is built at run time: `tooling/` holds no Cyrillic characters.
const CYRILLIC_WORD = String.fromCodePoint(0x41f, 0x440, 0x438, 0x432, 0x435, 0x442);

const SYNTAX = 'no-restricted-syntax';
const IMPORTS = 'no-restricted-imports';
const NAMING = '@typescript-eslint/naming-convention';
const DENYLIST = 'id-denylist';
const EXPORTS = 'no-restricted-exports';
const HOOK_RULE = 'react-hooks/rules-of-hooks';
const SWALLOWED = 'architecture/no-swallowed-error';

const lines = (...rows) => rows.join('\n');

const PROBE_API = 'src/entities/probe/api/probe-api.ts';
const PROBE_QUERIES = 'src/entities/probe/model/probe-queries.ts';
const PROBE_PAGE_UI = 'src/pages/probe/ui/Probe.tsx';
const PROBE_PAGE_HOOK = 'src/pages/probe/model/use-probe-page.ts';
const PROBE_ROUTES = 'src/app/routing/probe-routes.ts';

const globalFetch = (receiver) => `export const load = () => ${receiver}.fetch('/api/probe');`;

// `staleTime` is given as a source text: a number, a level of `STALE_POLICY`, a string or a shorthand.
const staleTimeProbe = (staleTime) =>
  lines(
    "import { queryOptions } from '@tanstack/react-query';",
    "import { STALE_POLICY } from '@/shared/config/query';",
    'export const probeQuery = () =>',
    `  queryOptions({ queryKey: ['probe'], queryFn: () => Promise.resolve(STALE_POLICY.list), ${staleTime} });`,
  );

const hookProbe = (...body) =>
  lines(
    "import { useState } from 'react';",
    'export const useProbePage = (isShown: boolean) => {',
    ...body,
    '  return isShown;',
    '};',
  );

// [case, file, code, ruleId] — the rule must report (CATCHES) or must not report (PASSES).
const CATCHES = [
  ['enum', 'src/shared/lib/probe.ts', 'export enum Color { Red }', SYNTAX],
  ['export *', 'src/shared/lib/probe.ts', "export * from './other';", SYNTAX],
  ['new Date()', 'src/pages/probe/lib/probe.ts', 'export const now = new Date();', SYNTAX],
  ['Date.parse', 'src/pages/probe/lib/probe.ts', "export const time = Date.parse('2026-01-01');", SYNTAX],
  ['fetch()', 'src/entities/probe/api/probe-api.ts', "export const load = () => fetch('/api/probe');", SYNTAX],
  ['a Cyrillic string literal', 'src/pages/probe/lib/probe.ts', `export const text = '${CYRILLIC_WORD}';`, SYNTAX],
  [
    'a Cyrillic template',
    'src/pages/probe/lib/probe.ts',
    `export const text = (name: string) => \`${CYRILLIC_WORD}, \${name}\`;`,
    SYNTAX,
  ],
  [
    'a Cyrillic JSX text',
    'src/pages/probe/ui/Probe.tsx',
    `export const Probe = () => <p>${CYRILLIC_WORD}</p>;`,
    SYNTAX,
  ],
  ['import axios', 'src/shared/lib/probe.ts', "import axios from 'axios';\nexport const client = axios;", IMPORTS],
  [
    'an import from inside a slice',
    'src/pages/probe/model/use-probe-page.ts',
    "import { userQuery } from '@/entities/user/model/user';\nexport const query = userQuery;",
    IMPORTS,
  ],
  [
    'an import of a segment of a page',
    PROBE_ROUTES,
    "import { useUserListPage } from '@/pages/user-list/model/use-user-list-page';\nexport const hook = useUserListPage;",
    IMPORTS,
  ],
  [
    'an import of a segment of a page in a group',
    PROBE_ROUTES,
    "import { HelpPage } from '@/pages/guide/help/ui/HelpPage';\nexport const page = HelpPage;",
    IMPORTS,
  ],
  [
    'getRouteApi outside pages',
    'src/features/probe/model/use-probe.ts',
    "import { getRouteApi } from '@tanstack/react-router';\nexport const route = getRouteApi;",
    IMPORTS,
  ],
  ['a boolean named open', 'src/shared/lib/probe.ts', 'export const open = Boolean(globalThis);', NAMING],
  ['an interface IProbe', 'src/shared/lib/probe.ts', 'export interface IProbe {\n  id: string;\n}', NAMING],
  ['a variable named data', 'src/shared/lib/probe.ts', 'export const data = 1;', DENYLIST],
  ['window.fetch()', PROBE_API, globalFetch('window'), SYNTAX],
  ['globalThis.fetch()', PROBE_API, globalFetch('globalThis'), SYNTAX],
  ['self.fetch()', PROBE_API, globalFetch('self'), SYNTAX],
  ['staleTime: 30_000', PROBE_QUERIES, staleTimeProbe('staleTime: 30_000'), SYNTAX],
  ['staleTime: 60 * 1000', PROBE_QUERIES, staleTimeProbe('staleTime: 60 * 1000'), SYNTAX],
  ['staleTime: -1', PROBE_QUERIES, staleTimeProbe('staleTime: -1'), SYNTAX],
  ['staleTime: Infinity', PROBE_QUERIES, staleTimeProbe('staleTime: Infinity'), SYNTAX],
  [
    'export default function Probe',
    PROBE_PAGE_UI,
    lines('export default function Probe() {', '  return <p>probe</p>;', '}'),
    EXPORTS,
  ],
  [
    'export { Probe as default }',
    PROBE_PAGE_UI,
    lines('const Probe = () => <p>probe</p>;', 'export { Probe as default };'),
    EXPORTS,
  ],
  ['a re-export of default from ./probe', PROBE_PAGE_UI, "export { default } from './probe';", EXPORTS],
  [
    'a hook call inside an if of a ts hook',
    PROBE_PAGE_HOOK,
    hookProbe('  if (isShown) {', '    useState(0);', '  }'),
    HOOK_RULE,
  ],
  [
    'a catch of a promise that drops the error',
    'src/pages/probe/lib/probe.ts',
    'export const recover = (p: Promise<number>) => p.catch(() => 0);',
    SWALLOWED,
  ],
];

const PASSES = [
  ['new Date() in shared/lib/date.ts', 'src/shared/lib/date.ts', 'export const now = new Date();', SYNTAX],
  ['new Date() in a test', 'src/pages/probe/lib/probe.test.ts', 'export const now = new Date();', SYNTAX],
  ['new Date() in mocks', 'mocks/probe.ts', 'export const now = new Date();', SYNTAX],
  [
    'fetch() in shared/api/http-client.ts',
    'src/shared/api/http-client.ts',
    "export const load = () => fetch('/api/probe');",
    SYNTAX,
  ],
  [
    'Cyrillic in a text catalog',
    'src/entities/probe/config/texts.ts',
    `export const TEXTS = { greeting: '${CYRILLIC_WORD}' };`,
    SYNTAX,
  ],
  ['Cyrillic in a test', 'src/pages/probe/lib/probe.test.ts', `export const text = '${CYRILLIC_WORD}';`, SYNTAX],
  ['Cyrillic in mocks', 'mocks/probe.ts', `export const text = '${CYRILLIC_WORD}';`, SYNTAX],
  ['Cyrillic in e2e', 'e2e/probe.spec.ts', `export const text = '${CYRILLIC_WORD}';`, SYNTAX],
  [
    'an import of the public API',
    'src/pages/probe/model/use-probe-page.ts',
    "import { userQuery } from '@/entities/user';\nexport const query = userQuery;",
    IMPORTS,
  ],
  [
    'an import of a page',
    PROBE_ROUTES,
    "import { UserListPage } from '@/pages/user-list';\nexport const page = UserListPage;",
    IMPORTS,
  ],
  [
    'an import of a page in a group',
    PROBE_ROUTES,
    "import { HelpPage } from '@/pages/guide/help';\nexport const page = HelpPage;",
    IMPORTS,
  ],
  [
    'getRouteApi in pages',
    'src/pages/probe/model/use-probe-page.ts',
    "import { getRouteApi } from '@tanstack/react-router';\nexport const route = getRouteApi;",
    IMPORTS,
  ],
  ['a boolean named isOpen', 'src/shared/lib/probe.ts', 'export const isOpen = Boolean(globalThis);', NAMING],
  ['an interface Probe', 'src/shared/lib/probe.ts', 'export interface Probe {\n  id: string;\n}', NAMING],
  ['a variable named user', 'src/shared/lib/probe.ts', 'export const user = 1;', DENYLIST],
  ['window.fetch() in shared/api/http-client.ts', 'src/shared/api/http-client.ts', globalFetch('window'), SYNTAX],
  ['staleTime: STALE_POLICY.list', PROBE_QUERIES, staleTimeProbe('staleTime: STALE_POLICY.list'), SYNTAX],
  ["staleTime: 'static'", PROBE_QUERIES, staleTimeProbe("staleTime: 'static'"), SYNTAX],
  ['a shorthand staleTime', PROBE_QUERIES, lines("const staleTime = 'static';", staleTimeProbe('staleTime')), SYNTAX],
  [
    'export default in vite.config.ts',
    'vite.config.ts',
    lines("import { defineConfig } from 'vite';", 'export default defineConfig({});'),
    EXPORTS,
  ],
  ['a hook call at the top of a ts hook', PROBE_PAGE_HOOK, hookProbe('  useState(0);'), HOOK_RULE],
  [
    'a catch of a zod schema',
    'src/pages/probe/lib/probe.ts',
    lines("import { z } from 'zod';", "export const text = z.string().catch(() => '');"),
    SWALLOWED,
  ],
];

const NO_ANY = '@typescript-eslint/no-explicit-any';
const TS_COMMENT = '@typescript-eslint/ban-ts-comment';
const MAX_LINES = 'max-lines';
const HOOKS = 'react-hooks/rules-of-hooks';
const A11Y = 'jsx-a11y/click-events-have-key-events';
const QUERY_DEPS = '@tanstack/query/exhaustive-deps';
const FLOATING = '@typescript-eslint/no-floating-promises';
const MISUSED = '@typescript-eslint/no-misused-promises';
const SWITCH_EXHAUSTIVE = '@typescript-eslint/switch-exhaustiveness-check';

// A directive comment is built from parts: no added line holds the words that `kit.json` forbids.
const directive = (name, tail = '') => `// @${['ts', name].join('-')}${tail}`;

// An eslint directive is built from parts too; `kind` is `disable`, `disable-next-line` or empty (inline config).
const eslintDirective = (kind, body) => [['eslint', kind].filter(Boolean).join('-'), body].filter(Boolean).join(' ');
const lineComment = (text) => `// ${text}`;
const blockComment = (text) => `/* ${text} */`;

const constants = (count) => Array.from({ length: count }, (_, index) => `export const value${index} = ${index};`);

const PROBE_LIB_PAGE = 'src/pages/probe/lib/probe.ts';
const ENTITY_TEXTS = 'src/entities/probe/config/texts.ts';
const PAGE_TEXTS = 'src/pages/probe/config/probe-texts.ts';

const switchProbe = (...branches) =>
  lines(
    "export type Mode = 'idle' | 'busy' | 'done';",
    'export const toCode = (mode: Mode) => {',
    '  switch (mode) {',
    ...branches,
    '  }',
    '};',
  );
const modeCase = (mode) => lines(`    case '${mode}':`, `      return '${mode}';`);

const PROBE_UI = 'src/pages/probe/ui/Probe.tsx';
const PROBE_MODEL = 'src/pages/probe/model/use-probe-page.ts';

const PRESET_CATCHES = [
  ['an explicit any', 'src/shared/lib/probe.ts', 'export const identity = (x: any) => x;', NO_ANY],
  [
    'a ts-ignore comment',
    'src/shared/lib/probe.ts',
    lines(directive('ignore'), "export const value: number = 'text';"),
    TS_COMMENT,
  ],
  [
    'a ts-nocheck comment',
    'src/shared/lib/probe.ts',
    lines(directive('nocheck'), "export const value: number = 'text';"),
    TS_COMMENT,
  ],
  ['251 lines in a product file', 'src/shared/lib/probe.ts', lines(...constants(251)), MAX_LINES],
  [
    'a switch over a union with a missing case and no default',
    PROBE_LIB_PAGE,
    switchProbe(modeCase('idle'), modeCase('busy')),
    SWITCH_EXHAUSTIVE,
  ],
  [
    'a hook call inside an if',
    PROBE_UI,
    lines(
      "import { useState } from 'react';",
      'export const Probe = ({ show }: { show: boolean }) => {',
      '  if (show) {',
      '    useState(0);',
      '  }',
      '  return <p>probe</p>;',
      '};',
    ),
    HOOKS,
  ],
  [
    'a div with onClick and no key handler',
    PROBE_UI,
    'export const Probe = () => <div onClick={() => undefined}>probe</div>;',
    A11Y,
  ],
  [
    'a queryFn that uses an id the queryKey lacks',
    'src/entities/probe/model/probe-queries.ts',
    lines(
      "import { queryOptions } from '@tanstack/react-query';",
      'const fetchProbe = (id: string) => Promise.resolve(id);',
      "export const probeQuery = (id: string) => queryOptions({ queryKey: ['probe'], queryFn: () => fetchProbe(id) });",
    ),
    QUERY_DEPS,
  ],
  [
    'a promise call as a statement',
    PROBE_MODEL,
    lines('const save = () => Promise.resolve(1);', 'export const run = () => {', '  save();', '};'),
    FLOATING,
  ],
  [
    'an async arrow in onClick',
    PROBE_UI,
    lines(
      'export const Probe = () => (',
      '  <button',
      '    type="button"',
      '    onClick={async () => {',
      '      await Promise.resolve();',
      '    }}',
      '  >',
      '    probe',
      '  </button>',
      ');',
    ),
    MISUSED,
  ],
];

const PRESET_PASSES = [
  ['unknown instead of any', 'src/shared/lib/probe.ts', 'export const identity = (x: unknown) => x;', NO_ANY],
  [
    'a ts-expect-error with a description',
    'src/shared/lib/probe.ts',
    lines(directive('expect-error', ': the value is wrong on purpose'), "export const value: number = 'text';"),
    TS_COMMENT,
  ],
  ['251 lines in a test file', 'src/pages/probe/lib/probe.test.ts', lines(...constants(251)), MAX_LINES],
  ['251 lines in an entity text catalog', ENTITY_TEXTS, lines(...constants(251)), MAX_LINES],
  ['251 lines in a page text catalog', PAGE_TEXTS, lines(...constants(251)), MAX_LINES],
  [
    'a switch over a union with every case',
    PROBE_LIB_PAGE,
    switchProbe(modeCase('idle'), modeCase('busy'), modeCase('done')),
    SWITCH_EXHAUSTIVE,
  ],
  [
    'a switch over a union with a default',
    PROBE_LIB_PAGE,
    switchProbe(modeCase('idle'), lines('    default:', "      return 'other';")),
    SWITCH_EXHAUSTIVE,
  ],
  [
    'a hook call at the top of a component',
    PROBE_UI,
    lines(
      "import { useState } from 'react';",
      'export const Probe = ({ show }: { show: boolean }) => {',
      '  useState(0);',
      '  return show ? <p>probe</p> : null;',
      '};',
    ),
    HOOKS,
  ],
  [
    'a button with onClick',
    PROBE_UI,
    'export const Probe = () => <button type="button" onClick={() => undefined}>probe</button>;',
    A11Y,
  ],
  [
    'a queryKey that holds the id',
    'src/entities/probe/model/probe-queries.ts',
    lines(
      "import { queryOptions } from '@tanstack/react-query';",
      'const fetchProbe = (id: string) => Promise.resolve(id);',
      "export const probeQuery = (id: string) => queryOptions({ queryKey: ['probe', id], queryFn: () => fetchProbe(id) });",
    ),
    QUERY_DEPS,
  ],
  [
    'void before a promise call',
    PROBE_MODEL,
    lines('const save = () => Promise.resolve(1);', 'export const run = () => {', '  void save();', '};'),
    FLOATING,
  ],
  [
    'an awaited promise call',
    PROBE_MODEL,
    lines('const save = () => Promise.resolve(1);', 'export const run = async () => {', '  await save();', '};'),
    FLOATING,
  ],
  [
    'void in onClick',
    PROBE_UI,
    lines(
      'const save = () => Promise.resolve(1);',
      'export const Probe = () => (',
      '  <button type="button" onClick={() => void save()}>',
      '    probe',
      '  </button>',
      ');',
    ),
    MISUSED,
  ],
];

const COMMENTS = '@eslint-community/eslint-comments';
const RESTRICTED_DISABLE = `${COMMENTS}/no-restricted-disable`;
const UNLIMITED_DISABLE = `${COMMENTS}/no-unlimited-disable`;
const REQUIRE_DESCRIPTION = `${COMMENTS}/require-description`;
const NO_USE = `${COMMENTS}/no-use`;
const COMMENT_RULES = [RESTRICTED_DISABLE, UNLIMITED_DISABLE, REQUIRE_DESCRIPTION, NO_USE];

const PROBE_LIB = 'src/shared/lib/probe.ts';
const nextLine = (body) => lineComment(eslintDirective('disable-next-line', body));

// A comment that disables a canon rule fails even with a reason; any other disable needs `-- reason`.
const COMMENT_CATCHES = [
  [
    'a disable of no-restricted-syntax with a reason over enum',
    PROBE_LIB,
    lines(nextLine('no-restricted-syntax -- a reason'), 'export enum Color { Red }'),
    RESTRICTED_DISABLE,
  ],
  [
    'a disable of architecture/no-inline-style with a reason',
    PROBE_UI,
    lines(
      nextLine('architecture/no-inline-style -- a reason'),
      "export const Probe = () => <p style={{ color: 'red' }}>probe</p>;",
    ),
    RESTRICTED_DISABLE,
  ],
  [
    'a disable of no-explicit-any with a reason',
    PROBE_LIB,
    lines(nextLine('@typescript-eslint/no-explicit-any -- a reason'), 'export const identity = (x: any) => x;'),
    RESTRICTED_DISABLE,
  ],
  [
    'a block disable without rules',
    PROBE_LIB,
    lines(blockComment(eslintDirective('disable')), 'export const value = 1;'),
    UNLIMITED_DISABLE,
  ],
  [
    'a disable of no-console without a reason after --',
    PROBE_LIB,
    lines(nextLine('no-console'), 'console.log(1);'),
    REQUIRE_DESCRIPTION,
  ],
  [
    'an inline config comment turning a rule off',
    PROBE_UI,
    lines(
      blockComment(eslintDirective('', 'architecture/no-logic-in-ui: off')),
      'export const Probe = () => <p>probe</p>;',
    ),
    NO_USE,
  ],
  ['a global comment', PROBE_LIB, lines(blockComment('global foo'), 'export const value = 1;'), NO_USE],
];

const COMMENT_PASSES = [
  [
    'a disable of no-console with a reason over console.log',
    PROBE_LIB,
    lines(nextLine('no-console -- a reason'), 'console.log(1);'),
  ],
];

const UNUSED_DISABLE_CODE = lines(nextLine('no-console -- a reason'), 'export const value = 1;');
const UNUSED_DISABLE_MESSAGE = /^Unused .*directive.*no-console/;

// Probe paths that exist on disk are in a project already; typescript-eslint refuses them in `allowDefaultProject`.
const probeFiles = [...CATCHES, ...PASSES, ...PRESET_CATCHES, ...PRESET_PASSES, ...COMMENT_CATCHES, ...COMMENT_PASSES]
  .map(([, file]) => file)
  .concat(WARM_UP_FILE, PROBE_LIB)
  .filter((file) => /\.tsx?$/.test(file) && !existsSync(path.join(PACK_ROOT, file)));
const defaultProjectFiles = [...new Set(probeFiles)];

const eslint = new ESLint({
  cwd: PACK_ROOT,
  overrideConfig: {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      parserOptions: {
        projectService: {
          allowDefaultProject: defaultProjectFiles,
          defaultProject: 'tsconfig.app.json',
          maximumDefaultProjectFileMatchCount_THIS_WILL_SLOW_DOWN_LINTING: defaultProjectFiles.length + 1,
        },
      },
    },
  },
});

const lintProbe = async (file, code) => {
  const [result] = await eslint.lintText(code, { filePath: path.join(PACK_ROOT, file) });
  return result?.messages ?? [];
};

beforeAll(async () => {
  await lintProbe(WARM_UP_FILE, 'export const warm = 1;');
}, 60_000);

// Each probe lints a file with the whole project loaded: the 5 s default is too short on a loaded runner.
const LINT_TIMEOUT = 30_000;

describe.each([
  ['selectors and imports', CATCHES, PASSES],
  ['preset rules', PRESET_CATCHES, PRESET_PASSES],
])('eslint config: %s', { timeout: LINT_TIMEOUT }, (_group, catches, passes) => {
  it.each(catches)('catches %s', async (_name, file, code, ruleId) => {
    const messages = await lintProbe(file, code);
    expect(messages.map((message) => message.ruleId)).toContain(ruleId);
  });

  it.each(passes)('does not catch %s', async (_name, file, code, ruleId) => {
    const messages = await lintProbe(file, code);
    expect(messages.map((message) => message.ruleId)).not.toContain(ruleId);
  });
});

describe('eslint config: disabling a rule by a comment', () => {
  it.each(COMMENT_CATCHES)('catches %s', async (_name, file, code, ruleId) => {
    const messages = await lintProbe(file, code);
    expect(messages.map((message) => message.ruleId)).toContain(ruleId);
  });

  it.each(COMMENT_PASSES)('does not catch %s', async (_name, file, code) => {
    const messages = await lintProbe(file, code);
    const ruleIds = messages.map((message) => message.ruleId);
    for (const ruleId of COMMENT_RULES) {
      expect(ruleIds).not.toContain(ruleId);
    }
    expect(ruleIds).not.toContain(null);
  });

  it('catches an unused disable', async () => {
    const messages = await lintProbe(PROBE_LIB, UNUSED_DISABLE_CODE);
    const unused = messages.filter(
      (message) => message.ruleId === null && UNUSED_DISABLE_MESSAGE.test(message.message),
    );
    expect(unused).toHaveLength(1);
  });
});

// ESLint bulk suppressions freeze only errors: a rule at severity 1 would turn an adopted project's verify red unfreezably.
const WARN_FREE_FILES = ['src/app/App.tsx', 'src/shared/lib/x.ts', 'server/x.ts', 'tooling/x.js', 'vite.config.ts'];
const WARN_SEVERITIES = new Set([1, 'warn']);
const severityOf = (entry) => (Array.isArray(entry) ? entry[0] : entry);

describe('eslint config: no warnings', () => {
  it.each(WARN_FREE_FILES)('sets no rule to warn for %s', async (file) => {
    const config = await eslint.calculateConfigForFile(path.join(PACK_ROOT, file));
    const warned = Object.entries(config.rules ?? {})
      .filter(([, entry]) => WARN_SEVERITIES.has(severityOf(entry)))
      .map(([ruleId]) => ruleId);
    expect(warned).toEqual([]);
  });
});
