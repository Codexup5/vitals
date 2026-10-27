#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const SLICE_LAYERS = ['pages', 'features', 'entities'];
export const SEGMENTS = ['ui', 'model', 'lib', 'config', 'api'];
const KEBAB_CASE = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const PASCAL_CASE = /^[A-Z][A-Za-z0-9]*$/;
const KEBAB_TS_FILE = /^[a-z0-9]+(-[a-z0-9]+)*(\.d)?\.ts$/;

const NO_CHILDREN = () => [];

const TOOL_OUTPUT = ['__snapshots__'];

const isOwnEntry = (name) => !name.startsWith('.') && !TOOL_OUTPUT.includes(name);

const listFolders = (directory) =>
  fs.statSync(directory, { throwIfNoEntry: false })?.isDirectory()
    ? fs
        .readdirSync(directory, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name)
    : [];

/**
 * @param {string} relative file path from the project root, with `/` separators
 * @param {(candidate: string) => boolean} exists checks that a file exists at a path from the project root
 * @param {(directory: string) => string[]} children names of the folders inside a folder from the project root
 * @returns {{ rule: string, message: string } | null}
 */
export const checkPlacement = (relative, exists, children = NO_CHILDREN) => {
  if (!relative.startsWith('src/')) return null;

  const all = relative.split('/');
  const [, layer, folder, child] = all;
  const isGroupSlice = (name) =>
    !SEGMENTS.includes(name) &&
    (exists(`src/pages/${folder}/${name}/index.ts`) || !exists(`src/pages/${folder}/index.ts`));
  const isGroupMember = layer === 'pages' && all.length > 4 && isGroupSlice(child);
  const groupSlice =
    layer === 'pages' && all.length <= 5 && !isGroupMember
      ? children(`src/pages/${folder}`).find(isGroupSlice)
      : undefined;
  const parts = isGroupMember ? [all[0], layer, `${folder}/${child}`, ...all.slice(4)] : all;
  const [, , slice, segment] = parts;
  const fileName = parts.at(-1) ?? '';
  const directories = all.slice(1, -1);
  const isTest = /\.test\.tsx?$/.test(fileName);

  const deny = (rule, reason, hint) => ({
    rule,
    message: `File is in the wrong place: ${relative}\n${reason}\n→ ${hint}`,
  });

  if (layer === 'widgets') {
    return deny(
      'widgets',
      'The widgets layer is not used (architecture.md §1).',
      'A block of one page goes to pages/<page>/ui, an action for 2+ pages goes to features.',
    );
  }

  if (layer === 'shared' && fileName === 'index.ts') {
    return deny(
      'shared-barrel',
      'A shared folder has no public API: import the file directly, such as @/shared/api/http-client',
      'Delete the barrel and import each file by its own path.',
    );
  }

  const badFolder = directories.find((directory) => !KEBAB_CASE.test(directory));
  if (badFolder) {
    return deny(
      'kebab-folder',
      `Folder "${badFolder}" is not kebab-case.`,
      'Folders and slices are kebab-case: user-list, invite-user.',
    );
  }

  if (fileName.endsWith('.tsx') && !isTest && layer !== 'app') {
    const component = fileName.replace(/\.tsx$/, '');
    if (!PASCAL_CASE.test(component)) {
      return deny(
        'pascal-component',
        'A component file name is PascalCase.',
        `${parts.slice(0, -1).join('/')}/${component.charAt(0).toUpperCase()}${component.slice(1)}.tsx`,
      );
    }
  }

  if (fileName.endsWith('.ts') && !isTest && !KEBAB_TS_FILE.test(fileName)) {
    return deny('kebab-file', 'A file name is kebab-case.', 'user-mapper.ts, use-user-list-page.ts');
  }

  if (fileName.endsWith('.module.scss')) {
    const componentName = fileName.replace('.module.scss', '.tsx');
    const component = [...parts.slice(0, -1), componentName].join('/');
    if (!exists(component)) {
      return deny(
        'styles-near-component',
        'Styles live next to their component.',
        `Create ${componentName} in this folder first.`,
      );
    }
  }

  if (!SLICE_LAYERS.includes(layer)) return null;

  if (groupSlice !== undefined && fileName === 'index.ts' && all.length === 4) {
    return deny(
      'group-index',
      `A slice group has no public API: export from src/pages/${folder}/${groupSlice}/index.ts`,
      `Delete src/pages/${folder}/index.ts and import each slice of the group by its own path.`,
    );
  }

  if (groupSlice !== undefined) {
    return deny(
      'segment',
      `Folder src/pages/${folder} is a slice group: it holds slices, not segments.`,
      `Move the file into a slice of the group, such as src/pages/${folder}/${groupSlice}/${child}/${fileName}.`,
    );
  }

  if (parts.length === 4 && fileName !== 'index.ts') {
    return deny(
      'slice-root',
      'Only index.ts lives in the slice root.',
      `src/${layer}/${slice}/{${SEGMENTS.join('|')}}/${fileName}`,
    );
  }

  if (fileName === 'index.ts' && parts.length > 4) {
    return deny(
      'nested-index',
      'index.ts inside a slice is banned: there is one public API, in the slice root.',
      `Export from src/${layer}/${slice}/index.ts or import the file directly.`,
    );
  }

  if (parts.length > 4 && !SEGMENTS.includes(segment)) {
    return deny('segment', `Unknown segment "${segment}".`, `Segments: ${SEGMENTS.join(', ')}.`);
  }

  if (segment === 'api' && layer !== 'entities') {
    return deny(
      'api-outside-entity',
      'Transport and DTOs live only in an entity.',
      `src/entities/<domain>/api/${fileName}`,
    );
  }

  if (fileName.endsWith('.tsx') && !isTest && segment !== 'ui') {
    return deny('component-outside-ui', 'Components live only in ui/.', `src/${layer}/${slice}/ui/${fileName}`);
  }

  if (/^use-.+\.ts$/.test(fileName) && segment !== 'model') {
    return deny(
      'hook-outside-model',
      'A hook is logic, it belongs in model/.',
      `src/${layer}/${slice}/model/${fileName}`,
    );
  }

  return null;
};

