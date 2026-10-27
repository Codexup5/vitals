import { captureException } from '@sentry/react';

type ErrorContext = Readonly<Record<string, unknown>>;

export const reportError = (error: unknown, context: ErrorContext = {}) => {
  if (import.meta.env.DEV) console.error(error, context);
  captureException(error, { extra: context });
};
