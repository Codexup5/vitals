import { revalidateLogic, useForm } from '@tanstack/react-form';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';

import { userDraftSchema, type User, type UserDraft } from '@/entities/user';
import { isApiError } from '@/shared/api/api-error';
import { getErrorMessage } from '@/shared/api/error-message';
import { clearServerError, focusFirstInvalidField, putServerErrors } from '@/shared/lib/form-errors';

import { toUserFormValues } from '../lib/user-form-values';

interface UserFormOptions {
  initialUser?: User;
  save: (draft: UserDraft) => Promise<User>;
}

interface SubmitMeta {
  formElement: HTMLFormElement | undefined;
}

const NO_SUBMIT_META: SubmitMeta = { formElement: undefined };

export const useUserForm = ({ initialUser, save }: UserFormOptions) => {
  const navigate = useNavigate();
  const [submitErrorText, setSubmitErrorText] = useState<string>();

  const form = useForm({
    defaultValues: toUserFormValues(initialUser),
    onSubmitMeta: NO_SUBMIT_META,
    validationLogic: revalidateLogic(),
    validators: { onDynamic: userDraftSchema },
    listeners: {
      onChange: ({ fieldApi }) => {
        clearServerError(fieldApi);
        setSubmitErrorText(undefined);
      },
    },
    onSubmitInvalid: ({ formApi, meta }) => {
      if (meta.formElement) focusFirstInvalidField(meta.formElement, formApi);
    },
    onSubmit: async ({ value, formApi, meta }) => {
      setSubmitErrorText(undefined);
      const draft = userDraftSchema.parse(value);
      let user: User;
      try {
        user = await save(draft);
      } catch (error) {
        // Catch only the mutation: MutationCache reports its unexpected errors, a parse or navigate failure goes up
        const matchedFields = isApiError(error) ? putServerErrors(formApi, error.fields) : 0;
        if (matchedFields === 0) setSubmitErrorText(getErrorMessage(error));
        else if (meta.formElement) focusFirstInvalidField(meta.formElement, formApi);
        return;
      }
      await navigate({ to: '/users/$userId', params: { userId: user.id } });
    },
  });

  return { form, submitErrorText };
};
