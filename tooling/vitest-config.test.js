import { copyFileSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { loadConfigFromFile } from 'vite';
import { describe, expect, it } from 'vitest';

const PACK_ROOT = path.resolve(import.meta.dirname, '..');
const APP_SETUP = 'src/app/test/setup.ts';

// the config is loaded from a temp project: a copy of the pack's vitest.config.ts, a stub vite.config.ts (the real one
// imports tooling/security/csp.mjs) and the pack's node_modules as a junction; the app setup file is there only on demand
const loadAppProject = async ({ withSetup }) => {
  const root = mkdtempSync(path.join(tmpdir(), 'vitest-config-'));
  const link = path.join(root, 'node_modules');
  writeFileSync(path.join(root, 'package.json'), '{ "type": "module" }\n');
  copyFileSync(path.join(PACK_ROOT, 'vitest.config.ts'), path.join(root, 'vitest.config.ts'));
  writeFileSync(path.join(root, 'vite.config.ts'), 'export default {};\n');
  if (withSetup) {
    mkdirSync(path.join(root, 'src/app/test'), { recursive: true });
    writeFileSync(path.join(root, APP_SETUP), 'export {};\n');
  }
  let linked = false;
  try {
    symlinkSync(path.join(PACK_ROOT, 'node_modules'), link, 'junction');
    linked = true;
    // the 'runner' loader evaluates the config in memory: no bundled temp file lands in the pack's node_modules
    const loaded = await loadConfigFromFile(
      { command: 'serve', mode: 'test' },
      path.join(root, 'vitest.config.ts'),
      root,
      undefined,
      undefined,
      'runner',
    );
    if (!loaded) throw new Error('vitest.config.ts did not load');
    const app = loaded.config.test.projects.find((project) => project?.test?.name === 'app');
    return app.test;
  } finally {
    if (linked) unlinkSync(link);
    rmSync(root, { recursive: true, force: true });
  }
};

describe('vitest config app project', () => {
  it('has no setup file when the app does not ship src/app/test/setup.ts', async () => {
    const app = await loadAppProject({ withSetup: false });

    expect(app.setupFiles).toEqual([]);
  });

  it('lists src/app/test/setup.ts when the app ships it', async () => {
    const app = await loadAppProject({ withSetup: true });

    expect(app.setupFiles).toEqual(['./src/app/test/setup.ts']);
  });
});
