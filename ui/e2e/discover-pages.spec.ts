// Discover, search, previews and the cast of library titles (spec discover AC-01..AC-04,
// AC-06, AC-13, AC-14, AC-17; imdb-ratings AC-04, AC-08) against the real binary with a fake
// TMDB (`fake-tmdb.mjs`). Titles added by `discover-add.spec.ts` are not used here.

import { expect, test } from '@playwright/test';
import { DISCOVER, PLAIN, cards, release, settle } from './discover-helpers';
import { LIBRARY, movieId, seriesId } from './library-helpers';

const SHELVES = [
  'Trending heute',
  'Trending diese Woche',
  'Beliebte Filme',
  'Demnächst im Kino',
  'Jetzt im Kino',
  'Neu als Stream oder Disc',
  'Beliebte Serien',
];

test.describe('discover page (AC-01, AC-13, AC-17)', () => {
  test('shows the shelves in order with up to 20 titles, ratings and library state', async ({
    page,
  }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    const shelves = page.getByTestId('shelf');
    await expect(shelves.getByRole('heading', { level: 2 })).toHaveText(SHELVES);
    for (const shelf of await shelves.all()) {
      expect(await shelf.getByTestId('title-card').count()).toBeLessThanOrEqual(20);
    }

    const trending = shelves.first();
    await expect(trending.getByTestId('title-card')).toHaveCount(20);
    const boat = cards(trending, 'movie/501');
    await expect(boat.getByText('In der Bibliothek')).toBeVisible();
    const boatId = await movieId(page.request, DISCOVER, 'Das Boot');
    await expect(boat.getByRole('link', { name: 'Das Boot' })).toHaveAttribute(
      'href',
      `/filme/${boatId}`,
    );
    await expect(boat.getByRole('button', { name: /Hinzufügen/ })).toHaveCount(0);
    const star = cards(trending, 'movie/502');
    await expect(star).toContainText('Film · 2024');
    await expect(star.getByRole('link', { name: 'Sternenfahrt' })).toHaveAttribute(
      'href',
      '/entdecken/filme/502',
    );
    await expect(star.getByRole('button', { name: 'Hinzufügen „Sternenfahrt“' })).toBeVisible();
    await expect(star.getByTestId('rating')).toHaveText('IMDb 7,8 (12.345 Stimmen)');
    const unrated = cards(page, 'movie/505').first();
    await expect(unrated.getByTestId('rating')).toHaveText('IMDb –');

    // A title whose IMDb id is still loading shows the placeholder until it is known.
    const slow = cards(trending, 'movie/504').getByTestId('rating');
    await expect(slow).toHaveText('IMDb … (Bewertung folgt)');
    await release(page.request, 'movie', 504);
    await expect(slow).toHaveText('IMDb 5,0 (1.000 Stimmen)', { timeout: 15_000 });

    await expect(page.getByTestId('fetched').first()).toContainText('Stand: ');
    await expect(page.getByRole('link', { name: /Information courtesy of IMDb/ })).toHaveAttribute(
      'href',
      'https://www.imdb.com',
    );
    await expect(page.getByText(/Stand der Bewertungen:/)).toBeVisible();
  });

  test('"Mehr" opens the whole shelf, which loads more while scrolling', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await page.getByRole('link', { name: 'Mehr aus „Beliebte Filme“' }).click();
    await expect(page).toHaveURL(/\/entdecken\/regal\/beliebte-filme$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Beliebte Filme' })).toBeVisible();
    const tiles = page.getByTestId('title-card');
    await expect(tiles).toHaveCount(20);
    await expect
      .poll(async () => {
        await tiles.last().scrollIntoViewIfNeeded();
        return tiles.count();
      })
      .toBe(62);
    await expect(page.getByRole('button', { name: 'Weitere laden' })).toHaveCount(0);
  });

  test('every image comes from sofarr (AC-17)', async ({ page }) => {
    const images: string[] = [];
    page.on('request', (r) => {
      if (r.resourceType() === 'image') images.push(r.url());
    });
    await page.goto(`${DISCOVER}/entdecken`);
    await expect(page.getByTestId('shelf')).toHaveCount(7);
    await page.goto(`${DISCOVER}/entdecken/personen/9000`);
    await expect(page.getByRole('heading', { level: 1, name: 'Anna Beispiel' })).toBeVisible();
    await page.goto(`${DISCOVER}/entdecken/filme/502`);
    await expect(page.getByRole('heading', { level: 1, name: /Sternenfahrt/ })).toBeVisible();
    await expect.poll(() => images.length).toBeGreaterThan(2);
    expect(images.filter((url) => !url.startsWith(`${DISCOVER}/api/images/`))).toEqual([]);
  });

  test('a stale answer shows the German notice with its age (AC-14)', async ({ page }) => {
    await page.route('**/api/discover/shelves', async (route) => {
      const answer = await route.fetch();
      const body = (await answer.json()) as Record<string, unknown>;
      body['freshness'] = {
        fetched_at: '2026-11-10T08:00:00Z',
        stale: true,
        notice: 'TMDB ist nicht erreichbar – Stand von vor 2 Tagen.',
      };
      await route.fulfill({ response: answer, json: body });
    });
    await page.goto(`${DISCOVER}/entdecken`);
    await expect(page.getByTestId('stale-notice')).toHaveText(
      'TMDB ist nicht erreichbar – Stand von vor 2 Tagen.',
    );
    await expect(page.getByTestId('fetched')).toHaveText('Stand: 10.11.2026, 09:00:00');
    await expect(page.getByTestId('shelf')).toHaveCount(7);
  });

  test('without cached data a German message replaces the page (AC-14)', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken/personen/9999`);
    await expect(page.getByRole('heading', { name: 'TMDB ist nicht verfügbar' })).toBeVisible();
    await expect(
      page.getByText(/TMDB ist nicht erreichbar, und es liegen noch keine/),
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Erneut versuchen' })).toBeVisible();
  });
});

test.describe('filters (AC-02)', () => {
  test('a filter replaces the shelves with one list that keeps 20 titles at the rating', async ({
    page,
  }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await expect(page.getByTestId('shelf')).toHaveCount(7);
    await page.getByLabel('Art').selectOption({ label: 'Filme' });
    await page.getByLabel('Genre').selectOption({ label: 'Dokumentarfilm' });
    await page.getByLabel('IMDb-Bewertung').selectOption({ label: 'ab 7,0' });
    await expect(page).toHaveURL(/\?art=filme&genre=99&bewertung=7$/);
    await expect(page.getByTestId('shelf')).toHaveCount(0);
    const results = page.getByRole('list', { name: 'Ergebnisse' });

    // Still loading its rating, the title stays with the placeholder; at 5.0 it goes.
    const slow = cards(results, 'movie/507');
    await expect(slow.getByTestId('rating')).toHaveText('IMDb … (Bewertung folgt)');
    await release(page.request, 'movie', 507);
    await expect(slow).toHaveCount(0, { timeout: 15_000 });
    await expect
      .poll(() => results.getByTestId('title-card').count(), { timeout: 15_000 })
      .toBeGreaterThanOrEqual(20);
    for (const rating of await results.getByTestId('rating').allTextContents()) {
      expect(rating).toMatch(/^IMDb (7|8|9),\d|^IMDb …/);
    }

    await page.getByRole('button', { name: 'Filter zurücksetzen' }).click();
    await expect(page).toHaveURL(`${DISCOVER}/entdecken`);
    await expect(page.getByTestId('shelf')).toHaveCount(7);
  });

  test('the page address holds the filters: "Serien" shows only series', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken?art=serien`);
    await expect(page.getByLabel('Art')).toHaveValue('series');
    const results = page.getByRole('list', { name: 'Ergebnisse' }).getByTestId('title-card');
    await expect(results.first()).toBeVisible();
    for (const key of await results.evaluateAll((els) => els.map((e) => e.dataset['key']))) {
      expect(key).toMatch(/^series\//);
    }
  });

  test('without ratings no rating filter, rating or attribution is offered', async ({ page }) => {
    await page.goto(`${PLAIN}/entdecken?bewertung=7`);
    await expect(page.getByTestId('shelf')).toHaveCount(7);
    await expect(page.getByLabel('Genre')).toBeVisible();
    await expect(page.getByLabel('IMDb-Bewertung')).toHaveCount(0);
    await expect(page.getByTestId('rating')).toHaveCount(0);
    await expect(page.getByText(/Information courtesy of IMDb/)).toHaveCount(0);
  });
});

