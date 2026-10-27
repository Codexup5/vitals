import type { AnyFieldApi, AnyFormApi } from '@tanstack/react-form';

import type { FieldErrors } from '../api/api-error';

const isIssueWithMessage = (issue: unknown): issue is { message: string } =>
  typeof issue === 'object' && issue !== null && 'message' in issue && typeof issue.message === 'string';

export const getFieldErrorText = (errors: readonly unknown[]) => {
  for (const issue of errors) {
    if (typeof issue === 'string') return issue;
    if (isIssueWithMessage(issue)) return issue.message;
  }
  return undefined;
};

export const putServerErrors = (form: AnyFormApi, fields: FieldErrors) => {
  const knownFields = Object.entries(fields).filter(([name]) => name in form.fieldInfo);
  for (const [name, message] of knownFields) {
    form.setFieldMeta(name, (meta) => ({ ...meta, errorMap: { ...meta.errorMap, onServer: message } }));
  }
  return knownFields.length;
};

export const focusFirstInvalidField = (formElement: HTMLFormElement, form: AnyFormApi): void => {
  const hasErrors = (name: string) => (form.getFieldMeta(name)?.errors.length ?? 0) > 0;
  for (const control of formElement.elements) {
    if (!(control instanceof HTMLElement) || !('name' in control) || typeof control.name !== 'string') continue;
    if (!hasErrors(control.name)) continue;
    control.focus();
    return;
  }
};

export const clearServerError = (field: AnyFieldApi) => {
  if (field.state.meta.errorMap.onServer === undefined) return;
  field.setMeta((meta) => ({ ...meta, errorMap: { ...meta.errorMap, onServer: undefined } }));
};
