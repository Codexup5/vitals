---
name: form
description: A form on TanStack Form + Zod — create and edit, validation by schema, field errors from the back end, the submitting state. Use for any submit form. Triggers «форма», «форма редактирования», «ошибки полей с бэка».
---

The reference is `src/pages/user-form`.

## 1. The schema lives in the entity (`model/<domain>-draft.ts`)

```ts
export const orderDraftSchema = z.object({
  title: z.string().trim().min(1, X_TEXTS.validation.required).max(ORDER_TITLE_MAX_LENGTH, …),
  amount: z.string().trim().pipe(z.coerce.number().int().positive()),
});
export type OrderDraftInput = z.input<typeof orderDraftSchema>;
export type OrderDraft = z.output<typeof orderDraftSchema>;
```

Input fields are strings (`Input`); the schema turns them into data (`Draft`). Limits come from the entity's `config/`.

## 2. The hook — `model/use-x-form.ts`

```ts
const form = useForm({
  defaultValues: toXFormValues(initialX),
  validationLogic: revalidateLogic(),
  validators: { onDynamic: orderDraftSchema },
  listeners: { onChange: ({ fieldApi }) => clearServerError(fieldApi) },
  onSubmit: async ({ value, formApi }) => {
    setSubmitErrorText(undefined);
    try {
      const saved = await save(orderDraftSchema.parse(value));
      await navigate({ to: '/orders/$orderId', params: { orderId: saved.id } });
    } catch (error) {
      if (!isApiError(error)) reportError(error, { form: 'order' });
      const matched = isApiError(error) ? putServerErrors(formApi, error.fields) : 0;
      if (matched === 0) setSubmitErrorText(getErrorMessage(error));
    }
  },
});
```

- No errors before the first submit; after it, a check on every change (`revalidateLogic`).
- Back-end field errors go under the fields; an error without a field — as text next to the button.
- Create and edit share one hook: the difference is `initialX` and `save`.

## 3. Markup

- `noValidate` on `form`, `onSubmit` → `form.handleSubmit()`.
- A field: `label htmlFor`, `aria-invalid`, `aria-describedby` on the error text — one private field component in the
  slice (from the third occurrence — in `shared/ui`).
- The button — `form.Subscribe` on `isSubmitting`: "Saving…" and `disabled`.

## 4. Test

An empty submit → errors under the fields; a 422 error → under the field; editing the field → the error is gone; a
successful submit → navigation.
