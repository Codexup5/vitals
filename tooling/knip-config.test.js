import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const PACK_ROOT = path.resolve(import.meta.dirname, '..');
const KNIP_CLI = path.join(PACK_ROOT, 'node_modules/knip/bin/knip.js');

// A temp project with the knip.json of the pack: mocks/handlers.ts is an entry of that config and uses one export.
// api/trpc/[trpc].ts is a nested Vercel route: api/**/*.ts is an entry, so nothing imports it and it is not reported.
const FILES = {
  'package.json': `${JSON.stringify({ name: 'knip-probe', private: true, type: 'module' })}\n`,
  'mocks/handlers.ts': "import { used } from '../src/entities/probe/index.ts';\n\nexport const handlers = [used];\n",
  'api/trpc/[trpc].ts': "export default function handler() {\n  return 'ok';\n}\n",
  'src/entities/probe/index.ts': 'export const used = 1;\nexport const unused = 2;\n',
  'src/shared/lib/orphan.ts': 'export const orphan = 3;\n',
};

describe('knip config', () => {
  let projectRoot = '';
  let issues = [];

  beforeAll(() => {
    projectRoot = mkdtempSync(path.join(tmpdir(), 'knip-probe-'));
    copyFileSync(path.join(PACK_ROOT, 'knip.json'), path.join(projectRoot, 'knip.json'));
    for (const [file, content] of Object.entries(FILES)) {
      const target = path.join(projectRoot, file);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    const result = spawnSync(process.execPath, [KNIP_CLI, '--directory', projectRoot, '--reporter', 'json'], {
      encoding: 'utf8',
    });
    ({ issues } = JSON.parse(result.stdout));
  }, 60_000);

  afterAll(() => {
    rmSync(projectRoot, { recursive: true, force: true });
  });

  const exportsOf = () => issues.flatMap((issue) => issue.exports.map((item) => item.name));
  const filesOf = () => issues.flatMap((issue) => issue.files.map((item) => item.name.replaceAll('\\', '/')));

  it('catches an export nobody imports', () => {
    expect(exportsOf()).toContain('unused');
  });

  it('catches a file nobody imports', () => {
    expect(filesOf()).toContain('src/shared/lib/orphan.ts');
  });

  it('does not catch a nested Vercel route nobody imports', () => {
    expect(filesOf()).not.toContain('api/trpc/[trpc].ts');
  });

  it('does not catch an export an entry imports', () => {
    expect(exportsOf()).not.toContain('used');
    expect(filesOf()).not.toContain('src/entities/probe/index.ts');
  });
});
