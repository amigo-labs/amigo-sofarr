// Spec ui-cinema-library-rows: rows of poster tiles above the movie and series lists while
// the list is plain, the full list below under „Alle Filme“ or „Alle Serien“.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Locator, type Page, type Route } from '@playwright/test';
import type { ImdbInfo } from '../src/lib/api/generated/ImdbInfo';
import type { MovieListItem } from '../src/lib/api/generated/MovieListItem';
import type { Page as ListPage } from '../src/lib/api/generated/Page';
import type { SeriesListItem } from '../src/lib/api/generated/SeriesListItem';
import { expectNoHorizontalScroll } from './helpers';
import { RATINGS, waitForRatings } from './extras-helpers';
import { LIBRARY, items, movieId, openFilters, seriesId } from './library-helpers';
import { storeTheme } from './theme-helpers';

const ROWS = ['Zuletzt hinzugefügt', 'Fehlt noch', 'Unter Cutoff', 'Am besten bewertet'] as const;
type RowName = (typeof ROWS)[number];
type RowKey = 'added' | 'missing' | 'cutoff' | 'rated';
type Item = MovieListItem | SeriesListItem;
type Kind = 'movies' | 'series';

/** The API query of each row (the technical notes: one request per row, 20 titles). */
const QUERIES: Readonly<Record<RowKey, string>> = {
  added: 'sort=added&order=desc&page=1&page_size=20',
  missing: 'missing=yes&sort=added&order=desc&page=1&page_size=20',
  cutoff: 'cutoff_unmet=yes&sort=added&order=desc&page=1&page_size=20',
  rated: 'sort=rating&order=asc&page=1&page_size=20',
};

const row = (page: Page, name: RowName): Locator => page.getByRole('region', { name, exact: true });
const tiles = (page: Page, name: RowName): Locator =>
  page.getByRole('list', { name, exact: true }).locator(':scope > li');
const moreLink = (page: Page, name: RowName): Locator =>
  row(page, name).getByRole('link', { name: /^Alle anzeigen/ });
const sortOf = (page: Page): Locator => page.getByLabel('Sortierung');
const allRows = (page: Page): Locator => page.getByTestId('tile-row');
const toolbar = (page: Page): Locator => page.getByRole('toolbar', { name: 'Liste' });
const allHeading = (page: Page, name: string): Locator =>
  page.getByRole('heading', { level: 2, name, exact: true });

const hrefs = (scope: Locator): Promise<string[]> =>
  scope.evaluateAll((lis) => lis.map((li) => li.querySelector('a')?.getAttribute('href') ?? ''));

const hrefOf = (kind: Kind, item: Item): string =>
  `${kind === 'movies' ? '/filme' : '/serien'}/${item.id}`;

async function apiItems(page: Page, base: string, kind: Kind, query: string): Promise<Item[]> {
  const answer = await page.request.get(`${base}/api/library/${kind}?${query}`);
  return ((await answer.json()) as ListPage<Item>).items;
}

const top = async (scope: Locator): Promise<number> => (await scope.boundingBox())?.y ?? -1;
const bottom = async (scope: Locator): Promise<number> => {
  const box = await scope.boundingBox();
  return box === null ? -1 : box.y + box.height;
};

/** The kind and row of a row request (one page of 20); `null` for the list's own pages. */
function rowRequest(url: URL): { kind: Kind; key: RowKey } | null {
  if (url.searchParams.get('page_size') !== '20') return null;
  const kind: Kind = url.pathname.endsWith('/series') ? 'series' : 'movies';
  const query = url.searchParams.toString();
  const key = (Object.keys(QUERIES) as RowKey[]).find((k) => QUERIES[k] === query);
  return key === undefined ? null : { kind, key };
}

/**
 * Serves the row requests from `answer` (read at each request; `undefined` passes the
 * request on, `'fail'` answers with a server error) and counts them per row.
 */
async function serveRows(
  page: Page,
  answer: (kind: Kind, key: RowKey) => Item[] | 'fail' | undefined,
): Promise<Record<RowKey, number>> {
  const counts: Record<RowKey, number> = { added: 0, missing: 0, cutoff: 0, rated: 0 };
  await page.route(/\/api\/library\/(movies|series)\?/, async (route: Route) => {
    const request = rowRequest(new URL(route.request().url()));
    if (request === null) return route.fallback();
    counts[request.key] += 1;
    const served = answer(request.kind, request.key);
    if (served === undefined) return route.fallback();
    if (served === 'fail') {
      return route.fulfill({ status: 500, json: { error: 'Datenbank nicht erreichbar' } });
    }
    return route.fulfill({
      json: { page: 1, page_size: 20, total: served.length, items: served },
    });
  });
  return counts;
}

