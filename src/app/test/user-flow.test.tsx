import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { http, HttpResponse } from 'msw';
import { afterEach, describe, expect, it, onTestFinished, vi } from 'vitest';

import { USER_SEARCH_MAX_LENGTH } from '@/entities/user';

import { server } from '../../../mocks/node';
import { userDb } from '../../../mocks/user-db';
import { APP_TEXTS } from '../config/texts';
import { renderApp } from './render-app';

afterEach(() => vi.restoreAllMocks());

describe('user screens', () => {
  it('lists users and filters them through the URL', async () => {
    const { router } = renderApp('/users');
    const table = await screen.findByRole('table');
    expect(within(table).getAllByRole('row')).toHaveLength(4);

    await userEvent.type(screen.getByRole('searchbox'), 'анна');
    await userEvent.click(screen.getByRole('button', { name: 'Найти' }));

    expect(await screen.findByText('1 запись')).toBeInTheDocument();
    expect(router.state.location.search).toEqual({ search: 'анна' });

    await userEvent.clear(screen.getByRole('searchbox'));
    await userEvent.click(screen.getByRole('button', { name: 'Найти' }));

    expect(await screen.findByText('3 записи')).toBeInTheDocument();
    expect(router.state.location.href).toBe('/users');
  });

  it('shows a busy skeleton instead of the table while the list loads', async () => {
    let releaseList: () => void = () => undefined;
    const listHeld = new Promise<void>((resolve) => {
      releaseList = resolve;
    });
    server.use(
      http.get('*/api/users', async () => {
        await listHeld;
        const items = userDb.list();
        return HttpResponse.json({ items, total: items.length });
      }),
    );
    renderApp('/users');

    const loading = await screen.findByText('Загружаем…');
    expect(loading.closest('[aria-busy="true"]')).not.toBeNull();
    expect(screen.queryByRole('table')).not.toBeInTheDocument();

    releaseList();
    expect(await screen.findByRole('table')).toBeInTheDocument();
    expect(screen.queryByText('Загружаем…')).not.toBeInTheDocument();
  });

  it('caps the search box at the entity limit', async () => {
    renderApp('/users');
    expect(await screen.findByRole('searchbox')).toHaveAttribute('maxlength', String(USER_SEARCH_MAX_LENGTH));
  });

  it('shows the rest of the list when one item is broken', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    server.use(
      http.get('*/api/users', () =>
        HttpResponse.json({
          items: [
            { id: 'u1', first_name: 'Анна', last_name: 'Петрова', email: 'anna@example.com', role: 'admin' },
            { id: 'u2', first_name: 'Борис', last_name: 'Иванов', role: 'owner' },
          ],
          total: 2,
        }),
      ),
    );
    renderApp('/users');
    const table = await screen.findByRole('table');
    expect(within(table).getAllByRole('row')).toHaveLength(2);
    expect(within(table).getByText('Анна Петрова')).toBeInTheDocument();
    expect(within(table).getByText('—')).toBeInTheDocument();
  });

  it('offers to reset filters when nothing matches', async () => {
    renderApp('/users?search=%22nobody%22');
    expect(await screen.findByRole('heading', { name: 'Никого не нашли' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Сбросить фильтры' })).toBeInTheDocument();
  });

  it('keeps focus on the search button after a search', async () => {
    renderApp('/users');
    await userEvent.type(await screen.findByRole('searchbox'), 'анна');
    await userEvent.click(screen.getByRole('button', { name: 'Найти' }));

    expect(await screen.findByText('1 запись')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Найти' })).toHaveFocus();
  });

  it('empties the search box when the filters are reset from the empty state', async () => {
    renderApp('/users?search=%22nobody%22');
    expect(await screen.findByRole('searchbox')).toHaveValue('nobody');

    await userEvent.click(await screen.findByRole('button', { name: 'Сбросить фильтры' }));

    expect(await screen.findByRole('table')).toBeInTheDocument();
    expect(screen.getByRole('searchbox')).toHaveValue('');
  });

  it('offers the last page when the page is out of range', async () => {
    renderApp('/users?page=99');
    await userEvent.click(await screen.findByRole('button', { name: 'К последней странице' }));
    expect(await screen.findByRole('table')).toBeInTheDocument();
  });

  it('shows a recoverable error when the list fails', async () => {
    server.use(http.get('*/api/users', () => HttpResponse.json({}, { status: 503 })));
    renderApp('/users');
    expect(await screen.findByRole('alert', {}, { timeout: 5000 })).toHaveTextContent('Сервер не смог');
    expect(screen.getByRole('button', { name: 'Повторить' })).toBeInTheDocument();
  });

  it('shows not found for a missing user', async () => {
    renderApp('/users/missing');
    expect(await screen.findByRole('heading', { name: 'Страница не найдена' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: APP_TEXTS.brand })).toBeInTheDocument();
  });

  it('keeps the layout when the user card fails to load', async () => {
    server.use(http.get('*/api/users/:userId', () => HttpResponse.json({}, { status: 503 })));
    renderApp('/users/u2');
    expect(await screen.findByRole('alert', {}, { timeout: 5000 })).toHaveTextContent('Сервер не смог');
    expect(screen.getByRole('link', { name: APP_TEXTS.brand })).toBeInTheDocument();
  });

  it('creates a user and puts the server field error under the field', async () => {
    renderApp('/users/new');
    await userEvent.type(await screen.findByLabelText('Имя'), 'Дина');
    await userEvent.type(screen.getByLabelText('Фамилия'), 'Орлова');
    await userEvent.type(screen.getByLabelText('Почта'), 'anna@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Сохранить' }));

    expect(await screen.findByText('Эта почта уже занята')).toBeInTheDocument();

    await userEvent.clear(screen.getByLabelText('Почта'));
    await userEvent.type(screen.getByLabelText('Почта'), 'dina@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Сохранить' }));

    expect(await screen.findByRole('heading', { name: 'Дина Орлова' })).toBeInTheDocument();
  });

  it('validates the form before sending', async () => {
    renderApp('/users/new');
    await userEvent.click(await screen.findByRole('button', { name: 'Сохранить' }));
    expect(await screen.findAllByText('Заполните поле')).toHaveLength(3);
  });

  it('focuses the first field when an empty form is submitted', async () => {
    renderApp('/users/new');
    await userEvent.click(await screen.findByRole('button', { name: 'Сохранить' }));

    await screen.findAllByText('Заполните поле');
    expect(screen.getByLabelText('Имя')).toHaveFocus();
  });

  it('focuses the email field when the server rejects the email', async () => {
    renderApp('/users/new');
    await userEvent.type(await screen.findByLabelText('Имя'), 'Дина');
    await userEvent.type(screen.getByLabelText('Фамилия'), 'Орлова');
    await userEvent.type(screen.getByLabelText('Почта'), 'anna@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Сохранить' }));

    await screen.findByText('Эта почта уже занята');
    expect(screen.getByLabelText('Почта')).toHaveFocus();
  });

  it('removes the submit error once a field is edited', async () => {
    server.use(http.post('*/api/users', () => HttpResponse.json({}, { status: 409 })));
    renderApp('/users/new');
    await userEvent.type(await screen.findByLabelText('Имя'), 'Дина');
    await userEvent.type(screen.getByLabelText('Фамилия'), 'Орлова');
    await userEvent.type(screen.getByLabelText('Почта'), 'dina@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Сохранить' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Запись уже изменили. Обновите страницу и повторите.');

    await userEvent.type(screen.getByLabelText('Имя'), 'а');

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('deletes a user after an in-place confirmation', async () => {
    const requests: string[] = [];
    const logRequest = ({ request }: { request: Request }) => {
      requests.push(`${request.method} ${new URL(request.url).pathname}`);
    };
    server.events.on('request:start', logRequest);
    onTestFinished(() => server.events.removeListener('request:start', logRequest));

    renderApp('/users/u2');
    await userEvent.click(await screen.findByRole('button', { name: 'Удалить' }));
    await userEvent.click(screen.getByRole('button', { name: 'Да, удалить' }));

    const table = await screen.findByRole('table');
    expect(within(table).queryByText('Борис Иванов')).not.toBeInTheDocument();
    const userRequests = requests.filter((entry) => entry.endsWith('/api/users/u2'));
    expect(userRequests).toEqual(['GET /api/users/u2', 'DELETE /api/users/u2']);
  });

  it('does not show the deleted user in the cached list while the list reloads', async () => {
    const { router } = renderApp('/users');
    await userEvent.click(await screen.findByRole('link', { name: 'Борис Иванов' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Удалить' }));

    let releaseList: () => void = () => undefined;
    const listHeld = new Promise<void>((resolve) => {
      releaseList = resolve;
    });
    server.use(
      http.get('*/api/users', async () => {
        await listHeld;
        const items = userDb.list();
        return HttpResponse.json({ items, total: items.length });
      }),
    );
    await userEvent.click(screen.getByRole('button', { name: 'Да, удалить' }));

    await waitFor(() => expect(router.state.location.pathname).toBe('/users'));
    expect(screen.queryByText('Борис Иванов')).not.toBeInTheDocument();

    releaseList();
    const table = await screen.findByRole('table');
    expect(within(table).getAllByRole('row')).toHaveLength(3);
    expect(within(table).getByText('Анна Петрова')).toBeInTheDocument();
    expect(within(table).queryByText('Борис Иванов')).not.toBeInTheDocument();
  });

  it('returns to the filtered list after the deletion', async () => {
    userDb.insert({
      id: 'u4',
      first_name: 'Глеб',
      last_name: 'Миронов',
      email: 'gleb@example.com',
      role: 'editor',
      created_at: '2026-04-02T08:00:00.000Z',
    });
    const { router } = renderApp('/users');
    await userEvent.selectOptions(await screen.findByRole('combobox', { name: 'Роль' }), 'editor');
    await userEvent.click(screen.getByRole('button', { name: 'Найти' }));
    await waitFor(() => expect(router.state.location.search).toEqual({ role: 'editor' }));

    await userEvent.click(await screen.findByRole('link', { name: 'Борис Иванов' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Удалить' }));
    await userEvent.click(screen.getByRole('button', { name: 'Да, удалить' }));

    const table = await screen.findByRole('table');
    expect(router.state.location.pathname).toBe('/users');
    expect(router.state.location.search).toEqual({ role: 'editor' });
    expect(within(table).queryByText('Борис Иванов')).not.toBeInTheDocument();
    expect(within(table).getByText('Глеб Миронов')).toBeInTheDocument();
  });

  it('returns to the first page after the deletion', async () => {
    const { router } = renderApp('/users/u2?page=2&role=editor');
    await userEvent.click(await screen.findByRole('button', { name: 'Удалить' }));
    await userEvent.click(screen.getByRole('button', { name: 'Да, удалить' }));

    await screen.findByRole('heading', { name: 'Никого не нашли' });
    expect(router.state.location.pathname).toBe('/users');
    expect(router.state.location.search).toEqual({ role: 'editor' });
  });

  it('explains why the deletion failed', async () => {
    server.use(http.delete('*/api/users/:userId', () => HttpResponse.json({}, { status: 409 })));
    renderApp('/users/u2');
    await userEvent.click(await screen.findByRole('button', { name: 'Удалить' }));
    await userEvent.click(screen.getByRole('button', { name: 'Да, удалить' }));

    expect(await screen.findByText('Запись уже изменили. Обновите страницу и повторите.')).toBeInTheDocument();
  });
});
