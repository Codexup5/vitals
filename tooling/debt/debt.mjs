#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';

import {
  findForeignChanges,
  FOREIGN_CHANGES_LINE,
  listChangedPaths,
  readKitPaths,
  readUntouchedKitFiles,
} from '../../scripts/lib/kit-files.mjs';
import { judgeDebt } from './judge.mjs';
import {
  findKnipFindings,
  findPlacementFindings,
  findSteigerFindings,
  findStylelintFindings,
  isInScope,
  toRootPaths,
} from './tools.mjs';

const ROOT = path.resolve(import.meta.dirname, '../..');

const CHECKS = {
  stylelint: {
    find: (root, options) => findStylelintFindings(root, options),
    labels: { file: 'stylelint-baseline.json', freeze: 'lint:styles:baseline', noun: 'stylelint findings' },
  },
  steiger: {
    find: (root, options) => findSteigerFindings(root, options),
    labels: { file: 'steiger-baseline.json', freeze: 'lint:arch:baseline', noun: 'steiger findings' },
  },
  placement: {
    find: (root, options) => findPlacementFindings(root, options),
    labels: { file: 'placement-baseline.json', freeze: 'lint:placement:baseline', noun: 'placement findings' },
  },
  knip: {
    find: (root, options) => findKnipFindings(root, options),
    labels: { file: 'knip-baseline.json', freeze: 'knip:baseline', noun: 'knip findings' },
  },
};

const fail = (lines) => {
  process.stderr.write(`${lines.join('\n')}\n`);
  return 1;
};

const readBaseline = (file) => {
  if (!fs.existsSync(file)) return null;
  let parsed;
  try {
    parsed = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (error) {
    throw new Error(`${path.basename(file)} cannot be read: ${error.message}`, { cause: error });
  }
  if (typeof parsed?.findings !== 'object' || parsed.findings === null) {
    throw new Error(`${path.basename(file)} has no "findings" object`);
  }
  return parsed.findings;
};

const writeBaseline = (file, findings) => {
  const sorted = Object.fromEntries(Object.entries(findings).sort(([left], [right]) => (left < right ? -1 : 1)));
  fs.writeFileSync(file, `${JSON.stringify({ findings: sorted }, null, 2)}\n`);
};

// A run on given files judges only the baseline entries of those files, the rest is not looked at
const scopeBaseline = (baseline, given) =>
  baseline === null || given === undefined
    ? baseline
    : Object.fromEntries(Object.entries(baseline).filter(([key]) => isInScope(key.split('|')[0], given)));

const run = async () => {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      baseline: { type: 'boolean', default: false },
      sync: { type: 'boolean', default: false },
      fix: { type: 'boolean', default: false },
    },
  });
  const [name, ...files] = positionals;
  const check = CHECKS[name];
  if (check === undefined) return fail([`unknown check ${name}: use ${Object.keys(CHECKS).join(', ')}`]);
  if (values.baseline && values.sync) return fail(['use --baseline or --sync, not both']);
  const mode = values.baseline ? 'baseline' : values.sync ? 'sync' : 'check';
  if (mode !== 'check' && files.length > 0) return fail([`--${mode} freezes the whole project: give no files`]);
  if (values.fix && mode !== 'check') return fail([`--fix works with a check, not with --${mode}`]);

  const kitPaths = readKitPaths(ROOT);
  if (mode === 'sync') {
    const foreign = findForeignChanges(listChangedPaths(ROOT), kitPaths);
    if (foreign.length > 0) return fail([FOREIGN_CHANGES_LINE, ...foreign]);
  }

  const given = files.length > 0 ? toRootPaths(ROOT, files) : undefined;
  const baselinePath = path.join(ROOT, check.labels.file);
  const findings = await check.find(ROOT, { files: given, fix: values.fix });
  const result = judgeDebt({
    mode,
    findings,
    baseline: scopeBaseline(readBaseline(baselinePath), given),
    kitFiles: readUntouchedKitFiles(ROOT),
    labels: check.labels,
  });
  if (result.write !== undefined) writeBaseline(baselinePath, result.write);
  if (result.lines.length > 0) {
    (result.exitCode === 0 ? process.stdout : process.stderr).write(`${result.lines.join('\n')}\n`);
  }
  return result.exitCode;
};

try {
  process.exitCode = await run();
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
