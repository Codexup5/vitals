import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { updatePreferences } from '@/shared/lib/preferences';

import { renderApp } from './render-app';

describe('app shell', () => {
  it('opens the first module and lists all seven in the navigation', async () => {
    updatePreferences({ locale: 'ru' });
    renderApp('/');

    expect(await screen.findByRole('heading', { level: 1, name: 'Ошибки и падения' })).toBeInTheDocument();
    const navigation = screen.getByRole('navigation', { name: 'Модули' });
    expect(navigation.querySelectorAll('a')).toHaveLength(7);
    expect(screen.getByRole('link', { name: 'Ошибки и падения' })).toHaveAttribute('aria-current', 'page');
  });

  it('moves between modules', async () => {
    updatePreferences({ locale: 'ru' });
    renderApp('/errors');

    await userEvent.click(await screen.findByRole('link', { name: 'Сеть' }));

    expect(await screen.findByRole('heading', { level: 1, name: 'Сеть' })).toBeInTheDocument();
    expect(screen.getByText('Модуль появится на этапе 6 плана работ.')).toBeInTheDocument();
  });

  it('switches the language and the theme', async () => {
    updatePreferences({ locale: 'ru', theme: 'dark' });
    renderApp('/sensors');
    await screen.findByRole('heading', { level: 1, name: 'Датчики' });

    await userEvent.click(screen.getByRole('button', { name: 'Switch the interface to English' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Sensors' })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe('en');

    await userEvent.click(screen.getByRole('button', { name: 'Light theme' }));
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('shows not found for an unknown module', async () => {
    updatePreferences({ locale: 'en' });
    renderApp('/bios');

    expect(await screen.findByRole('heading', { level: 1, name: 'Section not found' })).toBeInTheDocument();
  });
});
