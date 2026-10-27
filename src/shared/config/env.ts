import { z } from 'zod';

// zod probes eval with Function('') when a z.object is built, and script-src without 'unsafe-eval' reports it as a CSP
// violation; the switch sits here because the bundler places env.ts in the shared chunk ahead of every schema, which
// a side-effect import in main.tsx is not (it runs after the shared chunk). The prod CSP e2e fails if that changes
z.config({ jitless: true });

const envSchema = z.object({
  VITE_API_URL: z.string().min(1),
  VITE_API_MOCKS: z
    .enum(['true', 'false'])
    .default('false')
    .transform((flag) => flag === 'true'),
  VITE_SENTRY_DSN: z.preprocess((value) => (value === '' ? undefined : value), z.url().optional()),
  VITE_RELEASE: z.preprocess((value) => (value === '' ? undefined : value), z.string().optional()),
  VITE_VITALS_URL: z.preprocess(
    (value) => (value === '' ? undefined : value),
    z.union([z.url(), z.string().startsWith('/')]).optional(),
  ),
  MODE: z.string(),
});

interface Env {
  apiUrl: string;
  shouldMockApi: boolean;
  sentryDsn: string | undefined;
  release: string | undefined;
  vitalsUrl: string | undefined;
  mode: string;
}

const INERT_ENV: Env = {
  apiUrl: '',
  shouldMockApi: false,
  sentryDsn: undefined,
  release: undefined,
  vitalsUrl: undefined,
  mode: '',
};

export const parseEnv = (source: Record<string, unknown>): { env: Env; issues: string | null } => {
  const result = envSchema.safeParse(source);
  if (!result.success) return { env: INERT_ENV, issues: z.prettifyError(result.error) };

  const parsed = result.data;
  return {
    env: {
      apiUrl: parsed.VITE_API_URL,
      shouldMockApi: parsed.VITE_API_MOCKS,
      sentryDsn: parsed.VITE_SENTRY_DSN,
      release: parsed.VITE_RELEASE,
      vitalsUrl: parsed.VITE_VITALS_URL,
      mode: parsed.MODE,
    },
    issues: null,
  };
};

const { env, issues } = parseEnv(import.meta.env);

export const ENV: Readonly<Env> = env;

export const ENV_ISSUES = issues;