const withRating = (m: Item): Item => ({ ...m, imdb_rating: 7.5, imdb_votes: 900 });

/** Answers the ratings state as on, whatever the server says. */
async function ratingsOn(page: Page): Promise<void> {
  await page.route('**/api/imdb', (route) =>
    route.fulfill({
      json: {
        enabled: true,
        loaded_at: '2026-11-20T04:00:00Z',
        titles: 100_000,
        attribution: 'Information courtesy of IMDb (https://www.imdb.com). Used with permission.',
        link: 'https://www.imdb.com',
      } satisfies ImdbInfo,
    }),
  );
}

/** Switching monitoring succeeds without changing the shared library server. */
async function fakeMonitor(page: Page): Promise<void> {
  await page.route('**/api/library/monitor', async (route) => {
    const body = route.request().postDataJSON() as { items: unknown[] };
    return route.fulfill({ json: { changed: body.items.length } });
  });
}

test.describe('rows of the movie list (AC-01)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('four rows above the toolbar in this order', async ({ page }) => {
    const movies = await apiItems(page, LIBRARY, 'movies', QUERIES.added);
    await ratingsOn(page);
    const rated = movies.slice(0, 5).map(withRating);
    await serveRows(page, () => rated);
    await page.goto(`${LIBRARY}/filme`);
    for (const name of ROWS) await expect(row(page, name)).toBeVisible();
    const headings = await page.getByRole('heading', { level: 2 }).allTextContents();
    expect(headings.slice(0, 5)).toEqual([...ROWS, 'Alle Filme']);
    for (let i = 1; i < ROWS.length; i += 1) {
      expect(await top(row(page, ROWS[i] ?? 'Fehlt noch'))).toBeGreaterThan(
        await top(row(page, ROWS[i - 1] ?? 'Fehlt noch')),
      );
    }
    const h1 = page.getByRole('heading', { level: 1, name: 'Filme' });
    expect(await top(row(page, 'Zuletzt hinzugefügt'))).toBeGreaterThanOrEqual(await bottom(h1));
    expect(await top(toolbar(page))).toBeGreaterThan(await bottom(row(page, 'Am besten bewertet')));
  });

  test('the rows hold the movies of their filter and sort, at most 20', async ({ page }) => {
    const added = await apiItems(page, LIBRARY, 'movies', QUERIES.added);
    const missing = await apiItems(page, LIBRARY, 'movies', QUERIES.missing);
    const cutoff = await apiItems(page, LIBRARY, 'movies', QUERIES.cutoff);
    expect(added).toHaveLength(20);
    expect(missing.length).toBeGreaterThan(0);
    await page.goto(`${LIBRARY}/filme`);
    await expect(tiles(page, 'Zuletzt hinzugefügt')).toHaveCount(20);
    expect(await hrefs(tiles(page, 'Zuletzt hinzugefügt'))).toEqual(
      added.map((m) => hrefOf('movies', m)),
    );
    expect(await hrefs(tiles(page, 'Fehlt noch'))).toEqual(missing.map((m) => hrefOf('movies', m)));
    expect(missing.map((m) => m.id)).toContain(await movieId(page.request, LIBRARY, 'Fehlt Sehr'));
    if (cutoff.length > 0) {
      expect(await hrefs(tiles(page, 'Unter Cutoff'))).toEqual(
        cutoff.map((m) => hrefOf('movies', m)),
      );
    } else {
      await expect(row(page, 'Unter Cutoff')).toHaveCount(0);
    }
    // Ratings are off on this server: no „Am besten bewertet“.
    await expect(row(page, 'Am besten bewertet')).toHaveCount(0);
  });

  test('„Am besten bewertet“: best first, without the movies that have no rating', async ({
    page,
    request,
  }) => {
    await waitForRatings(request);
    const rated = await apiItems(page, RATINGS, 'movies', QUERIES.rated);
    const expected = rated.filter((m) => m.imdb_rating !== null).map((m) => hrefOf('movies', m));
    expect(expected.length).toBeGreaterThan(1);
    expect(expected.length).toBeLessThan(rated.length);
    await page.goto(`${RATINGS}/filme`);
    await expect(tiles(page, 'Am besten bewertet')).toHaveCount(expected.length);
    expect(await hrefs(tiles(page, 'Am besten bewertet'))).toEqual(expected);
    // The best rating first, also with few votes (imdb-ratings AC-06).
    const best = tiles(page, 'Am besten bewertet').first();
    await expect(best.getByRole('link')).toHaveAccessibleName(
      /^Nischenfilm, \d{4}, IMDb 9,8 \(12 Stimmen\)$/,
    );
    await expect(
      tiles(page, 'Am besten bewertet').getByRole('link', { name: /^(Ohne IMDb|Unbewertet),/ }),
    ).toHaveCount(0);
  });
});

