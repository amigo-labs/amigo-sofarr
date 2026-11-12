// Person pages (spec discover AC-05): cast and crew departments, sorting by year and IMDb
// rating, "nur fehlende", movies or series, library state and paging of long lists.

import { expect, test } from '@playwright/test';
import { DISCOVER, PLAIN, cards } from './discover-helpers';

const castSection = (page: import('@playwright/test').Page) =>
  page.getByTestId('credits').filter({ has: page.getByRole('heading', { name: /^Darsteller/ }) });

test.describe('person page (AC-05)', () => {
  test('lists cast and crew departments with library state, in pages', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken/personen/9000`);
    await expect(page.getByRole('heading', { level: 1, name: 'Anna Beispiel' })).toBeVisible();
    await expect(page.getByText('Hamburg, Deutschland')).toBeVisible();
    await expect(page.getByRole('heading', { level: 2 })).toHaveText([
      'Darsteller (65)',
      'Regie (1)',
      'Drehbuch (1)',
    ]);
    const cast = castSection(page);
    await expect(cast.getByTestId('title-card')).toHaveCount(50);
    await expect(cards(cast, 'series/601')).toContainText('Kapitänin');
    await cast.getByRole('button', { name: 'Weitere anzeigen (15) in „Darsteller“' }).click();
    await expect(cast.getByTestId('title-card')).toHaveCount(65);
    await expect(cards(cast, 'movie/501').getByText('In der Bibliothek')).toBeVisible();
    const directing = page.getByTestId('credits').filter({ hasText: 'Regie' });
    await expect(cards(directing, 'movie/502')).toContainText('Regie');
  });

  test('sorts by IMDb rating once ratings are known, and filters', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken/personen/9000`);
    const cast = castSection(page);
    await page.getByLabel('Sortierung').selectOption({ label: 'IMDb-Bewertung' });
    await expect(page).toHaveURL(/sortierung=bewertung/);
    // The best rated fillers (8,9) lead once every rating is known; "–" comes last.
    await expect(cast.getByTestId('rating').first()).toHaveText('IMDb 8,9 (1.000 Stimmen)', {
      timeout: 15_000,
    });
    await cast.getByRole('button', { name: /Weitere anzeigen/ }).click();
    await expect(cast.getByTestId('rating').last()).toHaveText('IMDb –');

    await page.getByLabel('Sortierung').selectOption({ label: 'Jahr, neueste zuerst' });
    await expect(cast.getByTestId('title-card').first()).toContainText('· 2024');

    await page.getByLabel('nur fehlende').check();
    await expect(page).toHaveURL(/nur=fehlende/);
    await expect(cards(cast, 'movie/501')).toHaveCount(0);
    await expect(cards(cast, 'movie/505')).toHaveCount(1);

    await page.getByLabel('nur fehlende').uncheck();
    await page.getByLabel('Art').selectOption({ label: 'Serien' });
    await expect(page).toHaveURL(/art=serien/);
    await expect(cast.getByTestId('title-card')).toHaveCount(1);
    await expect(page.getByText('Keine Titel passen zur Auswahl.')).toHaveCount(2);
  });

  test('a person known for crew work only has an empty cast with a note', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken/personen/9100`);
    await expect(page.getByRole('heading', { level: 1, name: 'Clara Kamera' })).toBeVisible();
    await expect(page.getByText(/hat keine Rollen vor der Kamera/)).toBeVisible();
    const camera = page.getByTestId('credits').filter({ hasText: 'Kamera (2)' });
    await expect(camera.getByTestId('title-card')).toHaveCount(2);
  });

  test('without ratings there is no sorting by rating', async ({ page }) => {
    await page.goto(`${PLAIN}/entdecken/personen/9000?sortierung=bewertung`);
    await expect(page.getByRole('heading', { level: 1, name: 'Anna Beispiel' })).toBeVisible();
    await expect(page.getByLabel('Sortierung')).toHaveValue('neueste');
    await expect(page.getByLabel('Sortierung').locator('option')).toHaveText([
      'Jahr, neueste zuerst',
      'Jahr, älteste zuerst',
    ]);
  });
});
