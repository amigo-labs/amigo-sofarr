// spec web-ui-library AC-02, AC-03, AC-06 (lists), AC-10, AC-22 against the seeded
// library server (2,501 movies, 23 series; e2e/seed.mjs).

import { expect, test } from '@playwright/test';
import { expectNoHorizontalScroll } from './helpers';
import { LIBRARY, apiTotal, expectTotal, items } from './library-helpers';

test.describe('movie list (AC-02, AC-03)', () => {
  test('shows the movies as a list with poster, title, year, quality and labels', async ({
    page,
    request,
  }) => {
    const total = await apiTotal(request, '/api/library/movies');
    await page.goto(`${LIBRARY}/filme`);
    await expectTotal(page, total, 'Filme');
    const boot = items(page).filter({ hasText: 'Das Boot' });
    await expect(boot).toContainText('1981');
    await expect(boot).toContainText('Bluray-1080p');
    await expect(boot.getByRole('button', { name: 'Überwachen „Das Boot“' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    const missing = items(page).filter({ hasText: 'Einzeln Überwachen' });
    await expect(missing).toContainText('fehlt');
    await expect(page.getByRole('button', { name: 'Liste' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  test('loads further movies while scrolling until every match is shown', async ({
    page,
    request,
  }) => {
    const total = await apiTotal(request, '/api/library/movies?has_file=no');
    expect(total).toBeGreaterThan(100);
    await page.goto(`${LIBRARY}/filme?datei=nein`);
    await expectTotal(page, total, 'Filme');
    await expect(items(page)).toHaveCount(50);
    while ((await items(page).count()) < total) {
      const before = await items(page).count();
      await items(page).last().scrollIntoViewIfNeeded();
      await expect.poll(() => items(page).count()).toBeGreaterThan(before);
    }
    await expect(items(page)).toHaveCount(total);
    await expect(page.getByRole('button', { name: 'Weitere laden' })).toHaveCount(0);
  });

  test('the poster grid is remembered in the browser', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme?suche=Boot`);
    await page.getByRole('button', { name: 'Poster' }).click();
    await expect(page.getByTestId('library-items')).toHaveClass(/grid/);
    await page.reload();
    await expect(page.getByRole('button', { name: 'Poster' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await expect(page.getByTestId('library-items')).toHaveClass(/grid/);
    await page.goto(`${LIBRARY}/serien`);
    await expect(page.getByRole('button', { name: 'Liste' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  test('filters combine with AND, are part of the address and survive a reload', async ({
    page,
    request,
  }) => {
    const expected = await apiTotal(
      request,
      '/api/library/movies?monitored=yes&missing=yes&has_file=no&tag=1&profile=4',
    );
    await page.goto(`${LIBRARY}/filme`);
    await page.getByLabel('Überwacht', { exact: true }).selectOption({ label: 'Ja' });
    await page.getByLabel('Fehlt', { exact: true }).selectOption({ label: 'Ja' });
    await page.getByLabel('Mit Datei', { exact: true }).selectOption({ label: 'Nein' });
    await page.getByLabel('Tag', { exact: true }).selectOption({ label: 'kinder' });
    await page.getByLabel('Qualitätsprofil', { exact: true }).selectOption({ label: '1080p' });
    await expect(page).toHaveURL(/ueberwacht=ja/);
    await expect(page).toHaveURL(/fehlt=ja/);
    await expect(page).toHaveURL(/datei=nein/);
    await expect(page).toHaveURL(/tag=1/);
    await expect(page).toHaveURL(/profil=4/);
    await expectTotal(page, expected, expected === 1 ? 'Film' : 'Filme');
    const shared = page.url();
    await page.goto(shared);
    await expectTotal(page, expected, expected === 1 ? 'Film' : 'Filme');
    await expect(page.getByLabel('Mit Datei', { exact: true })).toHaveValue('no');
  });

  test('cutoff filter matches the API', async ({ page, request }) => {
    const expected = await apiTotal(request, '/api/library/movies?cutoff_unmet=yes');
    await page.goto(`${LIBRARY}/filme?cutoff=ja`);
    await expectTotal(page, expected, 'Filme');
  });

  test('searches original and alternative titles', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme`);
    await page.getByLabel('Suche').fill('The Boat');
    await expect(page).toHaveURL(/suche=The\+Boat/);
    await expectTotal(page, 1, 'Film');
    await expect(items(page)).toHaveText(/Das Boot/);
    await page.getByLabel('Suche').fill('kein solcher Titel');
    await expect(page.getByText('Keine Filme passen zu Suche und Filtern.')).toBeVisible();
  });

  test('sorts by year descending, kept in the address', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme?suche=Film%20000`);
    await page.getByLabel('Sortierung', { exact: true }).selectOption({ label: 'Jahr' });
    await page.getByLabel('Reihenfolge', { exact: true }).selectOption({ label: 'absteigend' });
    await expect(page).toHaveURL(/sortierung=jahr/);
    await expect(page).toHaveURL(/richtung=ab/);
    // Film 0001..0009: years 1951..1959, newest first.
    await expect(items(page).first()).toContainText('Film 0009');
    await page.reload();
    await expect(items(page).first()).toContainText('Film 0009');
  });
});

test.describe('series list (AC-06)', () => {
  test('shows files and missing counts, filters by complete', async ({ page, request }) => {
    await page.goto(`${LIBRARY}/serien`);
    await expectTotal(page, await apiTotal(request, '/api/library/series'), 'Serien');
    const insel = items(page).filter({ hasText: 'Kleine Insel' });
    await expect(insel).toContainText('1 von 5 Dateien');
    await expect(insel).toContainText('1 fehlen');
    await expect(insel).toContainText('1 unter Cutoff');
    const complete = await apiTotal(request, '/api/library/series?complete=yes');
    await page.getByLabel('Komplett', { exact: true }).selectOption({ label: 'Ja' });
    await expect(page).toHaveURL(/komplett=ja/);
    await expectTotal(page, complete, complete === 1 ? 'Serie' : 'Serien');
    await expect(items(page).filter({ hasText: 'Kleine Insel' })).toHaveCount(0);
    await expect(page.getByLabel('Mit Datei', { exact: true })).toHaveCount(0);
  });
});

test.describe('monitoring from the list (AC-10)', () => {
  test('one item is saved at once', async ({ page, request }) => {
    await page.goto(`${LIBRARY}/filme?suche=Einzeln`);
    const button = page.getByRole('button', { name: 'Überwachen „Einzeln Überwachen“' });
    await expect(button).toHaveAttribute('aria-pressed', 'false');
    await button.click();
    await expect(button).toHaveAttribute('aria-pressed', 'true');
    expect(await apiTotal(request, '/api/library/movies?q=Einzeln&monitored=yes')).toBe(1);
  });

  test('several items after one confirmation naming their number', async ({ page, request }) => {
    let requests = 0;
    page.on('request', (r) => {
      if (r.url().endsWith('/api/library/monitor')) requests += 1;
    });
    await page.goto(`${LIBRARY}/filme?suche=Sammel`);
    await expectTotal(page, 3, 'Filme');
    for (const letter of ['A', 'B', 'C']) {
      await page.getByLabel(`„Sammel ${letter}“ auswählen`).check();
    }
    await page
      .getByRole('region', { name: 'Auswahl' })
      .getByRole('button', { name: 'Überwachen', exact: true })
      .click();
    const dialog = page.getByRole('dialog', { name: 'Auswahl überwachen' });
    await expect(dialog).toContainText('3 Filme überwachen?');
    await dialog.getByRole('button', { name: '3 Filme überwachen' }).click();
    await expect(page.getByText('3 Filme werden überwacht.')).toBeVisible();
    expect(requests).toBe(1);
    expect(await apiTotal(request, '/api/library/movies?q=Sammel&monitored=yes')).toBe(3);
  });

  test('a failing bulk change saves none and says so', async ({ page }) => {
    await page.route('**/api/library/monitor', (route) =>
      route.fulfill({
        status: 404,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'Film 999999 nicht gefunden – nichts geändert' }),
      }),
    );
    await page.goto(`${LIBRARY}/filme?suche=Film%20001`);
    await page.getByLabel('„Film 0010“ auswählen').check();
    await page.getByLabel('„Film 0011“ auswählen').check();
    await page
      .getByRole('region', { name: 'Auswahl' })
      .getByRole('button', { name: 'Nicht überwachen' })
      .click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: '2 Filme nicht überwachen' })
      .click();
    await expect(page.getByRole('alert')).toContainText(
      'Nichts geändert: Film 999999 nicht gefunden',
    );
  });
});

test.describe('narrow screens (AC-22)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  for (const [name, path] of [
    ['movie list', '/filme'],
    ['movie grid', '/filme?suche=Film%2000'],
    ['series list', '/serien'],
  ] as const) {
    test(`the ${name} shows cards without horizontal scrolling`, async ({ page }) => {
      await page.goto(`${LIBRARY}${path}`);
      await expect(items(page).first()).toBeVisible();
      if (name === 'movie grid') await page.getByRole('button', { name: 'Poster' }).click();
      await expectNoHorizontalScroll(page);
      if (name !== 'movie grid') {
        // Each entry is a card of its own.
        const card = items(page).first();
        const border = await card.evaluate((el) => getComputedStyle(el).borderTopStyle);
        expect(border).toBe('solid');
      }
    });
  }
});
