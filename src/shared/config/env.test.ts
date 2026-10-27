import { describe, expect, it } from 'vitest';

import { parseEnv } from './env';

const VALID_SOURCE = {
  VITE_API_URL: '/api',
  VITE_API_MOCKS: 'true',
  VITE_SENTRY_DSN: 'https://key@o0.ingest.sentry.io/0',
  VITE_RELEASE: 'abc123',
  MODE: 'production',
};

describe('parseEnv', () => {
  it('maps a valid config without issues', () => {
    expect(parseEnv(VALID_SOURCE)).toEqual({
      env: {
        apiUrl: '/api',
        shouldMockApi: true,
        sentryDsn: 'https://key@o0.ingest.sentry.io/0',
        release: 'abc123',
        mode: 'production',
      },
      issues: null,
    });
  });

  it('treats a missing or empty release as unknown', () => {
    expect(parseEnv({ ...VALID_SOURCE, VITE_RELEASE: undefined }).env.release).toBeUndefined();
    expect(parseEnv({ ...VALID_SOURCE, VITE_RELEASE: '' }).env.release).toBeUndefined();
  });

  it('treats an empty sentry dsn as missing', () => {
    const { env, issues } = parseEnv({ ...VALID_SOURCE, VITE_SENTRY_DSN: '' });

    expect(issues).toBeNull();
    expect(env.sentryDsn).toBeUndefined();
  });

  it('reports a missing api url and returns an inert env', () => {
    const { env, issues } = parseEnv({ ...VALID_SOURCE, VITE_API_URL: undefined });

    expect(issues).toContain('VITE_API_URL');
    expect(env.shouldMockApi).toBe(false);
    expect(env.sentryDsn).toBeUndefined();
  });
});
