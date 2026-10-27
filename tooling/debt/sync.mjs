#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

import {
  findForeignChanges,
  FOREIGN_CHANGES_LINE,
  listChangedPaths,
  readKitPaths,
} from '../../scripts/lib/kit-files.mjs';

const ROOT = path.resolve(import.meta.dirname, '../..');

// Each check freezes with its own --sync: the refusal and the "nothing to sync" without a baseline are its own
const STEPS = [
  { name: 'types', script: 'scripts/tsc-baseline.mjs', args: ['--sync'] },
  { name: 'eslint', script: 'tooling/eslint.mjs', args: ['--sync'] },
  { name: 'stylelint', script: 'tooling/debt/debt.mjs', args: ['stylelint', '--sync'] },
  { name: 'steiger', script: 'tooling/debt/debt.mjs', args: ['steiger', '--sync'] },
  { name: 'placement', script: 'tooling/debt/debt.mjs', args: ['placement', '--sync'] },
  { name: 'knip', script: 'tooling/debt/debt.mjs', args: ['knip', '--sync'] },
  { name: 'design tokens', script: 'tooling/design/tokens.mjs', args: ['--sync'] },
];

const run = () => {
  // No lock means no kit update (the kit repository itself): nothing came in to freeze
  if (!fs.existsSync(path.join(ROOT, 'kit-lock.json'))) {
    process.stdout.write('no kit-lock.json: debt:sync freezes the debt a kit update brings, nothing to sync\n');
    return 0;
  }
  const foreign = findForeignChanges(listChangedPaths(ROOT), readKitPaths(ROOT));
  if (foreign.length > 0) {
    process.stderr.write(`${[FOREIGN_CHANGES_LINE, ...foreign].join('\n')}\n`);
    return 1;
  }
  for (const { name, script, args } of STEPS) {
    process.stdout.write(`debt:sync ${name}\n`);
    const result = spawnSync(process.execPath, [path.join(ROOT, script), ...args], { cwd: ROOT, stdio: 'inherit' });
    if (result.error)
      throw new Error(`debt:sync ${name} did not run: ${result.error.message}`, { cause: result.error });
    if (result.status !== 0) {
      process.stderr.write(`debt:sync stopped at ${name}\n`);
      return result.status ?? 1;
    }
  }
  return 0;
};

try {
  process.exitCode = run();
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
