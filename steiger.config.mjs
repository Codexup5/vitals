import fsd from '@feature-sliced/steiger-plugin';
import { defineConfig } from 'steiger';

export default defineConfig([
  ...fsd.configs.recommended,
  {
    rules: {
      'fsd/no-public-api-sidestep': 'off',
    },
  },
  {
    files: ['./src/shared/**'],
    rules: { 'fsd/public-api': 'off' },
  },
]);
