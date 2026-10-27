import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

const SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'ci-tests.mjs');

let folder;

// The CI runner sets GITHUB_OUTPUT; the child env drops it so a test never appends to the real file of the job.
const envWithoutGithubOutput = () => {
  const env = { ...process.env };
  delete env.GITHUB_OUTPUT;
  return env;
};

const run = (env) => spawnSync(process.execPath, [SCRIPT], { cwd: folder, env, encoding: 'utf8' });

// writes kit.json when the case has one, runs the script and returns what it printed
const printedMode = (kitText) => {
  if (kitText !== undefined) writeFileSync(path.join(folder, 'kit.json'), kitText);
  const result = run(envWithoutGithubOutput());

  expect(result.status).toBe(0);
  return result.stdout;
};

beforeEach(() => {
  folder = mkdtempSync(path.join(tmpdir(), 'ci-tests-'));
});

afterEach(() => {
  rmSync(folder, { recursive: true, force: true });
});

describe('ci-tests', () => {
  it('prints on with no kit.json', () => {
    expect(printedMode()).toBe('on\n');
  });

  it.each([
    ['a kit.json without the key', '{"gates":{}}'],
    ['a kit.json that does not parse', '{'],
    ['an unknown mode', '{"tests":"sometimes"}'],
    ['the mode on', '{"tests":"on"}'],
  ])('prints on for %s', (_, kitText) => {
    expect(printedMode(kitText)).toBe('on\n');
  });

  it.each([
    ['the mode later', '{"tests":"later"}', 'later\n'],
    ['the mode later after a BOM', '\uFEFF{"tests":"later"}', 'later\n'],
    ['the mode off', '{"tests":"off"}', 'off\n'],
  ])('prints the mode for %s', (_, kitText, expected) => {
    expect(printedMode(kitText)).toBe(expected);
  });

  it('appends tests=mode to the file GITHUB_OUTPUT names, after the lines it holds', () => {
    writeFileSync(path.join(folder, 'kit.json'), '{"tests":"later"}');
    const outputFile = path.join(folder, 'github-output');
    writeFileSync(outputFile, 'a=1\n');

    const result = run({ ...envWithoutGithubOutput(), GITHUB_OUTPUT: outputFile });

    expect(result.status).toBe(0);
    expect(result.stdout).toBe('later\n');
    expect(readFileSync(outputFile, 'utf8')).toBe('a=1\ntests=later\n');
  });

  it('writes no file when GITHUB_OUTPUT is absent', () => {
    writeFileSync(path.join(folder, 'kit.json'), '{"tests":"off"}');

    const result = run(envWithoutGithubOutput());

    expect(result.status).toBe(0);
    expect(readdirSync(folder)).toEqual(['kit.json']);
  });
});
