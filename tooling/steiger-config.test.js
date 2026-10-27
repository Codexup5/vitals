import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const PACK_ROOT = path.resolve(import.meta.dirname, '..');
const STEIGER_CLI = path.join(PACK_ROOT, 'node_modules/steiger/dist/cli.mjs');
const PACK_CONFIG_URL = pathToFileURL(path.join(PACK_ROOT, 'steiger.config.mjs')).href;

// A temp project: the steiger config re-exports the config of the pack, the sources import each other by relative paths.
const FILES = {
  'steiger.config.mjs': `export { default } from '${PACK_CONFIG_URL}';\n`,
  'src/entities/user/model/user.ts': 'export const user = 1;\n',
  'src/entities/user/index.ts': "export { user } from './model/user';\n",
  'src/entities/order/model/order.ts': "import { user } from '../../user/model/user';\n\nexport const order = user;\n",
  'src/entities/order/index.ts': "export { order } from './model/order';\n",
  'src/shared/lib/money.ts': 'export const toMoney = 1;\n',
  'src/pages/order/ui/OrderPage.tsx':
    "import { order } from '../../../entities/order';\nimport { toMoney } from '../../../shared/lib/money';\n\nexport const OrderPage = () => order + toMoney;\n",
  'src/pages/order/index.ts': "export { OrderPage } from './ui/OrderPage';\n",
  'src/pages/guide/help/ui/HelpPage.tsx': 'export const HelpPage = () => null;\n',
  'src/pages/guide/help/index.ts': "export { HelpPage } from './ui/HelpPage';\n",
  'src/pages/guide/releases/ui/ReleasesPage.tsx': 'export const ReleasesPage = () => null;\n',
  'src/pages/guide/releases/index.ts': "export { ReleasesPage } from './ui/ReleasesPage';\n",
};

const ORDER_MODEL = 'src/entities/order/model/order.ts';
const ORDER_PAGE = 'src/pages/order/ui/OrderPage.tsx';
const MONEY = 'src/shared/lib/money.ts';

const reportsOf = (diagnostics, ruleName) =>
  diagnostics
    .filter((diagnostic) => diagnostic.ruleName === ruleName)
    .map((diagnostic) => diagnostic.location.path.replaceAll('\\', '/'));

describe('steiger config', () => {
  let projectRoot = '';
  let diagnostics = [];

  beforeAll(() => {
    projectRoot = mkdtempSync(path.join(tmpdir(), 'steiger-probe-'));
    for (const [file, content] of Object.entries(FILES)) {
      const target = path.join(projectRoot, file);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    const result = spawnSync(process.execPath, [STEIGER_CLI, './src', '--reporter', 'json'], {
      cwd: projectRoot,
      encoding: 'utf8',
    });
    diagnostics = JSON.parse(result.stdout);
  }, 60_000);

  afterAll(() => {
    rmSync(projectRoot, { recursive: true, force: true });
  });

  it('catches an import between slices of one layer', () => {
    const files = reportsOf(diagnostics, 'fsd/forbidden-imports');
    expect(files.some((file) => file.endsWith(ORDER_MODEL))).toBe(true);
  });

  it('does not catch an import of a public API from a higher layer', () => {
    const files = reportsOf(diagnostics, 'fsd/forbidden-imports');
    expect(files.some((file) => file.endsWith(ORDER_PAGE))).toBe(false);
  });

  it('does not catch a slice group in pages', () => {
    const inGroup = diagnostics
      .filter((diagnostic) => diagnostic.ruleName !== 'fsd/insignificant-slice')
      .map((diagnostic) => diagnostic.location.path.replaceAll('\\', '/'))
      .filter((file) => file.includes('/src/pages/guide'));
    expect(inGroup).toEqual([]);
  });

  it('does not ask a public API of shared', () => {
    expect(reportsOf(diagnostics, 'fsd/public-api')).toEqual([]);
    expect(diagnostics.some((diagnostic) => diagnostic.location.path.replaceAll('\\', '/').endsWith(MONEY))).toBe(
      false,
    );
  });
});
