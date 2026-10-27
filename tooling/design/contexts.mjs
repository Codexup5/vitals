import path from 'node:path';

import { createDictionary, createHooks } from './build.mjs';

const BASE_THEME = 'base';
const WINDOWS_LONG_PATH = /^\/\/\?\//;

// No value transforms: the checks read the DTCG values, with the aliases resolved
const PLATFORMS = { css: { transforms: [] } };

// Style Dictionary reports a token file as a glob match, which on Windows carries the //?/ long path prefix
const toRootRelative = (root, file) =>
  path.relative(root, file.replace(WINDOWS_LONG_PATH, '')).split(path.sep).join('/');

const isToken = (node) => typeof node === 'object' && node !== null && '$value' in node;

const flatten = (tree, root) =>
  Object.entries(tree).flatMap(([key, node]) => {
    if (key.startsWith('$') || typeof node !== 'object' || node === null) return [];
    if (!isToken(node)) return flatten(node, root);
    return [
      {
        path: node.path.join('.'),
        type: node.$type,
        value: node.$value,
        // the original keeps the aliases of `surfaces` as written
        kit: node.original.$extensions?.kit ?? {},
        file: toRootRelative(root, node.filePath),
      },
    ];
  });

// The base alone, or the base with one layer over it
const resolveTokens = async (config, root, base, layer) => {
  const dictionary = createDictionary({
    hooks: createHooks(config),
    include: layer ? [base.path] : [],
    source: [(layer ?? base).path],
    platforms: PLATFORMS,
  });
  const { tokens } = await dictionary.getPlatformTokens('css');
  return flatten(tokens, root);
};

export const readContexts = async (config, root) => {
  const [base, ...rest] = config.layers;
  const coarse = rest.find((layer) => layer.pointer === 'coarse');

  const baseTokens = await resolveTokens(config, root, base);
  const themeContexts = [{ name: base.theme ?? BASE_THEME, kind: 'theme', tokens: baseTokens }];
  for (const layer of rest.filter((candidate) => candidate.theme)) {
    const tokens = await resolveTokens(config, root, base, layer);
    themeContexts.push({ name: layer.theme, kind: 'theme', tokens });
  }
  const coarseTokens = coarse ? await resolveTokens(config, root, base, coarse) : baseTokens;
  return [
    ...themeContexts,
    { name: 'fine', kind: 'pointer', tokens: baseTokens },
    { name: 'coarse', kind: 'pointer', tokens: coarseTokens },
  ];
};
