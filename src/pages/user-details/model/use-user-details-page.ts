import { useSuspenseQuery } from '@tanstack/react-query';
import { getRouteApi } from '@tanstack/react-router';
import { useState } from 'react';

import { getUserFullName, useDeleteUser, userByIdQuery } from '@/entities/user';
import { getErrorMessage } from '@/shared/api/error-message';

import { toUserAttributes } from '../lib/user-attributes';

const route = getRouteApi('/users/$userId');

export const useUserDetailsPage = () => {
  const { userId } = route.useParams();
  const navigate = route.useNavigate();
  const { data: user } = useSuspenseQuery(userByIdQuery(userId));
  const deletion = useDeleteUser(userId, {
    onDeleted: () => navigate({ to: '/users', search: ({ search, role }) => ({ search, role }) }),
  });
  const [isConfirmingDelete, setIsConfirmingDelete] = useState(false);

  return {
    userId,
    title: getUserFullName(user),
    attributes: toUserAttributes(user),
    deletion: {
      isConfirming: isConfirmingDelete,
      isPending: deletion.isPending,
      errorText: deletion.isError ? getErrorMessage(deletion.error) : undefined,
      askConfirmation: () => setIsConfirmingDelete(true),
      cancel: () => setIsConfirmingDelete(false),
      confirm: () => deletion.mutate(),
    },
  };
};
