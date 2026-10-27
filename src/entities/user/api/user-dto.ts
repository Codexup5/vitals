import { z } from 'zod';

import { lenientArray } from '@/shared/api/lenient-array';

import { USER_ROLES } from '../config/user-roles';

export const userDtoSchema = z.object({
  id: z.string(),
  first_name: z.string(),
  last_name: z.string(),
  email: z.string(),
  role: z.enum(USER_ROLES),
  created_at: z.iso.datetime({ offset: true }).nullable().catch(null),
});

export const userDraftDtoSchema = userDtoSchema.pick({ first_name: true, last_name: true, email: true, role: true });

export const userPageDtoSchema = z.object({
  items: lenientArray(userDtoSchema, 'userPageDto.items'),
  total: z.number().int().nonnegative(),
});

export type UserDto = z.infer<typeof userDtoSchema>;

export type UserPageDto = z.infer<typeof userPageDtoSchema>;

export type UserDraftDto = z.infer<typeof userDraftDtoSchema>;
