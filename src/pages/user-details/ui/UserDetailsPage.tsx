import { Link } from '@tanstack/react-router';

import { COMMON_TEXTS } from '@/shared/config/texts';
import { Button } from '@/shared/ui/button/Button';

import { USER_DETAILS_TEXTS } from '../config/texts';
import { useUserDetailsPage } from '../model/use-user-details-page';

import styles from './UserDetailsPage.module.scss';

export const UserDetailsPage = () => {
  const { userId, title, attributes, deletion } = useUserDetailsPage();

  return (
    <main>
      <Link from="/users/$userId" to="/users" search={(list) => list}>
        {USER_DETAILS_TEXTS.backToList}
      </Link>
      <h1>{title}</h1>

      <dl className={styles.attributes} aria-label={USER_DETAILS_TEXTS.attributes}>
        {attributes.map(({ label, text }) => (
          <div key={label} className={styles.attribute}>
            <dt>{label}</dt>
            <dd>{text}</dd>
          </div>
        ))}
      </dl>

      <div className={styles.actions}>
        <Link to="/users/$userId/edit" params={{ userId }}>
          {USER_DETAILS_TEXTS.edit}
        </Link>
        {deletion.isConfirming ? (
          <div role="group" aria-label={USER_DETAILS_TEXTS.confirmDelete} className={styles.confirm}>
            <p>{USER_DETAILS_TEXTS.confirmDelete}</p>
            <Button type="button" onClick={deletion.confirm} disabled={deletion.isPending}>
              {deletion.isPending ? COMMON_TEXTS.actions.deleting : USER_DETAILS_TEXTS.confirmDeleteAction}
            </Button>
            <Button type="button" onClick={deletion.cancel} disabled={deletion.isPending}>
              {COMMON_TEXTS.actions.cancel}
            </Button>
          </div>
        ) : (
          <Button type="button" onClick={deletion.askConfirmation}>
            {COMMON_TEXTS.actions.delete}
          </Button>
        )}
      </div>
      {deletion.errorText && <p role="alert">{deletion.errorText}</p>}
    </main>
  );
};
