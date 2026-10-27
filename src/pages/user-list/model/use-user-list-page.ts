import { useQuery } from '@tanstack/react-query';
import { getRouteApi } from '@tanstack/react-router';
import { useState, type SubmitEvent } from 'react';

import { userListQuery, userListSearchSchema } from '@/entities/user';

import { USER_LIST_PAGE_SIZE } from '../config/user-list';
import { toUserListView } from '../lib/user-list-view';

const route = getRouteApi('/users');

export const useUserListPage = () => {
  const search = route.useSearch();
  const navigate = route.useNavigate();
  const hasFilters = search.search !== '' || search.role !== undefined;

  const {
    data: userPage,
    error,
    isPlaceholderData,
    refetch,
  } = useQuery(
    userListQuery({
      page: search.page,
      pageSize: USER_LIST_PAGE_SIZE,
      search: search.search,
      role: search.role,
    }),
  );

  const view = toUserListView({ userPage, error, page: search.page, hasFilters, isRefreshing: isPlaceholderData });

  const urlFilters = { search: search.search, role: search.role ?? '' };
  const [draft, setDraft] = useState(urlFilters);
  const [syncedFilters, setSyncedFilters] = useState(urlFilters);
  if (syncedFilters.search !== urlFilters.search || syncedFilters.role !== urlFilters.role) {
    setSyncedFilters(urlFilters);
    setDraft(urlFilters);
  }

  const changeSearch = (value: string) => setDraft((previous) => ({ ...previous, search: value }));

  const changeRole = (value: string) => setDraft((previous) => ({ ...previous, role: value }));

  const handleFilterSubmit = (event: SubmitEvent<HTMLFormElement>) => {
    event.preventDefault();
    void navigate({ search: userListSearchSchema.parse(draft) });
  };

  const resetFilters = () => void navigate({ search: {} });

  const goToPage = (page: number) => void navigate({ search: (previous) => ({ ...previous, page }) });

  return {
    view,
    filters: draft,
    changeSearch,
    changeRole,
    handleFilterSubmit,
    resetFilters,
    goToPage,
    retry: () => void refetch(),
  };
};
