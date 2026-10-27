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
    // architecture.md §1: an entity is the contract with the back end, even with one consumer
    files: ['./src/entities/**'],
    rules: { 'fsd/insignificant-slice': 'off' },
  },
  {
    files: ['./src/shared/**'],
    rules: { 'fsd/public-api': 'off' },
  },
]);