test.describe('search and previews (AC-03, AC-04)', () => {
  test('search shows movies, series and people in three groups', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await page.getByLabel('Suchbegriff').fill('Stern');
    await page.getByRole('button', { name: 'Suchen' }).click();
    await expect(page).toHaveURL(/\/entdecken\/suche\?begriff=Stern$/);
    await expect(page.getByRole('heading', { level: 2 })).toHaveText([
      'Filme',
      'Serien',
      'Personen',
    ]);
    await expect(page.getByRole('list', { name: 'Filme' })).toContainText('Sternenfahrt');
    await expect(page.getByRole('list', { name: 'Serien' })).toContainText('Sternstunden');
    await expect(page.getByRole('link', { name: 'Stella Stern' })).toHaveAttribute(
      'href',
      '/entdecken/personen/9200',
    );
  });

  test('search results carry their library state', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken/suche?begriff=Insel`);
    const island = cards(page.getByRole('list', { name: 'Serien' }), 'series/603');
    await expect(island.getByText('In der Bibliothek')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('Keine Filme gefunden.')).toBeVisible();
  });

  test('a title outside the library opens its preview', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken/suche?begriff=Sternenfahrt`);
    await page.getByRole('link', { name: 'Sternenfahrt' }).click();
    await expect(page).toHaveURL(/\/entdecken\/filme\/502$/);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Sternenfahrt (2024)');
    await expect(page.getByText('Originaltitel: Star Voyage')).toBeVisible();
    await expect(page.getByText('1 Std. 45 Min.')).toBeVisible();
    await expect(page.getByTestId('rating')).toHaveText('IMDb 7,8 (12.345 Stimmen)');
    await expect(page.getByRole('link', { name: 'Trailer auf YouTube' })).toHaveAttribute(
      'href',
      'https://www.youtube.com/watch?v=e2e502',
    );
    await expect(page.getByTestId('fetched')).toContainText('Stand: ');
    await page
      .getByRole('list', { name: 'Besetzung' })
      .getByRole('link', { name: 'Anna Beispiel' })
      .click();
    await expect(page.getByRole('heading', { level: 1, name: 'Anna Beispiel' })).toBeVisible();
  });

  test('a title in the library opens its library page', async ({ page }) => {
    const boat = await movieId(page.request, DISCOVER, 'Das Boot');
    await page.goto(`${DISCOVER}/entdecken/filme/501`);
    await expect(page).toHaveURL(`${DISCOVER}/filme/${boat}`);
    const island = await seriesId(page.request, DISCOVER, 'Kleine Insel');
    await page.goto(`${DISCOVER}/entdecken/serien/603`);
    await expect(page).toHaveURL(`${DISCOVER}/serien/${island}`);
  });
});

