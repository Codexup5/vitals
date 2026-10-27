import type { User } from '../model/user';

export const getUserFullName = ({ firstName, lastName }: Pick<User, 'firstName' | 'lastName'>) =>
  `${firstName} ${lastName}`.trim();
