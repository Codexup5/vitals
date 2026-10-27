import path, { posix } from 'node:path';

const normalize = (filename) => filename.replaceAll('\\', '/');

// Folders above the lint root do not count: a project may sit in a folder named src or api
export const projectPath = ({ filename, cwd }) => {
  const relative = normalize(path.relative(cwd, filename));
  return relative.startsWith('../') || path.isAbsolute(relative) ? normalize(filename) : `/${relative}`;
};

export const isInSegment = (filename, segment) => normalize(filename).includes(`/${segment}/`);

export const isEntityModel = (filename) => /\/src\/entities\/[^/]+\/model\//.test(normalize(filename));

export const isTestFile = (filename) => /\.test\.[jt]sx?$/.test(filename);

export const toSrcPath = (filename) => {
  const path = normalize(filename);
  const index = path.lastIndexOf('/src/');
  return index === -1 ? null : path.slice(index + '/src/'.length);
};

export const resolveFromSrc = (filename, specifier) => {
  if (specifier.startsWith('@/')) return posix.normalize(specifier.slice('@/'.length));
  const own = toSrcPath(filename);
  if (own === null || !(specifier.startsWith('./') || specifier.startsWith('../'))) return null;
  return posix.join(posix.dirname(own), specifier);
};
