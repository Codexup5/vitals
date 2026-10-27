import { FieldApi, FormApi, type AnyFieldApi, type AnyFormApi } from '@tanstack/react-form';
import { describe, expect, it, onTestFinished } from 'vitest';

import { clearServerError, focusFirstInvalidField, getFieldErrorText, putServerErrors } from './form-errors';

const createMountedForm = () => {
  const form: AnyFormApi = new FormApi({ defaultValues: { email: '', name: '' } });
  form.mount();
  const email: AnyFieldApi = new FieldApi({ form, name: 'email' });
  email.mount();
  const name: AnyFieldApi = new FieldApi({ form, name: 'name' });
  name.mount();
  return { form, email, name };
};

const createFormElement = (controlTags: readonly (readonly [tag: string, name: string])[]) => {
  const formElement = document.createElement('form');
  for (const [tag, name] of controlTags) {
    const control = document.createElement(tag);
    control.setAttribute('name', name);
    formElement.append(control);
  }
  document.body.append(formElement);
  onTestFinished(() => formElement.remove());
  return formElement;
};

describe('getFieldErrorText', () => {
  it('returns the first string and skips the rest', () => {
    expect(getFieldErrorText(['first', 'second'])).toBe('first');
  });

  it('returns the message of the first issue with a message', () => {
    expect(getFieldErrorText([{ message: 'from issue' }, 'later'])).toBe('from issue');
  });

  it('skips values that are neither a string nor an issue with a text message', () => {
    expect(getFieldErrorText([undefined, null, 5, { message: 7 }, {}, 'found'])).toBe('found');
  });

  it('returns undefined when there is no text', () => {
    expect(getFieldErrorText([])).toBeUndefined();
    expect(getFieldErrorText([undefined, { code: 'x' }])).toBeUndefined();
  });
});

describe('putServerErrors', () => {
  it('puts the errors of the fields the form knows and returns their count', () => {
    const { form } = createMountedForm();

    const putCount = putServerErrors(form, { email: 'Email is taken', name: 'Name is too long' });

    expect(putCount).toBe(2);
    expect(form.getFieldMeta('email')?.errorMap.onServer).toBe('Email is taken');
    expect(form.getFieldMeta('name')?.errorMap.onServer).toBe('Name is too long');
  });

  it('skips the fields the form does not know', () => {
    const { form } = createMountedForm();

    const putCount = putServerErrors(form, { email: 'Email is taken', phone: 'Phone is wrong' });

    expect(putCount).toBe(1);
    expect(form.getFieldMeta('email')?.errorMap.onServer).toBe('Email is taken');
    expect(form.getFieldMeta('phone')).toBeUndefined();
  });

  it('returns zero when no field is known', () => {
    const { form } = createMountedForm();

    expect(putServerErrors(form, { phone: 'Phone is wrong' })).toBe(0);
    expect(putServerErrors(form, {})).toBe(0);
  });
});

describe('focusFirstInvalidField', () => {
  it('focuses the first control in DOM order whose name has errors, not the first in the form state', () => {
    const { form } = createMountedForm();
    const formElement = createFormElement([
      ['input', 'name'],
      ['input', 'email'],
    ]);
    putServerErrors(form, { email: 'Email is taken', name: 'Name is wrong' });

    focusFirstInvalidField(formElement, form);

    expect(document.activeElement).toBe(formElement.elements.namedItem('name'));
  });

  it('skips controls without errors and focuses a later invalid one', () => {
    const { form } = createMountedForm();
    const formElement = createFormElement([
      ['input', 'name'],
      ['select', 'email'],
    ]);
    putServerErrors(form, { email: 'Email is taken' });

    focusFirstInvalidField(formElement, form);

    expect(document.activeElement).toBe(formElement.elements.namedItem('email'));
  });

  it('ignores controls whose name the form does not know', () => {
    const { form } = createMountedForm();
    const formElement = createFormElement([
      ['input', 'phone'],
      ['input', 'email'],
    ]);
    putServerErrors(form, { email: 'Email is taken' });

    focusFirstInvalidField(formElement, form);

    expect(document.activeElement).toBe(formElement.elements.namedItem('email'));
  });

  it('leaves the focus alone when no field has errors', () => {
    const { form } = createMountedForm();
    const formElement = createFormElement([
      ['input', 'name'],
      ['input', 'email'],
    ]);

    focusFirstInvalidField(formElement, form);

    expect(document.activeElement).toBe(document.body);
  });
});

describe('clearServerError', () => {
  it('clears onServer and leaves the other error sources', () => {
    const { email } = createMountedForm();
    email.setMeta((meta) => ({
      ...meta,
      errorMap: { ...meta.errorMap, onServer: 'Email is taken', onChange: 'Email is wrong' },
    }));

    clearServerError(email);

    expect(email.state.meta.errorMap.onServer).toBeUndefined();
    expect(email.state.meta.errorMap.onChange).toBe('Email is wrong');
  });

  it('leaves the field as it is when it has no server error', () => {
    const { email } = createMountedForm();
    email.setMeta((meta) => ({ ...meta, errorMap: { ...meta.errorMap, onChange: 'Email is wrong' } }));
    const metaBefore = email.state.meta;

    clearServerError(email);

    expect(email.state.meta).toBe(metaBefore);
  });
});
