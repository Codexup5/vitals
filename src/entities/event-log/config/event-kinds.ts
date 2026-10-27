/** Every kind of the event feed, in the order the filter shows them; `error` is a System error of no known kind. */
export const EVENT_KINDS = ['crash', 'bsod', 'whea', 'gpuReset', 'appCrash', 'nvContainer', 'iommu', 'error'] as const;

/** The kinds the crash chart stacks: the ones that mean the PC or a program fell over. Order fixes their colours. */
export const CHART_KINDS = ['crash', 'bsod', 'whea', 'gpuReset', 'appCrash'] as const;

export const HEALTH_RANGES = ['30d', '90d', 'months'] as const;

export const HEALTH_RANGE_DAYS = { '30d': 30, '90d': 90, months: 30 } as const;

export const EVENT_PAGE_SIZE = 50;
