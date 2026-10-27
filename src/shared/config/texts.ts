import { pluralize } from '../lib/plural';

export const COMMON_TEXTS = {
  actions: {
    save: 'Сохранить',
    saving: 'Сохраняем…',
    cancel: 'Отмена',
    back: 'Назад',
    delete: 'Удалить',
    deleting: 'Удаляем…',
    retry: 'Повторить',
    search: 'Найти',
  },
  errors: {
    network: 'Нет связи с сервером. Проверьте интернет и повторите.',
    timeout: 'Сервер отвечает слишком долго. Повторите через минуту.',
    server: 'Сервер не смог обработать запрос. Повторите через минуту.',
    notFound: 'Такой страницы нет. Возможно, её удалили или ссылка устарела.',
    forbidden: 'Недостаточно прав для этого действия.',
    unauthorized: 'Сессия закончилась. Войдите снова.',
    conflict: 'Запись уже изменили. Обновите страницу и повторите.',
    validation: 'Сервер не принял данные. Проверьте поля и повторите.',
    tooManyRequests: 'Слишком много запросов. Подождите минуту и повторите.',
    unexpected: 'Что-то пошло не так. Обновите страницу.',
    screenTitle: 'Не удалось загрузить',
  },
  pagination: {
    label: 'Страницы',
    previous: 'Назад',
    next: 'Вперёд',
    position: (page: number, pageCount: number) => `Страница ${page} из ${pageCount}`,
  },
  loading: 'Загружаем…',
  missingValue: '—',
  results: (count: number) => `${count} ${pluralize(count, ['запись', 'записи', 'записей'])}`,
} as const;
