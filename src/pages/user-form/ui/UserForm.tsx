import { useId, type ChangeEvent, type ReactNode } from 'react';

import { USER_ROLES, USER_TEXTS } from '@/entities/user';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { getFieldErrorText } from '@/shared/lib/form-errors';
import { Button } from '@/shared/ui/button/Button';

import { toUserRole } from '../lib/user-form-values';
import type { useUserForm } from '../model/use-user-form';

import styles from './UserForm.module.scss';

type UserFormApi = ReturnType<typeof useUserForm>['form'];

interface UserFormProps {
  form: UserFormApi;
  submitErrorText: string | undefined;
}

interface FieldProps {
  label: string;
  errors: readonly unknown[];
  children: (control: {
    id: string;
    'aria-invalid': true | undefined;
    'aria-describedby': string | undefined;
  }) => ReactNode;
}

const Field = ({ label, errors, children }: FieldProps) => {
  const id = useId();
  const errorText = getFieldErrorText(errors);
  const errorId = `${id}-error`;

  return (
    <div className={styles.field}>
      <label htmlFor={id}>{label}</label>
      {children({
        id,
        'aria-invalid': errorText ? true : undefined,
        'aria-describedby': errorText ? errorId : undefined,
      })}
      {errorText && (
        <p id={errorId} className={styles.error}>
          {errorText}
        </p>
      )}
    </div>
  );
};

const TEXT_FIELDS = [
  { name: 'firstName', type: 'text', autoComplete: 'given-name' },
  { name: 'lastName', type: 'text', autoComplete: 'family-name' },
  { name: 'email', type: 'email', autoComplete: 'email' },
] as const;

export const UserForm = ({ form, submitErrorText }: UserFormProps) => (
  <form
    className={styles.form}
    noValidate
    onSubmit={(event) => {
      event.preventDefault();
      void form.handleSubmit({ formElement: event.currentTarget });
    }}
  >
    {TEXT_FIELDS.map(({ name, type, autoComplete }) => (
      <form.Field key={name} name={name}>
        {(field) => (
          <Field label={USER_TEXTS.fields[name]} errors={field.state.meta.errors}>
            {(control) => (
              <input
                {...control}
                name={field.name}
                type={type}
                autoComplete={autoComplete}
                value={field.state.value}
                onBlur={field.handleBlur}
                onChange={(event: ChangeEvent<HTMLInputElement>) => field.handleChange(event.target.value)}
              />
            )}
          </Field>
        )}
      </form.Field>
    ))}

    <form.Field name="role">
      {(field) => (
        <Field label={USER_TEXTS.fields.role} errors={field.state.meta.errors}>
          {(control) => (
            <select
              {...control}
              name={field.name}
              value={field.state.value}
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(toUserRole(event.target.value))}
            >
              {USER_ROLES.map((role) => (
                <option key={role} value={role}>
                  {USER_TEXTS.roles[role]}
                </option>
              ))}
            </select>
          )}
        </Field>
      )}
    </form.Field>

    {submitErrorText && (
      <p role="alert" className={styles.error}>
        {submitErrorText}
      </p>
    )}

    <form.Subscribe selector={(state) => state.isSubmitting}>
      {(isSubmitting) => (
        <Button type="submit" disabled={isSubmitting}>
          {isSubmitting ? COMMON_TEXTS.actions.saving : COMMON_TEXTS.actions.save}
        </Button>
      )}
    </form.Subscribe>
  </form>
);
