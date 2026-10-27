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
import { findNewErrors, pruneBaseline } from '../../scripts/lib/tsc-baseline.mjs';
import { judgeDebt } from '../debt/judge.mjs';
import { buildTokens, CONFIG_FILE, OUTPUT_FILE, readTokenConfig, TokenConfigError } from './build.mjs';
import { readContexts } from './contexts.mjs';
import { checkTokens, RULES } from './rules.mjs';

const DEFAULT_ROOT = path.resolve(import.meta.dirname, '../..');
const SOURCES = Object.fromEntries(RULES.map(({ id, source }) => [id, source]));
const BASELINE_FILE = 'design-baseline.json';

const normalizeLineEnds = (text) => text.replace(/\r\n/g, '\n');

const describeSubject = ({ subject, context }) => `${subject}${context ? ` (${context})` : ''}`;

const describeFinding = (finding) =>
  `${finding.level} ${finding.id} — ${describeSubject(finding)}: ${finding.detail} · ${SOURCES[finding.id]}`;

// The counting of scripts/lib/tsc-baseline.mjs takes a finding as file, code and message
const toCounted = (finding) => ({ file: finding.file, code: finding.id, message: describeSubject(finding) });

const sumCounts = (counts) => Object.values(counts).reduce((sum, count) => sum + count, 0);

const readBaseline = (root) => {
  const file = path.join(root, BASELINE_FILE);
  if (!fs.existsSync(file)) return null;
  let parsed;
  try {
    parsed = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (error) {
    throw new TokenConfigError(`${BASELINE_FILE} cannot be read: ${error.message}`);
  }
  if (typeof parsed?.violations !== 'object' || parsed.violations === null) {
    throw new TokenConfigError(`${BASELINE_FILE} has no "violations" object`);
  }
  return parsed.violations;
};

const findViolations = async (config, root) => {
  const findings = checkTokens(await readContexts(config, root));
  return {
    violations: findings.filter(({ level }) => level === 'violation'),
    warnings: findings.filter(({ level }) => level === 'warning'),
  };
};

const judgeRules = async (config, root, showWarnings) => {
  const { violations, warnings } = await findViolations(config, root);
  const baseline = readBaseline(root) ?? {};
  const counted = violations.map(toCounted);
  const fresh = new Set(findNewErrors(counted, baseline).map((error) => counted.indexOf(error)));
  const known = violations.length - fresh.size;
  const fixed = sumCounts(baseline) - sumCounts(pruneBaseline(counted, baseline));
  const printed = violations.filter((_, index) => fresh.has(index));
  for (const finding of showWarnings ? [...printed, ...warnings] : printed) {
    process.stdout.write(`${describeFinding(finding)}\n`);
  }
  if (!showWarnings && warnings.length > 0) {
    process.stdout.write(`${warnings.length} warnings: pnpm tokens:check --warnings lists them\n`);
  }
  if (fixed > 0) {
    process.stdout.write(
      `${fixed} fixed design violations still counted in ${BASELINE_FILE}: run pnpm tokens:baseline\n`,
    );
  } else if (known > 0) {
    process.stdout.write(`${known} known design violations (${BASELINE_FILE})\n`);
  }
  if (fresh.size === 0) return 0;
  process.stderr.write(`${fresh.size} design violations${known > 0 ? ` past ${BASELINE_FILE}` : ''}\n`);
  return 1;
};

// The rule of tooling/debt/judge.mjs writes the freeze: a refusal prints to stderr, a mode with nothing to write
// prints its lines, otherwise the sorted write lands in the file with the line of a freeze. The lines of judgeDebt are
// dropped when it returns a write: both freezes keep the line `N known design violations`, so the sync line of
// judge.mjs ("findings added by sync") is not printed
const applyJudgement = (mode, counted, baseline, root) => {
  const { exitCode, lines, write } = judgeDebt({
    mode,
    findings: counted,
    baseline,
    kitFiles: readUntouchedKitFiles(root),
    labels: { file: BASELINE_FILE, freeze: 'tokens:baseline', noun: 'design violations' },
  });
  if (write === undefined) {
    (exitCode === 0 ? process.stdout : process.stderr).write(`${lines.join('\n')}\n`);
    return exitCode;
  }
  const sorted = Object.fromEntries(Object.entries(write).sort(([a], [b]) => (a < b ? -1 : 1)));
  fs.writeFileSync(path.join(root, BASELINE_FILE), `${JSON.stringify({ violations: sorted }, null, 2)}\n`);
  process.stdout.write(`${BASELINE_FILE}: ${sumCounts(sorted)} known design violations\n`);
  return exitCode;
};

const freezeViolations = async (config, root) => {
  const { violations } = await findViolations(config, root);
  return applyJudgement('baseline', violations.map(toCounted), readBaseline(root), root);
};

// A kit update brought the rules: while only kit files changed, every violation is the new rules' baseline
const syncViolations = async (config, root) => {
  const foreign = findForeignChanges(listChangedPaths(root), readKitPaths(root));
  if (foreign.length > 0) {
    process.stderr.write(`${[FOREIGN_CHANGES_LINE, ...foreign].join('\n')}\n`);
    return 1;
  }
  const baseline = readBaseline(root);
  const { violations } = await findViolations(config, root);
  return applyJudgement('sync', violations.map(toCounted), baseline, root);
};

const run = async ({ check, baseline, sync, warnings, root }) => {
  if (baseline && sync) {
    process.stderr.write('use --baseline or --sync, not both\n');
    return 1;
  }
  const config = readTokenConfig(root);
  if (!config) {
    process.stdout.write(`design tokens are off: no ${CONFIG_FILE}\n`);
    return 0;
  }
  if (baseline) return freezeViolations(config, root);
  if (sync) return syncViolations(config, root);
  const built = await buildTokens(config);
  const file = path.join(root, OUTPUT_FILE);
  if (!check) {
    fs.writeFileSync(file, built);
    process.stdout.write(`${OUTPUT_FILE} built\n`);
    return 0;
  }
  const current = fs.existsSync(file) ? normalizeLineEnds(fs.readFileSync(file, 'utf8')) : null;
  if (current !== normalizeLineEnds(built)) {
    process.stderr.write(`${OUTPUT_FILE} is stale: run pnpm tokens\n`);
    return 1;
  }
  return judgeRules(config, root, warnings);
};

const { values } = parseArgs({
  options: {
    check: { type: 'boolean', default: false },
    baseline: { type: 'boolean', default: false },
    sync: { type: 'boolean', default: false },
    warnings: { type: 'boolean', default: false },
    root: { type: 'string' },
  },
});

try {
  process.exitCode = await run({
    check: values.check,
    baseline: values.baseline,
    sync: values.sync,
    warnings: values.warnings,
    root: path.resolve(values.root ?? DEFAULT_ROOT),
  });
} catch (error) {
  // like tooling/debt/debt.mjs: a broken kit-lock.json or git state is a broken project, so its message and exit 1
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
