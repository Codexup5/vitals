import path from 'node:path';

// plain quotes: JSON.stringify doubles Windows backslashes past the length lint-staged chunks by
const toArgs = (files) => files.map((file) => `"${path.relative(import.meta.dirname, file)}"`).join(' ');

export default {
  '*.{ts,tsx,js,mjs}': (files) => [
    `node tooling/eslint.mjs --fix --max-warnings=0 --no-warn-ignored ${toArgs(files)}`,
    `prettier --write --ignore-unknown ${toArgs(files)}`,
  ],
  // the debt CLI judges the staged files against stylelint-baseline.json, so frozen debt does not block a commit
  '*.scss': (files) => [
    `node tooling/debt/debt.mjs stylelint --fix ${toArgs(files)}`,
    `prettier --write ${toArgs(files)}`,
  ],
  '*.{json,md,yml,yaml,html}': (files) => `prettier --write --ignore-unknown ${toArgs(files)}`,
};
