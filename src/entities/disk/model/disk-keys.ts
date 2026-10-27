export const DISK_KEYS = {
  all: () => ['disk'] as const,
  overview: () => [...DISK_KEYS.all(), 'overview'] as const,
  scanStatus: () => [...DISK_KEYS.all(), 'scan-status'] as const,
};
