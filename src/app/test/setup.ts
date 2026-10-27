import '@testing-library/jest-dom/vitest';

import { cleanup, configure } from '@testing-library/react';
import { afterAll, afterEach, beforeAll } from 'vitest';

import { server } from '../../../mocks/node';
import { userDb } from '../../../mocks/user-db';

// The first visit to a lazy route compiles its chunk; under a parallel full run that alone passes the 1 s findBy default
configure({ asyncUtilTimeout: 5000 });

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));

afterEach(() => {
  cleanup();
  server.resetHandlers();
  userDb.reset();
});

afterAll(() => server.close());
