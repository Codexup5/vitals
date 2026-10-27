import { spawnSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

const SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'ci-port.mjs');

let folder;

// The CI runner sets GITHUB_ENV; the child env drops it so a test never appends to the real file of the job.
const envWithoutGithubEnv = () => {
  const env = { ...process.env };
  delete env.GITHUB_ENV;
  return env;
};

const run = (env) => spawnSync(process.execPath, [SCRIPT], { cwd: folder, env, encoding: 'utf8' });

const portOf = (stdout) => {
  const match = /^E2E_PORT=(\d+)\n$/.exec(stdout);
  expect(match).not.toBeNull();
  return Number(match[1]);
};

beforeEach(() => {
  folder = mkdtempSync(path.join(tmpdir(), 'ci-port-'));
});

afterEach(() => {
  rmSync(folder, { recursive: true, force: true });
});

describe('ci-port', () => {
  it('prints exactly one line E2E_PORT=n with a port from 1024 to 65535', () => {
    const result = run(envWithoutGithubEnv());

    expect(result.status).toBe(0);
    const port = portOf(result.stdout);
    expect(port).toBeGreaterThanOrEqual(1024);
    expect(port).toBeLessThanOrEqual(65535);
  });

  it('appends the same port to the file GITHUB_ENV names, after the lines it holds', () => {
    const envFile = path.join(folder, 'github-env');
    writeFileSync(envFile, 'FIRST=1\n');

    const result = run({ ...envWithoutGithubEnv(), GITHUB_ENV: envFile });

    expect(result.status).toBe(0);
    const port = portOf(result.stdout);
    expect(readFileSync(envFile, 'utf8')).toBe(`FIRST=1\nE2E_PORT=${port}\n`);
  });

  it('writes no file when GITHUB_ENV is absent', () => {
    const result = run(envWithoutGithubEnv());

    expect(result.status).toBe(0);
    expect(readdirSync(folder)).toEqual([]);
  });

  it('leaves the printed port free: a net server listens on it right after the script exits', async () => {
    const port = portOf(run(envWithoutGithubEnv()).stdout);
    const server = createServer();

    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(port, resolve);
    });
    await new Promise((resolve) => server.close(resolve));
  });
});
