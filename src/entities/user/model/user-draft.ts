import { z } from 'zod';

import { USER_TEXTS } from '../config/texts';
import { USER_EMAIL_MAX_LENGTH, USER_NAME_MAX_LENGTH } from '../config/user-limits';
import { USER_ROLES } from '../config/user-roles';

const { validation } = USER_TEXTS;

const nameSchema = z
  .string()
  .trim()
  .min(1, validation.required)
  .max(USER_NAME_MAX_LENGTH, validation.tooLong(USER_NAME_MAX_LENGTH));

export const userDraftSchema = z.object({
  firstName: nameSchema,
  lastName: nameSchema,
  email: z
    .string()
    .trim()
    .min(1, validation.required)
    .max(USER_EMAIL_MAX_LENGTH, validation.tooLong(USER_EMAIL_MAX_LENGTH))
    .pipe(z.email(validation.email)),
  role: z.enum(USER_ROLES),
});

export type UserDraftInput = z.input<typeof userDraftSchema>;

export type UserDraft = z.output<typeof userDraftSchema>;
