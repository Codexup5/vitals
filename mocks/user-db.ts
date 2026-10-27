import type { UserDraftDto, UserDto } from '@/entities/user/api/user-dto';

const SEED: readonly UserDto[] = [
  {
    id: 'u1',
    first_name: 'Анна',
    last_name: 'Петрова',
    email: 'anna@example.com',
    role: 'admin',
    created_at: '2026-01-12T09:00:00.000Z',
  },
  {
    id: 'u2',
    first_name: 'Борис',
    last_name: 'Иванов',
    email: 'boris@example.com',
    role: 'editor',
    created_at: '2026-02-03T10:30:00.000Z',
  },
  {
    id: 'u3',
    first_name: 'Вера',
    last_name: 'Сидорова',
    email: 'vera@example.com',
    role: 'viewer',
    created_at: '2026-03-21T14:15:00.000Z',
  },
];

let users: UserDto[] = [...SEED];

export const userDb = {
  reset: () => {
    users = [...SEED];
  },
  list: () => users,
  find: (id: string) => users.find((user) => user.id === id),
  insert: (user: UserDto) => {
    users = [...users, user];
    return user;
  },
  update: (id: string, patch: UserDraftDto) => {
    const current = users.find((user) => user.id === id);
    if (!current) return undefined;
    const next = { ...current, ...patch };
    users = users.map((user) => (user.id === id ? next : user));
    return next;
  },
  remove: (id: string) => {
    const wasFound = users.some((user) => user.id === id);
    users = users.filter((user) => user.id !== id);
    return wasFound;
  },
};
