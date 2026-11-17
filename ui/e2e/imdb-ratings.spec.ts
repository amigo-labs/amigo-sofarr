// spec imdb-ratings AC-04, AC-06 to AC-09 in the browser: the `ratings` server loads a local
// dataset at start (e2e/seed-extras.mjs); the `extras` server has ratings switched off.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { expectAttribution, expectNoHorizontalScroll } from './helpers';
import { EXTRAS, RATINGS, waitForRatings } from './extras-helpers';
import { expectTotal, items, movieId, openFilters, seriesId } from './library-helpers';

/** The compact rating of the lists, with votes in tooltip and name (ui-list-items AC-03). */
async function expectRating(rating: Locator, full: string, compact: string): Promise<void> {
  await expect(rating).toHaveText(compact);
  await expect(rating).toHaveAccessibleName(full);
  await expect(rating).toHaveAttribute('title', full);
}

const titles = (page: Page) => items(page).locator('a.title').allTextContents();

test.describe('ratings in the lists (AC-04, AC-08)', () => {
  test.beforeEach(async ({ request }) => {
    await waitForRatings(request);
  });

  test('every movie shows rating and votes, "–" without; attribution with the date', async ({
    page,
  }) => {
    await page.goto(`${RATINGS}/filme`);
    await expectTotal(page, 6, 'Filme');
    const rating = (title: string) => items(page).filter({ hasText: title }).getByTestId('rating');
    await expectRating(rating('Bestnote'), 'IMDb 9,1 (250.000 Stimmen)', '★ 9,1');
    await expectRating(rating('Nischenfilm'), 'IMDb 9,8 (12 Stimmen)', '★ 9,8');
    await expectRating(rating('Mittelmaß'), 'IMDb 5,2 (80.000 Stimmen)', '★ 5,2');
    await expectRating(rating('Ohne IMDb'), 'IMDb –', '–');
    await expectRating(rating('Unbewertet'), 'IMDb –', '–');
    await expectAttribution(page);

    await page.getByRole('button', { name: 'Poster' }).click();
    await expect(page.getByTestId('library-items')).toHaveClass(/grid/);
    await expectRating(rating('Gutfilm'), 'IMDb 7,5 (12.000 Stimmen)', '★ 7,5');
  });

  test('every series shows rating and votes, "–" without', async ({ page }) => {
    await page.goto(`${RATINGS}/serien`);
    await expectTotal(page, 3, 'Serien');
    const rating = (title: string) => items(page).filter({ hasText: title }).getByTestId('rating');
    await expectRating(rating('Hochserie'), 'IMDb 8,8 (50.000 Stimmen)', '★ 8,8');
    await expectRating(rating('Tiefserie'), 'IMDb 6,0 (3.000 Stimmen)', '★ 6,0');
    await expectRating(rating('Serie ohne IMDb'), 'IMDb –', '–');
    await expectAttribution(page);
  });

  test('cards on narrow screens show the rating', async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 740 });
    await page.goto(`${RATINGS}/filme`);
    await expectRating(
      items(page).filter({ hasText: 'Bestnote' }).getByTestId('rating'),
      'IMDb 9,1 (250.000 Stimmen)',
      '★ 9,1',
    );
    await expectNoHorizontalScroll(page);
  });
});

test.describe('ratings on the pages (AC-04, AC-08)', () => {
  test.beforeEach(async ({ request }) => {
    await waitForRatings(request);
  });

  test('a movie and a series show rating and votes with the attribution', async ({
    page,
    request,
  }) => {
    await page.goto(`${RATINGS}/filme/${await movieId(request, RATINGS, 'Bestnote')}`);
    await expect(page.getByTestId('rating')).toHaveText(
      /IMDb-Bewertung\s+9,1\s+\(250\.000 Stimmen\)/,
    );
    await expectAttribution(page);

    await page.goto(`${RATINGS}/serien/${await seriesId(request, RATINGS, 'Hochserie')}`);
    await expect(page.getByTestId('rating')).toHaveText(
      /IMDb-Bewertung\s+8,8\s+\(50\.000 Stimmen\)/,
    );
    await expectAttribution(page);
  });

  test('titles without an IMDb id or without a rating show "–"', async ({ page, request }) => {
    for (const title of ['Ohne IMDb', 'Unbewertet']) {
      await page.goto(`${RATINGS}/filme/${await movieId(request, RATINGS, title)}`);
      await expect(page.getByTestId('rating')).toHaveText(/^IMDb-Bewertung\s+–\s*$/);
    }
    await page.goto(`${RATINGS}/serien/${await seriesId(request, RATINGS, 'Serie ohne IMDb')}`);
    await expect(page.getByTestId('rating')).toHaveText(/^IMDb-Bewertung\s+–\s*$/);
  });
});

