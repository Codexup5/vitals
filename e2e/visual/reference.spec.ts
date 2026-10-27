import { expect, test } from '@playwright/test';

test('the user list', async ({ page }) => {
  await page.goto('/users');
  await expect(page.getByRole('row')).toHaveCount(4);
  await expect(page.getByRole('main')).toHaveScreenshot('user-list.png');
});

test('the user list in the dark theme', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/users');
  await expect(page.getByRole('row')).toHaveCount(4);
  await expect(page.getByRole('main')).toHaveScreenshot('user-list-dark.png');
});

test('the user details', async ({ page }) => {
  await page.goto('/users/u1');
  await expect(page.getByRole('heading', { name: 'Анна Петрова' })).toBeVisible();
  await expect(page.getByRole('main')).toHaveScreenshot('user-details.png');
});

test('the user form with its required-field messages', async ({ page }) => {
  await page.goto('/users/new');
  await page.getByRole('button', { name: 'Сохранить' }).click();
  await expect(page.getByText('Заполните поле').first()).toBeVisible();
  await expect(page.getByRole('main')).toHaveScreenshot('user-form-errors.png');
});
