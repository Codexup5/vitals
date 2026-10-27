import { Link } from '@tanstack/react-router';

import { USER_TEXTS } from '@/entities/user';

import { USER_LIST_TEXTS } from '../config/texts';
import type { UserRow } from '../lib/user-list-view';

import styles from './UserTable.module.scss';

interface UserTableProps {
  rows: readonly UserRow[];
  isRefreshing: boolean;
}

export const UserTable = ({ rows, isRefreshing }: UserTableProps) => (
  <div className={styles.scroller}>
    <table className={styles.table} aria-busy={isRefreshing}>
      <caption className={styles.caption}>{USER_LIST_TEXTS.tableCaption}</caption>
      <thead>
        <tr>
          <th scope="col">{USER_TEXTS.fields.fullName}</th>
          <th scope="col">{USER_TEXTS.fields.email}</th>
          <th scope="col">{USER_TEXTS.fields.role}</th>
          <th scope="col">{USER_TEXTS.fields.createdAt}</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.id}>
            <td>
              <Link from="/users" to="/users/$userId" params={{ userId: row.id }} search={(list) => list}>
                {row.fullName}
              </Link>
            </td>
            <td>{row.email}</td>
            <td>{row.roleLabel}</td>
            <td>{row.createdAtLabel}</td>
          </tr>
        ))}
      </tbody>
    </table>
  </div>
);
