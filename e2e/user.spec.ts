import { randomUUID } from 'node:crypto';
import { expect, test } from '@playwright/test';

test('a created user can be opened, edited and found in the list', async ({ page }, testInfo) => {
  const email = `e2e-${testInfo.project.name}-${randomUUID()}@example.com`;

  await page.goto('/');
  await expect(page).toHaveURL(/\/users/);
  await expect(page.getByRole('heading', { name: 'Пользователи' })).toBeVisible();

  await page.getByRole('link', { name: 'Добавить пользователя' }).click();
  await page.getByLabel('Имя', { exact: true }).fill('Борис');
  await page.getByLabel('Фамилия').fill('Тестов');
  await page.getByLabel('Почта').fill(email);
  await page.getByRole('button', { name: 'Сохранить' }).click();
  await expect(page.getByRole('heading', { name: 'Борис Тестов' })).toBeVisible();

  await page.getByRole('link', { name: 'Редактировать' }).click();
  await page.getByLabel('Имя', { exact: true }).fill('Богдан');
  await page.getByRole('button', { name: 'Сохранить' }).click();
  await expect(page.getByRole('heading', { name: 'Богдан Тестов' })).toBeVisible();

  await page.getByRole('link', { name: 'Ко всем пользователям' }).click();
  await page.getByRole('searchbox').fill(email);
  await page.getByRole('button', { name: 'Найти' }).click();
  // the previous page stays on screen while the search loads, and a shared database holds other runs' users:
  // wait for the header and the one found row, then look inside it
  await expect(page.getByRole('row')).toHaveCount(2);
  await expect(page.getByRole('row', { name: email }).getByRole('link', { name: 'Богдан Тестов' })).toBeVisible();
});

test('filters survive a reload because they live in the URL', async ({ page }) => {
  await page.goto('/users');
  await page.getByRole('searchbox').fill('вера');
  await page.getByRole('button', { name: 'Найти' }).click();
  await expect(page.getByText('1 запись')).toBeVisible();

  await page.reload();
  await expect(page.getByRole('searchbox')).toHaveValue('вера');
  await expect(page.getByText('1 запись')).toBeVisible();
});

test('the user list at 375 px does not scroll the page; the table scrolls inside its own box', async ({ page }) => {
  const viewport = { width: 375, height: 812 };
  await page.setViewportSize(viewport);
  await page.goto('/users');
  await expect(page.getByRole('table')).toBeVisible();

  const documentWidth = await page.evaluate<{ scrollWidth: number; clientWidth: number }>(
    '({ scrollWidth: document.documentElement.scrollWidth, clientWidth: document.documentElement.clientWidth })',
  );
  expect(documentWidth.scrollWidth).toBeLessThanOrEqual(documentWidth.clientWidth);

  const scrollBox = page.getByRole('table').locator('..');
  await expect(scrollBox.getByRole('columnheader', { name: 'Роль' })).toBeAttached();
  await expect(scrollBox).toHaveCSS('overflow-x', 'auto');
  const bounds = await scrollBox.boundingBox();
  expect(bounds, 'the box the table scrolls in is on screen').not.toBeNull();
  expect((bounds?.x ?? 0) + (bounds?.width ?? Infinity)).toBeLessThanOrEqual(viewport.width);
});
