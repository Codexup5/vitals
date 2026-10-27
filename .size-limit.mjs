import { existsSync, readFileSync } from 'node:fs';

const MANIFEST_PATH = 'dist/.vite/manifest.json';

if (!existsSync(MANIFEST_PATH)) {
  throw new Error(`${MANIFEST_PATH} not found: run pnpm build first`);
}

const manifest = JSON.parse(readFileSync(MANIFEST_PATH, 'utf8'));
const entryKey = Object.keys(manifest).find((key) => manifest[key].isEntry);

if (entryKey === undefined) {
  throw new Error(`${MANIFEST_PATH} has no chunk with isEntry`);
}

// startup chunks are the closure of the entry's static imports, exactly what Vite puts into modulepreload
const collectStaticChunks = (key, seen = new Set()) => {
  if (seen.has(key)) return seen;
  seen.add(key);
  for (const imported of manifest[key].imports ?? []) collectStaticChunks(imported, seen);
  return seen;
};

export default [
  {
    name: 'initial JS',
    path: [...collectStaticChunks(entryKey)].map((key) => `dist/${manifest[key].file}`),
    limit: '180 kB',
  },
];