test.describe('cast on the library pages (AC-06)', () => {
  test('a movie shows its first 10 cast members with links to their pages', async ({ page }) => {
    await page.goto(`${DISCOVER}/filme/${await movieId(page.request, DISCOVER, 'Das Boot')}`);
    const cast = page.getByRole('list', { name: 'Besetzung' });
    await expect(cast.getByRole('link')).toHaveCount(10);
    await expect(cast.getByRole('link', { name: 'Anna Beispiel' })).toHaveAttribute(
      'href',
      '/entdecken/personen/9000',
    );
    await expect(cast).toContainText('Rolle 1');
  });

  test('a series without TMDB id and a server without TMDB show no cast', async ({ page }) => {
    await page.goto(`${DISCOVER}/serien/${await seriesId(page.request, DISCOVER, 'Kleine Insel')}`);
    await expect(page.getByRole('heading', { name: 'Staffeln' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Besetzung' })).toHaveCount(0);
    await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, 'Das Boot')}`);
    await expect(page.getByRole('heading', { level: 1, name: /Das Boot/ })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Besetzung' })).toHaveCount(0);
    await expect(page.getByTestId('cast-failure')).toHaveCount(0);
  });
});

test.describe('without a TMDB key (AC-15)', () => {
  for (const path of [
    '/entdecken',
    '/entdecken/suche?begriff=Stern',
    '/entdecken/filme/502',
    '/entdecken/personen/9000',
    '/entdecken/regal/beliebte-filme',
  ]) {
    test(`${path} explains what is missing and offers nothing`, async ({ page }) => {
      await page.goto(`${LIBRARY}${path}`);
      await settle(page);
      const box = page.getByTestId('discover-unavailable');
      await expect(box).toContainText('Entdecken ist noch nicht eingerichtet.');
      await expect(box).toContainText('[tmdb]');
      await expect(page.getByRole('search')).toHaveCount(0);
      await expect(page.getByRole('button', { name: /Hinzufügen|Import-Liste/ })).toHaveCount(0);
      await expect(page.getByTestId('title-card')).toHaveCount(0);
    });
  }
});
