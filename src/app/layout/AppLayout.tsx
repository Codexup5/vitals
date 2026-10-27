import { Link, Outlet } from '@tanstack/react-router';

import { APP_TEXTS } from '../config/texts';

import styles from './AppLayout.module.scss';

export const AppLayout = () => (
  <div className={styles.layout}>
    <header className={styles.header}>
      <Link to="/users" search={{}} className={styles.brand}>
        {APP_TEXTS.brand}
      </Link>
    </header>
    <Outlet />
  </div>
);
