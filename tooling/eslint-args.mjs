const VERSION = /^(\d+)\.\d+\.\d+/;
const CONFIG_LOOKUP_FLAG = ['--flag', 'v10_config_lookup_from_file'];

export const eslintArgv = (version, args) => {
  const match = VERSION.exec(version);
  if (!match) {
    throw new Error(`eslint/package.json has the version "${version}", which is not a version (MAJOR.MINOR.PATCH)`);
  }
  return Number(match[1]) < 10 ? [...CONFIG_LOOKUP_FLAG, ...args] : [...args];
};

export const baselineRuns = (mode, hasSuppressions) => {
  const suppress = ['.', '--suppress-all'];
  const prune = ['.', '--prune-suppressions'];
  if (mode === 'sync') return hasSuppressions ? [suppress, prune] : [];
  return hasSuppressions ? [prune] : [suppress, prune];
};
