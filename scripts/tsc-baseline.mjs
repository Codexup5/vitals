#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

import {
  findForeignChanges,
  findKitFindings,
  FOREIGN_CHANGES_LINE,
  KIT_FILES_LINE,
  listChangedPaths,
  readKitPaths,
  readUntouchedKitFiles,
} from './lib/kit-files.mjs';
import {
  findNewErrors,
  groupTscOutput,
  hasGlobalError,
  isCrash,
  listProjects,
  pruneBaseline,
  toFingerprints,
  toProjectPath,
} from './lib/tsc-baseline.mjs';

const PACKAGE_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const BASELINE_FILE = 'tsc-baseline.json';
const BASELINE_PATH = path.join(PACKAGE_DIR, BASELINE_FILE);

const fail = (message) => {
  process.stderr.write(`${message}\n`);
  process.exit(1);
};

const asProjectFile = (reference) => {
  const posix = path.posix.normalize(reference.replaceAll('\\', '/'));
  return posix.endsWith('.json') ? posix : path.posix.join(posix, 'tsconfig.json');
};

const readRootConfig = (name) => {
  const { config, error } = ts.readConfigFile(path.join(PACKAGE_DIR, name), ts.sys.readFile);
  if (error) fail(`${name}: ${ts.flattenDiagnosticMessageText(error.messageText, '\n')}`);
  return config;
};

const findProjects = () => {
  const entries = fs.readdirSync(PACKAGE_DIR, { withFileTypes: true });
  const rootConfigs = entries
    .filter((entry) => entry.isFile() && /^tsconfig.*\.json$/.test(entry.name))
    .map((entry) => entry.name);
  const configs = rootConfigs.map(readRootConfig);
  const referenced = configs.flatMap((config) =>
    (config.references ?? []).map((reference) => asProjectFile(reference.path)),
  );
  const extended = configs
    .flatMap((config) => [config.extends ?? []].flat())
    .filter((target) => target.startsWith('.'))
    .map((target) => asProjectFile(target.endsWith('.json') ? target : `${target}.json`));
  const folders = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => ({
      name: entry.name,
      hasTsconfig: fs.existsSync(path.join(PACKAGE_DIR, entry.name, 'tsconfig.json')),
    }));
  return listProjects({ rootConfigs, referenced, extended, folders });
};

const runTsc = () => {
  const tsc = createRequire(import.meta.url).resolve('typescript/bin/tsc');
  const result = spawnSync(process.execPath, [tsc, '-b', ...findProjects(), '--noEmit', '--pretty', 'false'], {
    cwd: PACKAGE_DIR,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  });
  if (result.error) fail(`tsc did not run: ${result.error.message}`);
  return { output: result.stdout + result.stderr, status: result.status ?? 1, signal: result.signal };
};

const writeBaseline = (errors) => {
  const sorted = Object.fromEntries(Object.entries(errors).sort(([a], [b]) => (a < b ? -1 : 1)));
  fs.writeFileSync(BASELINE_PATH, `${JSON.stringify({ errors: sorted }, null, 2)}\n`);
};

const readBaseline = () => {
  let parsed;
  try {
    parsed = JSON.parse(fs.readFileSync(BASELINE_PATH, 'utf8'));
  } catch (error) {
    return fail(`${BASELINE_FILE} cannot be read: ${error.message}`);
  }
  if (typeof parsed?.errors !== 'object' || parsed.errors === null) {
    return fail(`${BASELINE_FILE} has no "errors" object`);
  }
  return parsed.errors;
};

const sumCounts = (counts) => Object.values(counts).reduce((sum, count) => sum + count, 0);

const refuseKitFileErrors = (errors, kitFiles) => {
  const inKitFiles = findKitFindings(errors, kitFiles);
  if (inKitFiles.length === 0) return;
  const lines = inKitFiles.map(({ file, code, message }) => `${file}: ${code} — ${message}`);
  fail(`${lines.join('\n')}\n\n${BASELINE_FILE} was not written: ${KIT_FILES_LINE}`);
};

const mode = process.argv[2];
if (mode !== undefined && mode !== '--update' && mode !== '--sync') {
  fail(`unknown argument ${mode}: use --update, --sync or no argument`);
}

const kitPaths = readKitPaths(PACKAGE_DIR);
const untouchedKitFiles = readUntouchedKitFiles(PACKAGE_DIR);
const hasBaseline = fs.existsSync(BASELINE_PATH);

if (mode === '--sync') {
  const foreign = findForeignChanges(listChangedPaths(PACKAGE_DIR), kitPaths);
  if (foreign.length > 0) fail([FOREIGN_CHANGES_LINE, ...foreign].join('\n'));
  if (!hasBaseline) {
    process.stdout.write(`no ${BASELINE_FILE}: nothing to sync\n`);
    process.exit(0);
  }
}

const { output, status, signal } = runTsc();
const { groups: rawGroups, unrecognized } = groupTscOutput(output);

if (signal != null) fail(`${output.trimEnd()}\ntsc was killed by ${signal}`.trimStart());
if (hasGlobalError(output) || isCrash(status, unrecognized, signal) || (status !== 0 && rawGroups.length === 0)) {
  fail(output.trimEnd());
}

const groups = rawGroups.map(({ error, lines }) => ({
  error: { ...error, file: toProjectPath(error.file, PACKAGE_DIR) },
  lines,
}));
const errors = groups.map((group) => group.error);

if (mode === '--sync') {
  refuseKitFileErrors(errors, untouchedKitFiles);
  const fingerprints = toFingerprints(errors);
  writeBaseline(fingerprints);
  process.stdout.write(`${BASELINE_FILE}: ${sumCounts(fingerprints)} known type errors\n`);
} else if (mode === '--update') {
  if (!hasBaseline) refuseKitFileErrors(errors, untouchedKitFiles);
  const fingerprints = hasBaseline ? pruneBaseline(errors, readBaseline()) : toFingerprints(errors);
  writeBaseline(fingerprints);
  process.stdout.write(`${BASELINE_FILE}: ${sumCounts(fingerprints)} known type errors\n`);
} else if (!hasBaseline) {
  process.stdout.write(output);
  process.exit(status);
} else {
  const baseline = readBaseline();
  const fresh = new Set(findNewErrors(errors, baseline));
  if (fresh.size > 0) {
    const lines = groups.filter((group) => fresh.has(group.error)).flatMap((group) => group.lines);
    fail(`${lines.join('\n')}\n\n${fresh.size} new type errors past ${BASELINE_FILE}`);
  }
  const fixed = sumCounts(baseline) - sumCounts(pruneBaseline(errors, baseline));
  if (fixed > 0) {
    process.stdout.write(`${fixed} fixed type errors still counted in ${BASELINE_FILE}: run pnpm typecheck:baseline\n`);
  } else {
    process.stdout.write(`${groups.length} known type errors (${BASELINE_FILE})\n`);
  }
}
