#!/usr/bin/env node
import { appendFileSync, readFileSync } from 'node:fs';

const MODES = ['on', 'later', 'off'];

// null for no kit.json and for JSON that does not parse: both read as tests on; a BOM is skipped, as the hooks do
const readKit = () => {
  try {
    return JSON.parse(readFileSync('kit.json', 'utf8').replace(/^\uFEFF/u, ''));
  } catch (error) {
    if (error instanceof SyntaxError || error.code === 'ENOENT') return null;
    throw error;
  }
};

const kit = readKit();
const mode = MODES.includes(kit?.tests) ? kit.tests : 'on';
console.log(mode);
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `tests=${mode}\n`);
