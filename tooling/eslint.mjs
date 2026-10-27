#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

import {
  findForeignChanges,
  findKitFindings,
  FOREIGN_CHANGES_LINE,
  KIT_FILES_LINE,
  listChangedPaths,
  readKitPaths,
  readUntouchedKitFiles,
} from '../scripts/lib/kit-files.mjs';
import { baselineRuns, eslintArgv } from './eslint-args.mjs';

const ROOT = path.resolve(import.meta.dirname, '..');
const SUPPRESSIONS_FILE = 'eslint-suppressions.json';
const SUPPRESSIONS_PATH = path.join(ROOT, SUPPRESSIONS_FILE);
const PASS_ON_UNPRUNED = '--pass-on-unpruned-suppressions';
const GROWING_FLAGS = ['--suppress-all', '--suppress-rule'];
const MODES = new Map([
  ['--baseline', 'baseline'],
  ['--sync', 'sync'],
]);

const fail = (lines) => {
  process.stderr.write(`${lines.join('\n')}\n`);
  return 1;
};

const packagePath = createRequire(import.meta.url).resolve('eslint/package.json');
const eslintPackage = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
const eslintBin = path.join(
  path.dirname(packagePath),
  typeof eslintPackage.bin === 'string' ? eslintPackage.bin : eslintPackage.bin.eslint,
);

const runEslint = (args, cwd) => {
  const result = spawnSync(process.execPath, [eslintBin, ...eslintArgv(eslintPackage.version, args)], {
    cwd,
    stdio: 'inherit',
  });
  if (result.error) throw new Error(`eslint did not run: ${result.error.message}`, { cause: result.error });
  return result.status ?? 1;
};

const readSuppressions = () => (fs.existsSync(SUPPRESSIONS_PATH) ? fs.readFileSync(SUPPRESSIONS_PATH, 'utf8') : null);

const refuseKitFiles = (previous, kitFiles) => {
  const written = readSuppressions();
  if (written === null || written === previous) return 0;
  const suppressed = Object.keys(JSON.parse(written)).map((file) => ({ file: file.replaceAll('\\', '/') }));
  const inKitFiles = findKitFindings(suppressed, kitFiles);
  if (inKitFiles.length === 0) return 0;
  if (previous === null) fs.rmSync(SUPPRESSIONS_PATH);
  else fs.writeFileSync(SUPPRESSIONS_PATH, previous);
  return fail([
    `${SUPPRESSIONS_FILE} was not written, it would freeze kit files:`,
    ...inKitFiles.map(({ file }) => file),
    KIT_FILES_LINE,
  ]);
};

const runFreeze = (mode, extraArgs) => {
  const kitPaths = readKitPaths(ROOT);
  if (mode === 'sync') {
    const foreign = findForeignChanges(listChangedPaths(ROOT), kitPaths);
    if (foreign.length > 0) return fail([FOREIGN_CHANGES_LINE, ...foreign]);
  }
  const previous = readSuppressions();
  const runs = baselineRuns(mode, previous !== null);
  let eslintStatus = 0;
  for (const run of runs) {
    // a suppress run after a rule left the kit would exit 2 on its stale entries, which the prune run drops next
    const pass = run.includes('--suppress-all') ? [PASS_ON_UNPRUNED] : [];
    eslintStatus = runEslint([...run, ...pass, ...extraArgs], ROOT);
    // ESLint writes the file before it picks the exit code: a failed run (a parse error) freezes nothing
    if (eslintStatus !== 0) {
      if (previous === null) fs.rmSync(SUPPRESSIONS_PATH, { force: true });
      else fs.writeFileSync(SUPPRESSIONS_PATH, previous);
      return fail([`${SUPPRESSIONS_FILE} was not written: eslint exited ${eslintStatus}, fix the errors it printed`]);
    }
  }
  const status = runs.length > 0 ? refuseKitFiles(previous, readUntouchedKitFiles(ROOT)) : 0;
  // ESLint writes the file in a shape prettier --check rejects: no final newline, an empty object over two lines
  const written = readSuppressions();
  const formatted = written === null ? null : `${JSON.stringify(JSON.parse(written), null, 2)}\n`;
  if (status === 0 && formatted !== null && formatted !== written) fs.writeFileSync(SUPPRESSIONS_PATH, formatted);
  return status === 0 ? eslintStatus : status;
};

// a plain run never grows the baseline, and a fixed frozen finding passes until the next freeze shrinks the file
const runPlain = (args) => {
  const growing = args.filter((arg) => GROWING_FLAGS.some((flag) => arg === flag || arg.startsWith(`${flag}=`)));
  if (growing.length > 0) {
    return fail([`${growing.join(', ')}: the baseline grows only through --baseline or --sync`]);
  }
  return runEslint(args.includes(PASS_ON_UNPRUNED) ? args : [...args, PASS_ON_UNPRUNED], process.cwd());
};

const [first, ...rest] = process.argv.slice(2);
const mode = MODES.get(first);

process.exit(mode ? runFreeze(mode, rest) : runPlain(process.argv.slice(2)));
