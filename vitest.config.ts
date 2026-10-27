import { existsSync } from 'node:fs';
import { defineConfig, mergeConfig } from 'vitest/config';

import viteConfig from './vite.config';

// an adopted project may keep its own test setup elsewhere: the kit setup file is listed only where it exists
const appSetup = './src/app/test/setup.ts';
const appSetupFiles = existsSync(new URL(appSetup, import.meta.url)) ? [appSetup] : [];

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      restoreMocks: true,
      projects: [
        '*/vitest.config.{ts,mjs}',
        {
          extends: true,
          test: {
            name: 'app',
            environment: 'jsdom',
            // the first scenario of a file renders the whole app cold: 4.5 s of the 5 s default on the PC runners
            testTimeout: 15_000,
            setupFiles: appSetupFiles,
            include: ['src/**/*.test.{ts,tsx}'],
          },
        },
        {
          extends: true,
          test: {
            name: 'tooling',
            environment: 'node',
            include: ['scripts/**/*.test.js', 'tooling/**/*.test.js'],
          },
        },
      ],
    },
  }),
);
