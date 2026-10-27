const SECOND = 1000;
const MINUTE = 60 * SECOND;

export const STALE_POLICY = {
  detail: 30 * SECOND,
  list: MINUTE,
  reference: 5 * MINUTE,
} as const;

export const MAX_QUERY_RETRIES = 2;

export const REQUEST_TIMEOUT_MS = 15 * SECOND;
