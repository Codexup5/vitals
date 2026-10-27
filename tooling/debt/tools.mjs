import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import stylelint from 'stylelint';

import { toProjectPath } from '../../scripts/lib/tsc-baseline.mjs';
import { auditPlacement } from '../placement.mjs';

const STYLE_FILES = ['src/**/*.scss'];

const toPosix = (file) => file.replaceAll('\\', '/');

const findingsOrder = (left, right) => {
  const leftKey = `${left.file}|${left.code}|${left.message}`;
  const rightKey = `${right.file}|${right.code}|${right.message}`;
  return leftKey < rightKey ? -1 : leftKey > rightKey ? 1 : 0;
};

/**
 * @param {string} file path from the root, with `/`
 * @param {string[] | undefined} given paths from the root, with `/`; no filter when undefined
 * @returns {boolean} the path is one of the given paths, inside one, or holds one
 */
export const isInScope = (file, given) =>
  given === undefined ||
  given.some((item) => file === item || file.startsWith(`${item}/`) || item.startsWith(`${file}/`));

/**
 * @param {string} root project root
 * @param {string[]} files paths from the cwd or absolute
 * @returns {string[]} paths from the root, with `/`
 */
export const toRootPaths = (root, files) => files.map((file) => toPosix(path.relative(root, path.resolve(file))));

const cutRuleSuffix = (text, rule) => (text.endsWith(` (${rule})`) ? text.slice(0, -(rule.length + 3)) : text);

/**
 * @param {string} root project root
 * @param {{ files?: string[], fix?: boolean }} [options] paths from the root, with `/`
 * @returns {Promise<{ file: string, code: string, message: string }[]>} the errors of stylelint
 */
export const findStylelintFindings = async (root, { files, fix = false } = {}) => {
  const { results } = await stylelint.lint({
    cwd: root,
    files: files?.map((file) => toPosix(path.join(root, file))) ?? STYLE_FILES,
    fix,
    allowEmptyInput: true,
  });
  const findings = [];
  for (const result of results) {
    const file = toProjectPath(result.source ?? '', root);
    const broken = [
      ...result.invalidOptionWarnings.map(({ text }) => text),
      ...result.parseErrors.map(({ text }) => text),
    ];
    if (broken.length > 0) throw new Error(`stylelint cannot judge ${file}: ${broken.join('; ')}`);
    for (const warning of result.warnings) {
      if (warning.severity !== 'error') continue;
      findings.push({ file, code: warning.rule, message: cutRuleSuffix(warning.text, warning.rule) });
    }
  }
  return findings.sort(findingsOrder);
};

const findPackageDir = (name, from) => {
  for (let directory = from; ; directory = path.dirname(directory)) {
    const candidate = path.join(directory, 'node_modules', name);
    if (fs.existsSync(path.join(candidate, 'package.json'))) return candidate;
    if (path.dirname(directory) === directory) throw new Error(`${name} is not installed from ${from}`);
  }
};

const steigerBin = (root) => {
  const dir = findPackageDir('steiger', root);
  const { bin } = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
  return path.join(dir, typeof bin === 'string' ? bin : bin.steiger);
};

const parseSteigerOutput = (output) => {
  let diagnostics;
  try {
    diagnostics = JSON.parse(output);
  } catch (error) {
    throw new Error(`steiger printed no JSON: ${error.message}\n${output.slice(0, 2000)}`, { cause: error });
  }
  if (!Array.isArray(diagnostics)) throw new Error(`steiger printed no JSON array:\n${output.slice(0, 2000)}`);
  return diagnostics;
};

/**
 * @param {string} root project root
 * @param {{ files?: string[] }} [options] paths from the root, with `/`
 * @returns {{ file: string, code: string, message: string }[]} the errors of steiger
 */
export const findSteigerFindings = (root, { files } = {}) => {
  const result = spawnSync(process.execPath, [steigerBin(root), './src', '--reporter', 'json'], {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  });
  if (result.error) throw new Error(`steiger did not run: ${result.error.message}`, { cause: result.error });
  const diagnostics = parseSteigerOutput(result.stdout);
  return diagnostics
    .filter(({ severity }) => severity === 'error')
    .map(({ location, ruleName, message }) => ({
      file: toProjectPath(location.path, root),
      code: ruleName,
      message,
    }))
    .filter(({ file }) => isInScope(file, files))
    .sort(findingsOrder);
};

/**
 * @param {string} root project root
 * @param {{ files?: string[] }} [options] paths from the root, with `/`
 * @returns {{ file: string, code: string, message: string }[]} the placement violations
 */
export const findPlacementFindings = (root, { files } = {}) =>
  auditPlacement(root, files)
    .map(({ file, rule, message }) => ({ file, code: rule, message: message.split('\n')[0] }))
    .sort(findingsOrder);

const KNIP_NON_ISSUE_KEYS = new Set(['file', 'owners']);

const describeKnipItem = (item) => {
  if (Array.isArray(item)) return item.map(({ name }) => name).join(', ');
  return item.namespace === undefined ? item.name : `${item.namespace}.${item.name}`;
};

/**
 * @param {string} output the stdout text of `knip --reporter json`
 * @returns {{ file: string, code: string, message: string }[]} the findings of the report, sorted
 */
export const parseKnipReport = (output) => {
  let report;
  try {
    report = JSON.parse(output);
  } catch (error) {
    throw new Error(`knip printed no JSON: ${error.message}\n${output.slice(0, 2000)}`, { cause: error });
  }
  if (!Array.isArray(report?.issues)) throw new Error(`knip printed no issues array:\n${output.slice(0, 2000)}`);
  const findings = [];
  for (const issue of report.issues) {
    const file = toPosix(issue.file);
    for (const [code, items] of Object.entries(issue)) {
      if (KNIP_NON_ISSUE_KEYS.has(code) || !Array.isArray(items)) continue;
      for (const item of items) findings.push({ file, code, message: describeKnipItem(item) });
    }
  }
  return findings.sort(findingsOrder);
};

/**
 * @param {string} root project root
 * @param {{ files?: string[] }} [options] paths from the root, with `/`
 * @returns {{ file: string, code: string, message: string }[]} the findings of knip
 */
export const findKnipFindings = (root, { files } = {}) => {
  const dir = findPackageDir('knip', root);
  const { bin } = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
  const args = [
    path.join(dir, typeof bin === 'string' ? bin : bin.knip),
    '--reporter',
    'json',
    '--no-progress',
    '--no-config-hints',
    '--no-exit-code',
  ];
  const result = spawnSync(process.execPath, args, { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (result.error) throw new Error(`knip did not run: ${result.error.message}`, { cause: result.error });
  if (result.status !== 0) {
    throw new Error(`knip did not run: exit ${result.status}\n${(result.stderr || result.stdout).slice(0, 2000)}`);
  }
  return parseKnipReport(result.stdout).filter(({ file }) => isInScope(file, files));
};
