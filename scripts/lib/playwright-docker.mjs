import fs from 'node:fs';
import path from 'node:path';

export const PLAYWRIGHT_IMAGE = 'mcr.microsoft.com/playwright:v1.63.0-noble';
const PLAYWRIGHT_VERSION = /:v(.+)-noble$/.exec(PLAYWRIGHT_IMAGE)[1];

const STORE_VOLUME = 'kit-playwright-pnpm-store';
const PASSED_ENV = ['CI', 'E2E_BASE_URL', 'E2E_HEADERS'];
const COPY_EXCLUDES = ['node_modules', '.git', '.sandbox', 'dist', 'test-results', '__screenshots__'];
const FLAGS = new Set(['--visual', '--update', '--dry-run']);

// the version check reads the installed package and stops the run when the lockfile moved ahead of the image
const VERSION_CHECK = [
  'const v = require("@playwright/test/package.json").version;',
  'if (v !== process.env.PLAYWRIGHT_VERSION) {',
  'console.error("playwright-docker: the image carries Playwright " + process.env.PLAYWRIGHT_VERSION +',
  '" but the lockfile installs " + v + ": bump PLAYWRIGHT_IMAGE in scripts/lib/playwright-docker.mjs");',
  'process.exit(1);',
  '}',
].join(' ');

const SCRIPT = [
  'set -eu',
  'PACK_DIR=/work',
  '[ "$PACK_PATH" = "." ] || PACK_DIR="/work/$PACK_PATH"',
  `trap 'if [ -n "\${HOST_UID:-}" ]; then chown -R "$HOST_UID:\${HOST_GID:-$HOST_UID}" "$PACK_DIR/e2e/__screenshots__" "$PACK_DIR/test-results"; fi' EXIT`,
  `tar -C /src ${COPY_EXCLUDES.map((name) => `--exclude=${name}`).join(' ')} -cf - . | tar -C /work -xf -`,
  'cd /work',
  'if [ "$PACK_PATH" = "." ]; then corepack pnpm install --frozen-lockfile; else corepack pnpm install --frozen-lockfile --filter "./$PACK_PATH"; fi',
  'cd "$PACK_DIR"',
  `node -e '${VERSION_CHECK}'`,
  'node node_modules/@playwright/test/cli.js test "$@"',
].join('\n');

export const parseArgs = (argv) => {
  const options = { visual: false, update: false, dryRun: false, passArgs: [] };
  const separator = argv.indexOf('--');
  const own = separator === -1 ? argv : argv.slice(0, separator);
  for (const arg of own) {
    if (!FLAGS.has(arg)) throw new Error(`playwright-docker: unknown flag ${arg}`);
    if (arg === '--visual') options.visual = true;
    if (arg === '--update') options.update = true;
    if (arg === '--dry-run') options.dryRun = true;
  }
  if (separator !== -1) options.passArgs = argv.slice(separator + 1);
  return options;
};

export const findWorkspaceRoot = (packRoot, exists = fs.existsSync) => {
  let current = path.resolve(packRoot);
  for (;;) {
    if (exists(path.join(current, 'pnpm-lock.yaml'))) return current;
    const parent = path.dirname(current);
    if (parent === current) throw new Error(`playwright-docker: no pnpm-lock.yaml at or above ${packRoot}`);
    current = parent;
  }
};

export const packPathIn = (workspaceRoot, packRoot) => {
  const relative = path.relative(workspaceRoot, packRoot).split(path.sep).join('/');
  return relative === '' ? '.' : relative;
};

export const buildDockerArgs = ({
  workspaceRoot,
  packPath,
  visual = false,
  update = false,
  passArgs = [],
  env = {},
  uid,
  gid,
}) => {
  const inContainer = packPath === '.' ? '/work' : `/work/${packPath}`;
  const onHost = packPath === '.' ? workspaceRoot : path.join(workspaceRoot, ...packPath.split('/'));
  const args = ['run', '--rm', '--init', '--ipc=host'];
  args.push('-v', `${workspaceRoot}:/src:ro`);
  for (const folder of ['e2e/__screenshots__', 'test-results']) {
    args.push('-v', `${path.join(onHost, ...folder.split('/'))}:${inContainer}/${folder}`);
  }
  args.push('-v', `${STORE_VOLUME}:/pnpm-store`);
  for (const name of PASSED_ENV) {
    if (env[name] !== undefined && env[name] !== '') args.push('-e', name);
  }
  args.push('-e', 'COREPACK_ENABLE_DOWNLOAD_PROMPT=0', '-e', 'pnpm_config_store_dir=/pnpm-store');
  args.push('-e', `PLAYWRIGHT_VERSION=${PLAYWRIGHT_VERSION}`, '-e', `PACK_PATH=${packPath}`);
  if (uid !== undefined) args.push('-e', `HOST_UID=${uid}`);
  if (gid !== undefined) args.push('-e', `HOST_GID=${gid}`);
  if (visual) args.push('-e', `E2E_VISUAL=${PLAYWRIGHT_IMAGE}`);
  args.push(PLAYWRIGHT_IMAGE, 'bash', '-c', SCRIPT, 'playwright-docker');
  if (update) args.push('--update-snapshots');
  if (visual) args.push('--pass-with-no-tests');
  args.push(...passArgs);
  return args;
};