test.describe('rows of the series list (AC-02)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('the same rows for series: missing episodes, episodes below the cutoff', async ({
    page,
  }) => {
    const added = await apiItems(page, LIBRARY, 'series', QUERIES.added);
    const missing = await apiItems(page, LIBRARY, 'series', QUERIES.missing);
    const cutoff = await apiItems(page, LIBRARY, 'series', QUERIES.cutoff);
    expect(missing.every((s) => 'episodes' in s && s.missing > 0)).toBe(true);
    expect(cutoff.every((s) => 'episodes' in s && s.cutoff_unmet > 0)).toBe(true);
    await page.goto(`${LIBRARY}/serien`);
    await expect(tiles(page, 'Zuletzt hinzugefügt')).toHaveCount(added.length);
    expect(await hrefs(tiles(page, 'Zuletzt hinzugefügt'))).toEqual(
      added.map((s) => hrefOf('series', s)),
    );
    expect(await hrefs(tiles(page, 'Fehlt noch'))).toEqual(missing.map((s) => hrefOf('series', s)));
    expect(missing.map((s) => s.id)).toContain(
      await seriesId(page.request, LIBRARY, 'Kleine Insel'),
    );
    if (cutoff.length > 0) {
      expect(await hrefs(tiles(page, 'Unter Cutoff'))).toEqual(
        cutoff.map((s) => hrefOf('series', s)),
      );
    } else {
      await expect(row(page, 'Unter Cutoff')).toHaveCount(0);
    }
    await expect(row(page, 'Am besten bewertet')).toHaveCount(0);
    const headings = await page.getByRole('heading', { level: 2 }).allTextContents();
    expect(headings.at(-1) === 'Alle Serien' || headings.includes('Alle Serien')).toBe(true);
  });

  test('„Am besten bewertet“ of the series while ratings are on', async ({ page, request }) => {
    await waitForRatings(request);
    const rated = await apiItems(page, RATINGS, 'series', QUERIES.rated);
    const expected = rated.filter((s) => s.imdb_rating !== null).map((s) => hrefOf('series', s));
    expect(expected.length).toBeGreaterThan(0);
    await page.goto(`${RATINGS}/serien`);
    await expect(row(page, 'Am besten bewertet')).toBeVisible();
    await expect(tiles(page, 'Am besten bewertet')).toHaveCount(expected.length);
    expect(await hrefs(tiles(page, 'Am besten bewertet'))).toEqual(expected);
  });
});

test.describe('the full list (AC-03)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  for (const [path, heading, noun] of [
    ['/filme', 'Alle Filme', 'Filme'],
    ['/serien', 'Alle Serien', 'Serien'],
  ] as const) {
    test(`„${heading}“ below the rows, with the toolbar as today`, async ({ page }) => {
      await page.goto(`${LIBRARY}${path}`);
      await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
      const h2 = allHeading(page, heading);
      await expect(h2).toBeVisible();
      expect(await top(h2)).toBeGreaterThan(await bottom(allRows(page).last()));
      expect(await top(toolbar(page))).toBeGreaterThan(await bottom(h2));
      expect(await top(items(page).first())).toBeGreaterThan(await bottom(toolbar(page)));
      await expect(page.getByTestId('total')).toHaveText(new RegExp(`^\\d+ ${noun}$`));
      // The toolbar works as today: the view switches the full list only.
      await page.getByRole('button', { name: 'Liste', exact: true }).click();
      await expect(page.getByTestId('library-items')).not.toHaveClass(/grid/);
      await expect(tiles(page, 'Zuletzt hinzugefügt').first().getByRole('link')).toBeVisible();
      await expect(
        tiles(page, 'Zuletzt hinzugefügt')
          .first()
          .locator('img, [data-testid="poster-placeholder"]'),
      ).toBeVisible();
    });
  }
});

