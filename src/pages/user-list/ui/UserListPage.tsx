import { Link } from '@tanstack/react-router';

import { USER_ROLES, USER_SEARCH_MAX_LENGTH, USER_TEXTS } from '@/entities/user';
import { COMMON_TEXTS } from '@/shared/config/texts';
import { Button } from '@/shared/ui/button/Button';

import { USER_LIST_TEXTS } from '../config/texts';
import { useUserListPage } from '../model/use-user-list-page';
import { UserListSkeleton } from './UserListSkeleton';
import { UserTable } from './UserTable';

import styles from './UserListPage.module.scss';

export const UserListPage = () => {
  const { view, filters, changeSearch, changeRole, handleFilterSubmit, resetFilters, goToPage, retry } =
    useUserListPage();

  return (
    <main>
      <header className={styles.header}>
        <h1>{USER_LIST_TEXTS.title}</h1>
        <Link to="/users/new" className={styles.primaryAction}>
          {USER_LIST_TEXTS.create}
        </Link>
      </header>

      <form className={styles.filters} onSubmit={handleFilterSubmit}>
        <label className={styles.field}>
          <span>{USER_LIST_TEXTS.searchLabel}</span>
          <input
            name="search"
            type="search"
            maxLength={USER_SEARCH_MAX_LENGTH}
            value={filters.search}
            onChange={(event) => changeSearch(event.target.value)}
          />
        </label>
        <label className={styles.field}>
          <span>{USER_LIST_TEXTS.roleFilterLabel}</span>
          <select name="role" value={filters.role} onChange={(event) => changeRole(event.target.value)}>
            <option value="">{USER_LIST_TEXTS.anyRole}</option>
            {USER_ROLES.map((role) => (
              <option key={role} value={role}>
                {USER_TEXTS.roles[role]}
              </option>
            ))}
          </select>
        </label>
        <Button type="submit">{COMMON_TEXTS.actions.search}</Button>
      </form>

      {view.status === 'loading' && <UserListSkeleton />}

      {view.status === 'error' && (
        <section role="alert" className={styles.state}>
          <h2>{COMMON_TEXTS.errors.screenTitle}</h2>
          <p>{view.message}</p>
          <Button type="button" onClick={retry}>
            {COMMON_TEXTS.actions.retry}
          </Button>
        </section>
      )}

      {view.status === 'empty' && (
        <section className={styles.state}>
          <h2>{view.hasFilters ? USER_LIST_TEXTS.notFoundTitle : USER_LIST_TEXTS.emptyTitle}</h2>
          <p>{view.hasFilters ? USER_LIST_TEXTS.notFoundDescription : USER_LIST_TEXTS.emptyDescription}</p>
          {view.hasFilters ? (
            <Button type="button" onClick={resetFilters}>
              {USER_LIST_TEXTS.resetFilters}
            </Button>
          ) : (
            <Link to="/users/new">{USER_LIST_TEXTS.create}</Link>
          )}
        </section>
      )}

      {view.status === 'pastLastPage' && (
        <section className={styles.state}>
          <h2>{USER_LIST_TEXTS.pastLastPageTitle}</h2>
          <Button type="button" onClick={() => goToPage(view.lastPage)}>
            {USER_LIST_TEXTS.pastLastPageAction}
          </Button>
        </section>
      )}

      {view.status === 'ready' && (
        <>
          <p className={styles.summary}>{COMMON_TEXTS.results(view.total)}</p>
          <UserTable rows={view.rows} isRefreshing={view.isRefreshing} />
          <nav aria-label={COMMON_TEXTS.pagination.label} className={styles.pagination}>
            <Button type="button" disabled={view.page <= 1} onClick={() => goToPage(view.page - 1)}>
              {COMMON_TEXTS.pagination.previous}
            </Button>
            <span>{COMMON_TEXTS.pagination.position(view.page, view.pageCount)}</span>
            <Button type="button" disabled={view.page >= view.pageCount} onClick={() => goToPage(view.page + 1)}>
              {COMMON_TEXTS.pagination.next}
            </Button>
          </nav>
        </>
      )}
    </main>
  );
};
