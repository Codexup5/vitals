// The CLI of the content security policy: node tooling/security/check-csp.mjs [--root DIR].

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { findCspDefects, readCspConfig } from './csp.mjs';

const DEFAULT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

const describeError = (error) => (error instanceof Error ? error.message : String(error));

function resolveRoot(args) {
  const flag = args.indexOf('--root');
  if (flag === -1) return DEFAULT_ROOT;
  const value = args[flag + 1];
  if (!value) {
    process.stderr.write('check-csp: --root needs a folder\n');
    process.exit(2);
  }
  return path.resolve(value);
}

const readText = (file) => (existsSync(file) ? readFileSync(file, 'utf8') : null);

// The parsed vercel.json, null when absent, or a finding when it is not JSON.
function readVercel(root) {
  const text = readText(path.join(root, 'vercel.json'));
  if (text === null) return { vercel: null, problems: [] };
  try {
    return { vercel: JSON.parse(text), problems: [] };
  } catch (error) {
    const detail = `vercel.json is not valid JSON: ${describeError(error)}`;
    return { vercel: null, problems: [{ id: 'csp/hosting', file: 'vercel.json', detail }] };
  }
}

function report(findings) {
  for (const defect of findings) {
    process.stdout.write(`violation ${defect.id} — ${defect.file}: ${defect.detail}\n`);
  }
  process.stdout.write(`CSP violations: ${findings.length}\n`);
  process.exitCode = 1;
}

function main() {
  const root = resolveRoot(process.argv.slice(2));
  let config;
  try {
    config = readCspConfig(root);
  } catch (error) {
    report([{ id: 'csp/config', file: 'csp.json', detail: describeError(error) }]);
    return;
  }
  if (config === null) {
    process.stdout.write('content security policy is off: no csp.json\n');
    return;
  }

  const { vercel, problems } = readVercel(root);
  const findings = [
    ...findCspDefects({ config, indexHtml: readText(path.join(root, 'index.html')), vercel }),
    ...problems,
  ];
  if (findings.length > 0) {
    report(findings);
    return;
  }
  process.stdout.write(`content security policy: ${config.mode}, no violation\n`);
}

main();
