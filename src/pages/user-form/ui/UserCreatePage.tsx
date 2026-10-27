import { Link } from '@tanstack/react-router';

import { USER_FORM_TEXTS } from '../config/texts';
import { useUserCreatePage } from '../model/use-user-create-page';
import { UserForm } from './UserForm';

export const UserCreatePage = () => {
  const { form, submitErrorText } = useUserCreatePage();

  return (
    <main>
      <Link to="/users" search={{}}>
        {USER_FORM_TEXTS.backToList}
      </Link>
      <h1>{USER_FORM_TEXTS.createTitle}</h1>
      <UserForm form={form} submitErrorText={submitErrorText} />
    </main>
  );
};