test.describe('searching and filtering (AC-04)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a search hides the rows; clearing it shows them again', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme`);
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    await page.getByLabel('Suche').fill('Fehlt Sehr');
    await expect(items(page)).toHaveCount(1);
    await expect(allRows(page)).toHaveCount(0);
    const h1 = page.getByRole('heading', { level: 1, name: 'Filme' });
    expect(await top(allHeading(page, 'Alle Filme'))).toBeGreaterThanOrEqual(await bottom(h1));
    await page.getByLabel('Suche').fill('');
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    await expect(row(page, 'Fehlt noch')).toBeVisible();
  });

  test('a filter in the address: no rows until the filters are cleared', async ({ page }) => {
    await page.goto(`${LIBRARY}/serien?ueberwacht=ja`);
    await expect(items(page).first()).toBeVisible();
    await expect(allRows(page)).toHaveCount(0);
    await page.getByRole('button', { name: 'Filter Überwacht: Ja entfernen' }).click();
    await expect(page).toHaveURL(`${LIBRARY}/serien`);
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
  });

  test('a filter set in the panel hides the rows', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme`);
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    await openFilters(page);
    await page.getByLabel('Überwacht', { exact: true }).selectOption({ label: 'Nein' });
    await expect(allRows(page)).toHaveCount(0);
  });
});

test.describe('„Alle anzeigen“ (AC-05)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('„Zuletzt hinzugefügt“: sort by added, newest first; the rows stay', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme`);
    await moreLink(page, 'Zuletzt hinzugefügt').click();
    await expect(page).toHaveURL(`${LIBRARY}/filme?sortierung=hinzugefuegt&richtung=ab`);
    await expect(sortOf(page)).toHaveValue('added');
    await expect(page.getByRole('button', { name: 'Aufsteigend sortieren' })).toBeVisible();
    const added = await apiItems(page, LIBRARY, 'movies', QUERIES.added);
    await expect(items(page).first().getByRole('link')).toHaveAttribute(
      'href',
      hrefOf('movies', added[0] as Item),
    );
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    // The page scrolled to the full list: its heading at the top of the window.
    const heading = allHeading(page, 'Alle Filme');
    await expect.poll(() => top(heading)).toBeLessThan(120);
    expect(await top(heading)).toBeGreaterThanOrEqual(0);
    expect(await page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
    // A reload keeps it.
    await page.reload();
    await expect(sortOf(page)).toHaveValue('added');
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
  });

  test('„Fehlt noch“ and „Unter Cutoff“: their filter, newest first; the rows hide', async ({
    page,
  }) => {
    const movies = await apiItems(page, LIBRARY, 'movies', QUERIES.added);
    await serveRows(page, (_, key) => (key === 'cutoff' ? movies.slice(0, 3) : undefined));
    for (const [name, param, filter] of [
      ['Fehlt noch', 'fehlt', 'Fehlt'],
      ['Unter Cutoff', 'cutoff', 'Cutoff nicht erreicht'],
    ] as const) {
      await page.goto(`${LIBRARY}/filme`);
      await moreLink(page, name).click();
      await expect(page).toHaveURL(
        `${LIBRARY}/filme?${param}=ja&sortierung=hinzugefuegt&richtung=ab`,
      );
      await expect(allRows(page)).toHaveCount(0);
      await expect(sortOf(page)).toHaveValue('added');
      await expect(
        page.getByRole('button', { name: `Filter ${filter}: Ja entfernen` }),
      ).toBeVisible();
      await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
      await page.reload();
      await expect(
        page.getByRole('button', { name: `Filter ${filter}: Ja entfernen` }),
      ).toBeVisible();
      await expect(allRows(page)).toHaveCount(0);
    }
  });

  test('„Am besten bewertet“: sort by rating, best first; the rows stay', async ({
    page,
    request,
  }) => {
    await waitForRatings(request);
    await page.goto(`${RATINGS}/filme`);
    await moreLink(page, 'Am besten bewertet').click();
    await expect(page).toHaveURL(`${RATINGS}/filme?sortierung=bewertung`);
    await expect(sortOf(page)).toHaveValue('rating');
    await expect(items(page).first()).toContainText('Nischenfilm');
    await expect(row(page, 'Am besten bewertet')).toBeVisible();
    await page.reload();
    await expect(sortOf(page)).toHaveValue('rating');
  });
});

