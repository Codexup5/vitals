#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  buildDockerArgs,
  findWorkspaceRoot,
  packPathIn,
  parseArgs,
  PLAYWRIGHT_IMAGE,
} from './lib/playwright-docker.mjs';

const PACK_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const fail = (message) => {
  process.stderr.write(`${message}\n`);
  process.exit(1);
};

const quote = (arg) => (/^[\w@%+=:,./-]+$/.test(arg) ? arg : `'${arg.replaceAll("'", `'\\''`)}'`);

let options;
let workspaceRoot;
try {
  options = parseArgs(process.argv.slice(2));
  workspaceRoot = findWorkspaceRoot(PACK_ROOT);
} catch (error) {
  fail(error.message);
}

const args = buildDockerArgs({
  workspaceRoot,
  packPath: packPathIn(workspaceRoot, PACK_ROOT),
  visual: options.visual,
  update: options.update,
  passArgs: options.passArgs,
  env: process.env,
  uid: process.getuid?.(),
  gid: process.getgid?.(),
});

if (options.dryRun) {
  process.stdout.write(`docker ${args.map(quote).join(' ')}\n`);
  process.exit(0);
}

for (const folder of ['e2e/__screenshots__', 'test-results']) {
  fs.mkdirSync(path.join(PACK_ROOT, ...folder.split('/')), { recursive: true });
}

process.stderr.write(`playwright-docker: ${PLAYWRIGHT_IMAGE}\n`);
const result = spawnSync('docker', args, { stdio: 'inherit' });
if (result.error) fail(`playwright-docker: docker did not run: ${result.error.message}`);
process.exit(result.status ?? 1);
