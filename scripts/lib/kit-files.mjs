import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

export const FOREIGN_CHANGES_LINE =
  'debt:sync freezes only the debt a kit update brings: commit your own changes first';
export const KIT_FILES_LINE = 'fix the kit files or resolve the .kit-new files first';

const ALLOWED_CHANGES = new Set([
  'CLAUDE.md',
  'kit-lock.json',
  'package.json',
  'pnpm-lock.yaml',
  'pnpm-workspace.yaml',
  '.gitignore',
  '.claude/settings.json',
  'docs/context/public-api.md',
  'docs/canon.md',
  'tsc-baseline.json',
  'eslint-suppressions.json',
  'stylelint-baseline.json',
  'steiger-baseline.json',
  'placement-baseline.json',
  'knip-baseline.json',
  'design-baseline.json',
]);

const toPosix = (file) => file.replaceAll('\\', '/');

// kit-lock.json of the folder: none when the file is missing; one Error for both readers when it is no JSON
const readLock = (root) => {
  const lockPath = path.join(root, 'kit-lock.json');
  if (!fs.existsSync(lockPath)) return {};
  try {
    return JSON.parse(fs.readFileSync(lockPath, 'utf8'));
  } catch (error) {
    throw new Error(`kit-lock.json cannot be read: ${error.message}`, { cause: error });
  }
};

// the files keys of the kit-lock.json that HEAD commits, so the files an update removed from the lock stay kit files:
// none when git has no HEAD lock, the folder is no repository or the committed lock is no JSON
const readHeadLockFiles = (root) => {
  const result = spawnSync('git', ['show', 'HEAD:./kit-lock.json'], {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  });
  if (result.error || result.status !== 0) return [];
  try {
    return Object.keys(JSON.parse(result.stdout).files ?? {});
  } catch (error) {
    if (error instanceof SyntaxError) return [];
    throw error;
  }
};

export const readKitPaths = (root) => {
  const lock = readLock(root);
  return {
    files: new Set([...Object.keys(lock.files ?? {}), ...readHeadLockFiles(root)]),
    seeds: new Set(Object.keys(lock.seeds ?? {})),
  };
};

// the hash of create-kit/lib/lock.mjs: sha256 of the bytes with CRLF read as LF; create-kit/lib/lock.test.js ties the
// two, so a change here goes into lock.mjs in the same commit
export const hashContent = (bytes) =>
  createHash('sha256')
    .update(Buffer.from(bytes.toString('latin1').replaceAll('\r\n', '\n'), 'latin1'))
    .digest('hex');

const hashFileOrNull = (file) => {
  try {
    return hashContent(fs.readFileSync(file));
  } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'EISDIR') return null;
    throw error;
  }
};

// a path of "files" in kit-lock.json whose file still holds the text the kit wrote, or that has its PATH.kit-new beside
// it (the kit text waits there, whatever the file holds or when it is deleted): a file the project kept as its own or
// merged, with no .kit-new pending, is its code, and its debt is frozen like any other
export const readUntouchedKitFiles = (root) => {
  const untouched = new Set();
  for (const [file, hash] of Object.entries(readLock(root).files ?? {})) {
    if (hashFileOrNull(path.join(root, file)) === hash || fs.existsSync(path.join(root, `${file}.kit-new`))) {
      untouched.add(file);
    }
  }
  return untouched;
};

export const findKitFindings = (findings, kitFiles) => findings.filter((finding) => kitFiles.has(finding.file));

const runGit = (root, args) => {
  const result = spawnSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (result.error || result.status !== 0) {
    throw new Error(`git ${args[0]} failed in ${root}: ${result.error?.message ?? result.stderr.trim()}`);
  }
  return result.stdout;
};

export const listChangedPaths = (root) => {
  const prefix = toPosix(runGit(root, ['rev-parse', '--show-prefix']).trim());
  const entries = runGit(root, ['status', '--porcelain=v1', '-z', '--untracked-files=all', '--', '.']).split('\0');
  const changed = [];
  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index];
    if (entry === '') continue;
    const file = toPosix(entry.slice(3));
    if (/[RC]/.test(entry.slice(0, 2))) index += 1;
    changed.push(file.startsWith(prefix) ? file.slice(prefix.length) : file);
  }
  return changed;
};

export const findForeignChanges = (changed, kitPaths) =>
  changed.filter((file) => {
    const posix = toPosix(file);
    return !(
      kitPaths.files.has(posix) ||
      kitPaths.seeds.has(posix) ||
      ALLOWED_CHANGES.has(posix) ||
      posix.endsWith('.kit-new')
    );
  });