test.describe('empty rows (AC-06)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a row without titles does not show', async ({ page }) => {
    const movies = await apiItems(page, LIBRARY, 'movies', QUERIES.added);
    await serveRows(page, (_, key) => (key === 'added' ? movies.slice(0, 4) : []));
    await page.goto(`${LIBRARY}/filme`);
    await expect(tiles(page, 'Zuletzt hinzugefügt')).toHaveCount(4);
    await expect(allRows(page)).toHaveCount(1);
  });

  test('no row with titles: the page starts with the full list', async ({ page }) => {
    await serveRows(page, () => []);
    await page.goto(`${LIBRARY}/serien`);
    await expect(items(page).first()).toBeVisible();
    await page.waitForLoadState('networkidle');
    await expect(allRows(page)).toHaveCount(0);
    const headings = await page.getByRole('heading', { level: 2 }).allTextContents();
    expect(headings[0]).toBe('Alle Serien');
  });
});

test.describe('tiles and monitoring (AC-07)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a row tile has the monitor toggle, no checkbox; a change shows everywhere', async ({
    page,
  }) => {
    // The first movie of the full list at the front of three rows.
    const movies = await apiItems(
      page,
      LIBRARY,
      'movies',
      'sort=title&order=asc&page=1&page_size=50',
    );
    const [first, ...others] = movies;
    if (first === undefined) throw new Error('no movies');
    await serveRows(page, (_, key) => (key === 'rated' ? [] : [first, ...others.slice(0, 4)]));
    await fakeMonitor(page);
    await page.goto(`${LIBRARY}/filme`);
    const toggle = `Überwachen „${first.title}“`;
    const inRow = (rowName: RowName) =>
      tiles(page, rowName).first().getByRole('button', { name: toggle, exact: true });
    const inList = items(page).first().getByRole('button', { name: toggle, exact: true });
    const before = await inList.getAttribute('aria-pressed');
    const after = before === 'true' ? 'false' : 'true';
    const shown = ['Zuletzt hinzugefügt', 'Fehlt noch', 'Unter Cutoff'] as const;
    for (const rowName of shown) {
      await expect(inRow(rowName)).toHaveAttribute('aria-pressed', before ?? '');
    }
    await expect(page.getByTestId('tile-row').getByRole('checkbox')).toHaveCount(0);

    await inRow('Fehlt noch').click();
    for (const rowName of shown)
      await expect(inRow(rowName)).toHaveAttribute('aria-pressed', after);
    await expect(inList).toHaveAttribute('aria-pressed', after);
    // The tile stays in its rows, whether it still matches them or not, until they load.
    for (const rowName of shown) {
      await expect(tiles(page, rowName).first().getByRole('link')).toHaveAttribute(
        'href',
        `/filme/${first.id}`,
      );
    }
  });

  test('a change in the full list shows on the rows’ tiles', async ({ page }) => {
    const movies = await apiItems(
      page,
      LIBRARY,
      'movies',
      'sort=title&order=asc&page=1&page_size=50',
    );
    const first = movies[0];
    if (first === undefined) throw new Error('no movies');
    await serveRows(page, (_, key) => (key === 'added' || key === 'missing' ? [first] : []));
    await fakeMonitor(page);
    await page.goto(`${LIBRARY}/filme`);
    const name = `Überwachen „${first.title}“`;
    const inRows = page.getByTestId('tile-row').getByRole('button', { name, exact: true });
    await expect(inRows).toHaveCount(2);
    const inList = items(page).first().getByRole('button', { name, exact: true });
    const before = await inList.getAttribute('aria-pressed');
    await inList.click();
    const after = before === 'true' ? 'false' : 'true';
    await expect(inList).toHaveAttribute('aria-pressed', after);
    await expect(inRows.nth(0)).toHaveAttribute('aria-pressed', after);
    await expect(inRows.nth(1)).toHaveAttribute('aria-pressed', after);
  });
});

