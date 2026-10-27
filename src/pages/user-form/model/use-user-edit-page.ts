import { useSuspenseQuery } from '@tanstack/react-query';
import { getRouteApi } from '@tanstack/react-router';

import { userByIdQuery, useUpdateUser } from '@/entities/user';

import { useUserForm } from './use-user-form';

const route = getRouteApi('/users/$userId/edit');

export const useUserEditPage = () => {
  const { userId } = route.useParams();
  const { data: user } = useSuspenseQuery(userByIdQuery(userId));
  const update = useUpdateUser(userId);
  const { form, submitErrorText } = useUserForm({ initialUser: user, save: (draft) => update.mutateAsync(draft) });

  return { userId, form, submitErrorText };
};
