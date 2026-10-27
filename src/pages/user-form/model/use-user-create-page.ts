import { useCreateUser } from '@/entities/user';

import { useUserForm } from './use-user-form';

export const useUserCreatePage = () => {
  const creation = useCreateUser();
  return useUserForm({ save: (draft) => creation.mutateAsync(draft) });
};