test.describe('loading time (AC-08)', () => {
  test('rows and the first page within 2 s with 2,501 movies', async ({ page, request }) => {
    const all = (await (
      await request.get(`${LIBRARY}/api/library/movies?page_size=1`)
    ).json()) as ListPage<MovieListItem>;
    expect(all.total).toBeGreaterThanOrEqual(2501);
    await page.goto(`${LIBRARY}/filme`);
    await expect(items(page).first()).toBeVisible();
    await expect(tiles(page, 'Zuletzt hinzugefügt').first()).toBeVisible();
    await expect(tiles(page, 'Fehlt noch').first()).toBeVisible();
    const ms = await page.evaluate(() => performance.now());
    const text = `movie list rows and first page shown after ${Math.round(ms)} ms`;
    test.info().annotations.push({ type: 'Messung', description: text });
    expect(ms, text).toBeLessThan(2000);
  });
});

test.describe('failures and reloads (AC-12)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a row that cannot be loaded does not show; it loads again when the list is plain again', async ({
    page,
  }) => {
    let failing: RowKey[] = ['missing'];
    const counts = await serveRows(page, (_, key) => (failing.includes(key) ? 'fail' : undefined));
    await page.goto(`${LIBRARY}/filme`);
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    await expect(items(page).first()).toBeVisible();
    await page.waitForLoadState('networkidle');
    await expect(row(page, 'Fehlt noch')).toHaveCount(0);
    await expect(page.getByText(/konnte nicht geladen werden/)).toHaveCount(0);
    expect(counts.added).toBe(1);
    // Sorting keeps the list plain: no new load.
    await page.getByLabel('Sortierung').selectOption({ label: 'Jahr' });
    await expect(page).toHaveURL(/sortierung=jahr/);
    await page.waitForLoadState('networkidle');
    expect(counts.added).toBe(1);
    // The list becomes plain again: the rows load, the failed one shows now.
    failing = [];
    await page.getByLabel('Suche').fill('Film 1');
    await expect(allRows(page)).toHaveCount(0);
    await page.getByLabel('Suche').fill('');
    await expect(row(page, 'Fehlt noch')).toBeVisible();
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    expect(counts.added).toBe(2);
  });
});

async function axe(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  expect(
    result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`),
  ).toEqual([]);
}

test.describe('accessibility in the dark palette (AC-09)', () => {
  test.beforeEach(async ({ page }) => storeTheme(page, 'dark'));

  for (const path of ['/filme', '/serien']) {
    for (const width of [1440, 360]) {
      test(`axe, named lists and keyboard on ${path} at ${width} px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await page.goto(`${LIBRARY}${path}`);
        for (const name of ['Zuletzt hinzugefügt', 'Fehlt noch'] as const) {
          await expect(page.getByRole('list', { name, exact: true })).toBeVisible();
        }
        await expect(items(page).first()).toBeVisible();
        await page.waitForLoadState('networkidle');
        await axe(page);
        await expectNoHorizontalScroll(page);
        // From „Alle anzeigen“ the arrows, then the first tile and its toggle, each with a
        // visible focus.
        const shelf = row(page, 'Zuletzt hinzugefügt');
        await moreLink(page, 'Zuletzt hinzugefügt').focus();
        await page.keyboard.press('Tab');
        const forward = shelf.getByRole('button', { name: 'Weiter in „Zuletzt hinzugefügt“' });
        await expect(forward).toBeFocused();
        expect(await forward.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
        await page.keyboard.press('Tab');
        const first = tiles(page, 'Zuletzt hinzugefügt').first();
        await expect(first.getByRole('link')).toBeFocused();
        expect(
          await first.getByRole('link').evaluate((el) => getComputedStyle(el).outlineStyle),
        ).not.toBe('none');
        await page.keyboard.press('Tab');
        const toggle = first.getByRole('button', { name: /^Überwachen „/ });
        await expect(toggle).toBeFocused();
        expect(await toggle.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
        await page.keyboard.press('Shift+Tab');
        const href = (await first.getByRole('link').getAttribute('href')) ?? '';
        await page.keyboard.press('Enter');
        await expect(page).toHaveURL(`${LIBRARY}${href}`);
      });
    }
  }
});
