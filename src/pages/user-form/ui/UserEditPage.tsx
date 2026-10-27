import { Link } from '@tanstack/react-router';

import { USER_FORM_TEXTS } from '../config/texts';
import { useUserEditPage } from '../model/use-user-edit-page';
import { UserForm } from './UserForm';

export const UserEditPage = () => {
  const { userId, form, submitErrorText } = useUserEditPage();

  return (
    <main>
      <Link to="/users/$userId" params={{ userId }}>
        {USER_FORM_TEXTS.backToUser}
      </Link>
      <h1>{USER_FORM_TEXTS.editTitle}</h1>
      <UserForm form={form} submitErrorText={submitErrorText} />
    </main>
  );
};
