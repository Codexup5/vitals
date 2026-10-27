import { spawnSync } from 'node:child_process';
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

const SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'gen-context.mjs');
const CATALOG = path.join('docs', 'context', 'public-api.md');

let project;

const write = (relativePath, text) => {
  const file = path.join(project, relativePath);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, text);
};

const run = (...args) => spawnSync(process.execPath, [SCRIPT, ...args], { cwd: project, encoding: 'utf8' });

beforeEach(() => {
  project = mkdtempSync(path.join(tmpdir(), 'gen-context-'));
  write(
    'src/entities/demo/index.ts',
    "export { demoValue } from './model/demo';\nexport type { DemoItem } from './model/demo';\n",
  );
  write('src/shared/lib/demo.ts', 'export function formatDemo(): string {\n  return "demo";\n}\n');
  mkdirSync(path.join(project, 'src', 'features', 'empty'), { recursive: true });
});

afterEach(() => {
  rmSync(project, { recursive: true, force: true });
});

describe('gen-context', () => {
  it('writes the catalog lines in the committed format', () => {
    const result = run();

    expect(result.status).toBe(0);
    const catalog = readFileSync(path.join(project, CATALOG), 'utf8');
    expect(catalog).toContain('## shared\n\nImport by the direct file path.\n');
    expect(catalog).toContain('- `@/shared/lib/demo` — `formatDemo`\n');
    expect(catalog).toContain('## entities\n');
    expect(catalog).toContain('- **`@/entities/demo`** — `demoValue` · types: `DemoItem`\n');
    expect(catalog).toContain('- **`@/features/empty`** — no public API\n');
  });

  it('lists a slice of a pages group by its own path, without a line of the group', () => {
    write('src/pages/guide/help/index.ts', "export { HelpPage } from './ui/HelpPage';\n");
    write('src/pages/guide/help/ui/HelpPage.tsx', 'export const HelpPage = () => null;\n');

    expect(run().status).toBe(0);

    const catalog = readFileSync(path.join(project, CATALOG), 'utf8');
    expect(catalog).toContain('- **`@/pages/guide/help`** — `HelpPage`\n');
    expect(catalog).not.toContain('`@/pages/guide`');
  });

  it('does not list a segment of a pages folder as a group slice', () => {
    write('src/pages/stray/ui/index.ts', "export { StrayPage } from './StrayPage';\n");

    expect(run().status).toBe(0);

    expect(readFileSync(path.join(project, CATALOG), 'utf8')).not.toContain('`@/pages/stray/ui`');
  });

  it('passes --check right after generation', () => {
    run();

    expect(run('--check').status).toBe(0);
  });

  it('fails --check with the stale message once the index gains an export', () => {
    run();
    appendFileSync(path.join(project, 'src/entities/demo/index.ts'), "export { demoExtra } from './model/extra';\n");

    const result = run('--check');

    expect(result.status).toBe(1);
    expect(result.stderr).toContain('docs/context/public-api.md is stale: run `pnpm context`');
  });
});
