import type { USER_ROLES } from './user-roles';

export const USER_TEXTS = {
  roles: {
    admin: 'Администратор',
    editor: 'Редактор',
    viewer: 'Читатель',
  } satisfies Record<(typeof USER_ROLES)[number], string>,
  fields: {
    fullName: 'Имя и фамилия',
    firstName: 'Имя',
    lastName: 'Фамилия',
    email: 'Почта',
    role: 'Роль',
    createdAt: 'Создан',
  },
  validation: {
    required: 'Заполните поле',
    tooLong: (max: number) => `Не длиннее ${max} символов`,
    email: 'Почта в формате name@example.com',
  },
} as const;
