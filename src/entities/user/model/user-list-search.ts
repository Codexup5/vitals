import { z } from 'zod';

import { USER_SEARCH_MAX_LENGTH } from '../config/user-limits';
import { USER_ROLES } from '../config/user-roles';

export const USER_LIST_SEARCH_DEFAULTS = { page: 1, search: '' } as const;

export const userListSearchSchema = z.object({
  page: z.number().int().positive().default(USER_LIST_SEARCH_DEFAULTS.page).catch(USER_LIST_SEARCH_DEFAULTS.page),
  search: z
    .string()
    .trim()
    .max(USER_SEARCH_MAX_LENGTH)
    .default(USER_LIST_SEARCH_DEFAULTS.search)
    .catch(USER_LIST_SEARCH_DEFAULTS.search),
  role: z.enum(USER_ROLES).optional().catch(undefined),
});
