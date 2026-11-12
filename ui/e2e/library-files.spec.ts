// spec web-ui-library AC-12, AC-13, AC-15, AC-16 against the live-mode server, whose movies
// have real files in the temp folder (e2e/seed.mjs).

import { expect, test } from '@playwright/test';
import { LIVE, movieId } from './library-helpers';

test.describe('rename (AC-13, AC-16)', () => {
  test('the preview shows old and new name; after confirming the library has the new name', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIVE, 'Neuer Name');
    await page.goto(`${LIVE}/filme/${id}`);
    await expect(page.getByTestId('file-path')).toHaveText('alter.name.mkv');
    await page.getByRole('button', { name: 'Umbenennen …' }).click();
    const dialog = page.getByRole('dialog', { name: 'Dateien umbenennen' });
    const preview = dialog.getByTestId('rename-preview');
    await expect(preview).toContainText('alter.name.mkv');
    await expect(preview).toContainText('→ Neuer Name (2001) Bluray-1080p.mkv');
    await dialog.getByRole('button', { name: 'Umbenennen' }).click();
    await expect(page.getByText('1 Datei umbenannt.')).toBeVisible();
    await expect(page.getByTestId('file-path')).toHaveText('Neuer Name (2001) Bluray-1080p.mkv');
    await page.getByRole('button', { name: 'Umbenennen …' }).click();
    await expect(page.getByRole('dialog')).toContainText(
      'Alle Dateinamen entsprechen dem Benennungsschema',
    );
  });

  test('a rename whose target exists is refused with a German message naming the file', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIVE, 'Ziel Belegt');
    await page.goto(`${LIVE}/filme/${id}`);
    await page.getByRole('button', { name: 'Umbenennen …' }).click();
    await page.getByRole('dialog').getByRole('button', { name: 'Umbenennen' }).click();
    const alert = page.getByRole('alert').filter({ hasText: 'Umbenennen nicht ausgeführt' });
    await expect(alert).toContainText('Ziel Belegt (2005) Bluray-1080p.mkv');
    await expect(page.getByTestId('file-path')).toHaveText('x.mkv');
  });
});

test.describe('folder (AC-15, AC-16)', () => {
  test('a differing folder shows old and new; after confirming it moves', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIVE, 'Neuer Titel');
    await page.goto(`${LIVE}/filme/${id}`);
    const differs = page.getByTestId('folder-differs');
    await expect(differs).toContainText('Alter Titel (2002)');
    await expect(differs).toContainText('Neuer Titel (2002)');
    await page.getByRole('button', { name: 'Ordner verschieben …' }).click();
    const dialog = page.getByRole('dialog', { name: 'Ordner verschieben' });
    await expect(dialog).toContainText('→');
    await dialog.getByRole('button', { name: 'Verschieben' }).click();
    await expect(page.getByText(/Ordner verschoben nach .*Neuer Titel \(2002\)/)).toBeVisible();
    await expect(page.getByTestId('folder')).toContainText('Neuer Titel (2002)');
    await expect(page.getByTestId('folder-differs')).toHaveCount(0);
  });

  test('a move whose target exists is refused; nothing moves', async ({ page, request }) => {
    const id = await movieId(request, LIVE, 'Blockiert');
    await page.goto(`${LIVE}/filme/${id}`);
    await page.getByRole('button', { name: 'Ordner verschieben …' }).click();
    await page.getByRole('dialog').getByRole('button', { name: 'Verschieben' }).click();
    const alert = page
      .getByRole('alert')
      .filter({ hasText: 'Ordner verschieben nicht ausgeführt' });
    await expect(alert).toContainText('Blockiert (2003)');
    await expect(page.getByTestId('folder')).toContainText('Blockiert alt (2003)');
  });
});

test.describe('remove (AC-12)', () => {
  test('with "Dateien löschen" the movie leaves the library and its file is removed', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIVE, 'Weg Damit');
    await page.goto(`${LIVE}/filme/${id}`);
    await page.getByRole('button', { name: 'Entfernen …' }).click();
    const dialog = page.getByRole('dialog', { name: '„Weg Damit“ entfernen' });
    await dialog.getByLabel(/Dateien löschen/).check();
    await dialog.getByRole('button', { name: 'Entfernen' }).click();
    await expect(page).toHaveURL(`${LIVE}/filme`);
    await expect(
      page.getByText('„Weg Damit“ wurde aus der Bibliothek entfernt. 1 Datei gelöscht.'),
    ).toBeVisible();
    const gone = await request.get(`${LIVE}/api/library/movies/${id}`);
    expect(gone.status()).toBe(404);
  });

  test('without files the movie leaves the library and no file is touched', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIVE, 'Ohne Dateien Weg');
    await page.goto(`${LIVE}/filme/${id}`);
    await page.getByRole('button', { name: 'Entfernen …' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByLabel(/Dateien löschen/)).toBeEnabled();
    await dialog.getByRole('button', { name: 'Entfernen' }).click();
    await expect(
      page.getByText('„Ohne Dateien Weg“ wurde aus der Bibliothek entfernt.'),
    ).toBeVisible();
    await expect(page.getByText('Datei gelöscht')).toHaveCount(0);
  });
});
