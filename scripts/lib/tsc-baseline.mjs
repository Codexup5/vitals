const ERROR_LINE = /^(.+?)\(\d+,\d+\): error (TS\d+): (.*)$/;

const toKey = ({ file, code, message }) => `${file}|${code}|${message}`;

export const parseTscOutput = (text) =>
  text.split(/\r?\n/).flatMap((line) => {
    const match = ERROR_LINE.exec(line);
    if (!match) return [];
    const [, file, code, message] = match;
    return [{ file: file.replaceAll('\\', '/'), code, message }];
  });

export const toFingerprints = (errors) => {
  const counts = {};
  for (const error of errors) {
    const key = toKey(error);
    counts[key] = (counts[key] ?? 0) + 1;
  }
  return counts;
};

export const findNewErrors = (errors, baseline) => {
  const left = { ...baseline };
  return errors.filter((error) => {
    const key = toKey(error);
    const count = left[key] ?? 0;
    if (count > 0) {
      left[key] = count - 1;
      return false;
    }
    return true;
  });
};

export const pruneBaseline = (errors, baseline) => {
  const current = toFingerprints(errors);
  const pruned = {};
  for (const [key, count] of Object.entries(baseline)) {
    const lowered = Math.min(count, current[key] ?? 0);
    if (lowered > 0) pruned[key] = lowered;
  }
  return pruned;
};

const GLOBAL_ERROR = /^error TS\d+:/;
const SUMMARY_LINE = /^Found \d+ errors?\b/;

const TSCONFIG_NAME = /^tsconfig.*\.json$/;

const isConfigError = (line) => {
  const [error] = parseTscOutput(line);
  return error !== undefined && TSCONFIG_NAME.test(error.file.split('/').at(-1));
};

export const hasGlobalError = (text) =>
  text.split(/\r?\n/).some((line) => GLOBAL_ERROR.test(line) || isConfigError(line));

export const groupTscOutput = (text) => {
  const groups = [];
  const unrecognized = [];
  for (const line of text.split(/\r?\n/)) {
    const [error] = parseTscOutput(line);
    if (error) groups.push({ error, lines: [line] });
    else if (/^\s+\S/.test(line)) groups.at(-1)?.lines.push(line);
    else if (line.trim() !== '' && !SUMMARY_LINE.test(line) && !GLOBAL_ERROR.test(line)) unrecognized.push(line);
  }
  return { groups, unrecognized };
};

export const isCrash = (status, unrecognized, signal) => signal != null || (status !== 0 && unrecognized.length > 0);

export const toProjectPath = (file, root) => {
  const posixFile = file.replaceAll('\\', '/');
  const posixRoot = root.replaceAll('\\', '/').replace(/\/+$/, '');
  const isDrivePath = /^[a-z]:\//i.test(posixFile);
  const inside = isDrivePath
    ? posixFile.toLowerCase().startsWith(`${posixRoot.toLowerCase()}/`)
    : posixFile.startsWith(`${posixRoot}/`);
  return inside ? posixFile.slice(posixRoot.length + 1) : posixFile;
};

const byName = (a, b) => (a < b ? -1 : 1);

export const listProjects = ({ rootConfigs, referenced, extended, folders }) => {
  const rootProjects = rootConfigs
    .filter((name) => name !== 'tsconfig.json' && !referenced.includes(name) && !extended.includes(name))
    .sort(byName);
  const nested = folders
    .filter(({ name, hasTsconfig }) => hasTsconfig && name !== 'node_modules' && !name.startsWith('.'))
    .map(({ name }) => `${name}/tsconfig.json`)
    .filter((project) => !referenced.includes(project))
    .sort(byName);
  return [...(rootConfigs.includes('tsconfig.json') ? ['tsconfig.json'] : []), ...rootProjects, ...nested];
};
