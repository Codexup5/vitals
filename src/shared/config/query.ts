const SECOND = 1000;
const MINUTE = 60 * SECOND;

export const STALE_POLICY = {
  live: 5 * SECOND,
  snapshot: MINUTE,
} as const;

export const MAX_QUERY_RETRIES = 2;