const byPath = (left, right) => (left < right ? -1 : left > right ? 1 : 0);

const walkSource = (root) => {
  const files = [];
  const walk = (directory) => {
    for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
      if (!isOwnEntry(entry.name)) continue;
      const entryPath = `${directory}/${entry.name}`;
      if (entry.isDirectory()) walk(entryPath);
      else files.push(entryPath);
    }
  };
  if (fs.existsSync(path.join(root, 'src'))) walk('src');
  return files;
};

/**
 * @param {string} root project root that holds `src/`
 * @returns {string[]} the files of `src/` that git tracks or would track, sorted; the walk outside a git work tree
 */
export const listProjectFiles = (root) => {
  const result = spawnSync('git', ['ls-files', '-co', '--exclude-standard', '-z', '--', 'src'], {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  });
  if (result.error || result.status !== 0) return walkSource(root).sort(byPath);
  return result.stdout
    .split('\0')
    .filter((file) => file !== '' && file.split('/').every(isOwnEntry) && fs.existsSync(path.join(root, file)))
    .sort(byPath);
};

/**
 * @param {string} root project root that holds `src/`
 * @param {string[]} [only] files from the root, `/` separated; every file of `src/` by default
 * @returns {{ file: string, rule: string, message: string }[]} violations sorted by file
 */
export const auditPlacement = (root, only) => {
  const files = only ?? listProjectFiles(root);
  const exists = (candidate) => fs.existsSync(path.join(root, candidate));
  const children = (directory) => listFolders(path.join(root, directory));
  const violations = [];
  for (const file of files) {
    const violation = checkPlacement(file, exists, children);
    if (violation) violations.push({ file, ...violation });
  }
  return violations.sort((left, right) => (left.file < right.file ? -1 : left.file > right.file ? 1 : 0));
};

// Node runs the module by its real path: compare against it, otherwise the CLI stays silent in a symlinked project
if (process.argv[1] && fs.realpathSync(process.argv[1]) === import.meta.filename) {
  const target = process.argv[2];
  if (target) {
    const violation = checkPlacement(
      target.split('\\').join('/'),
      (candidate) => fs.existsSync(candidate),
      listFolders,
    );
    if (violation) {
      process.stderr.write(`${violation.message}\n`);
      process.exitCode = 1;
    }
  }
}