test.describe('sorting and minimums (AC-06, AC-07)', () => {
  test.beforeEach(async ({ request }) => {
    await waitForRatings(request);
  });

  test('sorting by rating puts the best first and titles without a rating last', async ({
    page,
  }) => {
    await page.goto(`${RATINGS}/filme`);
    await expectTotal(page, 6, 'Filme');
    await page.getByLabel('Sortierung').selectOption({ label: 'Bewertung' });
    await expect(page).toHaveURL(/sortierung=bewertung/);
    await expect
      .poll(async () => (await titles(page)).slice(0, 4))
      .toEqual(['Nischenfilm', 'Bestnote', 'Gutfilm', 'Mittelmaß']);
    expect((await titles(page)).slice(4).toSorted()).toEqual(['Ohne IMDb', 'Unbewertet']);

    await page.getByRole('button', { name: 'Absteigend sortieren' }).click();
    await expect(page).toHaveURL(/richtung=ab/);
    await expect
      .poll(async () => (await titles(page)).slice(0, 4))
      .toEqual(['Mittelmaß', 'Gutfilm', 'Bestnote', 'Nischenfilm']);
    expect((await titles(page)).slice(4).toSorted()).toEqual(['Ohne IMDb', 'Unbewertet']);
  });

  test('minimum rating and votes combine, are part of the address and survive a reload', async ({
    page,
  }) => {
    await page.goto(`${RATINGS}/filme`);
    await expectTotal(page, 6, 'Filme');
    await openFilters(page);
    await page.getByLabel('Mindestbewertung', { exact: true }).selectOption({ label: 'ab 7,0' });
    await expect(page).toHaveURL(/mindestbewertung=7(&|$)/);
    await expectTotal(page, 3, 'Filme');
    expect((await titles(page)).toSorted()).toEqual(['Bestnote', 'Gutfilm', 'Nischenfilm']);

    await page.getByLabel('Mindeststimmen', { exact: true }).selectOption({ label: 'ab 1.000' });
    await expect(page).toHaveURL(/mindeststimmen=1000/);
    await expectTotal(page, 2, 'Filme');
    expect((await titles(page)).toSorted()).toEqual(['Bestnote', 'Gutfilm']);

    await page.reload();
    await expectTotal(page, 2, 'Filme');
    await expect(page.getByLabel('Mindestbewertung', { exact: true })).toHaveValue('7');
    await expect(page.getByLabel('Mindeststimmen', { exact: true })).toHaveValue('1000');

    await openFilters(page);
    await page.getByLabel('Mindestbewertung', { exact: true }).selectOption({ label: 'Alle' });
    await page.getByLabel('Mindeststimmen', { exact: true }).selectOption({ label: 'Alle' });
    await expectTotal(page, 6, 'Filme');
  });

  test('a minimum from the address that is not offered is kept and shown', async ({ page }) => {
    await page.goto(`${RATINGS}/serien?mindestbewertung=8.5`);
    await expectTotal(page, 1, 'Serie');
    await expect(items(page)).toContainText(['Hochserie']);
    await expect(page.getByLabel('Mindestbewertung', { exact: true })).toHaveValue('8.5');
  });
});

test.describe('ratings switched off (AC-09)', () => {
  test('lists offer no rating, sort or minimum, and old addresses still work', async ({ page }) => {
    const told = page.waitForResponse('**/api/imdb');
    await page.goto(`${EXTRAS}/filme?sortierung=bewertung&mindestbewertung=7&mindeststimmen=10`);
    expect(((await (await told).json()) as { enabled: boolean }).enabled).toBe(false);
    await expectTotal(page, 5, 'Filme');
    await expect(page.getByLabel('Sortierung').locator('option')).toHaveText([
      'Titel',
      'Jahr',
      'Hinzugefügt',
      'Größe',
    ]);
    await expect(page.getByLabel('Mindestbewertung', { exact: true })).toHaveCount(0);
    await expect(page.getByLabel('Mindeststimmen', { exact: true })).toHaveCount(0);
    await expect(page.getByTestId('rating')).toHaveCount(0);
    await expect(page.getByText('Information courtesy of IMDb')).toHaveCount(0);

    await page.goto(`${EXTRAS}/serien`);
    await expectTotal(page, 1, 'Serie');
    await expect(page.getByTestId('rating')).toHaveCount(0);
  });

  test('pages show no rating', async ({ page, request }) => {
    const told = page.waitForResponse('**/api/imdb');
    await page.goto(`${EXTRAS}/filme/${await movieId(request, EXTRAS, 'Untertitelfilm')}`);
    await expect(page.getByRole('heading', { level: 1, name: /Untertitelfilm/ })).toBeVisible();
    await expect(page.getByTestId('trailer')).toBeVisible();
    await told;
    await expect(page.getByTestId('rating')).toHaveCount(0);
    await expect(page.getByText('Information courtesy of IMDb')).toHaveCount(0);
  });
});
