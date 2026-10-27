export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [2, 'always', ['feat', 'fix', 'refactor', 'perf', 'test', 'docs', 'chore', 'ci', 'build', 'revert']],
    'subject-case': [0],
    'header-max-length': [2, 'always', 72],
    'body-max-line-length': [0],
  },
};
