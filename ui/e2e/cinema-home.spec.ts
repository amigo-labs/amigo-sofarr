// Spec ui-cinema-home: the rows „Demnächst“, „Zuletzt hinzugefügt“ and „Fehlt noch“ on the
// overview, between its key figures and its boxes.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Locator, type Page, type Route } from '@playwright/test';
import type { CalendarEntry } from '../src/lib/api/generated/CalendarEntry';
import type { MovieListItem } from '../src/lib/api/generated/MovieListItem';
import type { OverviewAnswer } from '../src/lib/api/generated/OverviewAnswer';
import type { Page as ListPage } from '../src/lib/api/generated/Page';
import type { SeriesListItem } from '../src/lib/api/generated/SeriesListItem';
import { expectNoHorizontalScroll } from './helpers';
import { LIBRARY, movieId, seriesId } from './library-helpers';
import { storeTheme } from './theme-helpers';

const ROWS = ['Demnächst', 'Zuletzt hinzugefügt', 'Fehlt noch'] as const;
type RowName = (typeof ROWS)[number];
const ALL_QUIET = 'Alles ruhig – keine Downloads, Probleme oder Termine.';

const row = (page: Page, name: RowName): Locator => page.getByRole('region', { name, exact: true });
const tiles = (page: Page, name: RowName): Locator =>
  page.getByRole('list', { name, exact: true }).locator(':scope > li');
const moreLink = (page: Page, name: RowName): Locator =>
  row(page, name).getByRole('link', { name: /^Alle anzeigen/ });

type Lists = {
  movies: MovieListItem[];
  series: SeriesListItem[];
  missingMovies: MovieListItem[];
  missingSeries: SeriesListItem[];
};

async function list<T>(page: Page, path: string): Promise<T[]> {
  const answer = await page.request.get(`${LIBRARY}${path}`);
  return ((await answer.json()) as ListPage<T>).items;
}

/** The rows' four list answers of the library server now. */
async function realLists(page: Page): Promise<Lists> {
  const query = '?sort=added&order=desc&page=1&page_size=20';
  const missing = '?missing=yes&sort=added&order=desc&page=1&page_size=20';
  return {
    movies: await list(page, `/api/library/movies${query}`),
    series: await list(page, `/api/library/series${query}`),
    missingMovies: await list(page, `/api/library/movies${missing}`),
    missingSeries: await list(page, `/api/library/series${missing}`),
  };
}

const page20 = <T>(items: T[]): ListPage<T> => ({
  page: 1,
  page_size: 20,
  total: items.length,
  items,
});

/**
 * Serves the rows' list requests (newest added, one page of 20) from `lists()`, read at
 * each request; `fail` names requests answered with a server error. The list pages' own
 * requests (other sorts) reach the server.
 */
async function serveLists(
  page: Page,
  lists: () => Lists,
  fail: () => readonly ('movies' | 'series' | 'missingMovies' | 'missingSeries')[] = () => [],
): Promise<void> {
  await page.route(/\/api\/library\/(movies|series)\?/, async (route: Route) => {
    const url = new URL(route.request().url());
    if (url.searchParams.get('sort') !== 'added' || url.searchParams.get('page_size') !== '20') {
      return route.fallback();
    }
    const series = url.pathname.endsWith('/series');
    const missing = url.searchParams.get('missing') === 'yes';
    const key = missing
      ? series
        ? 'missingSeries'
        : 'missingMovies'
      : series
        ? 'series'
        : 'movies';
    if (fail().includes(key)) {
      return route.fulfill({ status: 500, json: { error: 'Datenbank nicht erreichbar' } });
    }
    return route.fulfill({ json: page20(lists()[key] as unknown[]) });
  });
}

const EMPTY_LISTS: Lists = { movies: [], series: [], missingMovies: [], missingSeries: [] };

async function realOverview(page: Page): Promise<OverviewAnswer> {
  return (await (await page.request.get(`${LIBRARY}/api/overview`)).json()) as OverviewAnswer;
}

async function serveOverview(page: Page, answer: () => OverviewAnswer): Promise<void> {
  await page.route('**/api/overview', (route) => route.fulfill({ json: answer() }));
}

const EMPTY_BOXES = (answer: OverviewAnswer): OverviewAnswer => ({
  ...answer,
  queue: { ...answer.queue, client_configured: true, notice: null, items: [] },
  activity: [],
  problems: [],
  upcoming: [],
});

/** The browser's local date in `days` days as `YYYY-MM-DD`. */
async function localDay(page: Page, days: number): Promise<string> {
  return page.evaluate((d) => {
    const at = new Date();
    at.setDate(at.getDate() + d);
    const month = String(at.getMonth() + 1).padStart(2, '0');
    return `${at.getFullYear()}-${month}-${String(at.getDate()).padStart(2, '0')}`;
  }, days);
}

const movieEntry = (date: string, id: number, title: string, fields: Partial<CalendarEntry>) =>
  ({
    date,
    kind: 'movie',
    release: 'cinema',
    movie_id: id,
    series_id: null,
    episode_id: null,
    season: null,
    episode: null,
    title,
    episode_title: null,
    year: 2026,
    state: 'upcoming',
    monitored: true,
    has_poster: false,
    ...fields,
  }) satisfies CalendarEntry;

const episodeEntry = (date: string, id: number, season: number, episode: number) =>
  ({
    ...movieEntry(date, 0, 'Kleine Insel', {}),
    kind: 'episode',
    release: null,
    movie_id: null,
    series_id: id,
    episode_id: season * 100 + episode,
    season,
    episode,
    year: 2020,
  }) satisfies CalendarEntry;

const hrefs = (scope: Locator): Promise<string[]> =>
  scope.evaluateAll((items) =>
    items.map((li) => li.querySelector('a')?.getAttribute('href') ?? ''),
  );

/** The hrefs of movies and series merged newest added first, at most 20. */
function newest(movies: MovieListItem[], series: SeriesListItem[]): string[] {
  return [
    ...movies.map((m) => ({ at: Date.parse(m.added_at), href: `/filme/${m.id}` })),
    ...series.map((s) => ({ at: Date.parse(s.added_at), href: `/serien/${s.id}` })),
  ]
    .toSorted((a, b) => b.at - a.at)
    .slice(0, 20)
    .map((x) => x.href);
}

const top = async (scope: Locator): Promise<number> => (await scope.boundingBox())?.y ?? -1;
const bottom = async (scope: Locator): Promise<number> => {
  const box = await scope.boundingBox();
  return box === null ? -1 : box.y + box.height;
};

test.describe('rows (AC-01)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('three rows below the key figures and above the boxes, in this order', async ({ page }) => {
    const lists = await realLists(page);
    await serveLists(page, () => lists);
    await page.goto(`${LIBRARY}/`);
    for (const name of ROWS) await expect(row(page, name)).toBeVisible();
    const headings = page.getByRole('heading', { level: 2 });
    const names = await headings.allTextContents();
    expect(names.slice(0, 3)).toEqual([...ROWS]);
    const figures = page.getByRole('list', { name: 'Kennzahlen' });
    expect(await top(row(page, 'Demnächst'))).toBeGreaterThanOrEqual(await bottom(figures));
    expect(await top(row(page, 'Zuletzt hinzugefügt'))).toBeGreaterThan(
      await top(row(page, 'Demnächst')),
    );
    expect(await top(row(page, 'Fehlt noch'))).toBeGreaterThan(
      await top(row(page, 'Zuletzt hinzugefügt')),
    );
    const queue = page.getByRole('region', { name: 'Warteschlange' });
    expect(await top(queue)).toBeGreaterThanOrEqual(await bottom(row(page, 'Fehlt noch')));
    // „Zuletzt hinzugefügt“: movies and series together, newest added first, 20 tiles.
    await expect(tiles(page, 'Zuletzt hinzugefügt')).toHaveCount(20);
    expect(await hrefs(tiles(page, 'Zuletzt hinzugefügt'))).toEqual(
      newest(lists.movies, lists.series),
    );
    // „Fehlt noch“: monitored missing movies and series with missing episodes.
    const missing = newest(lists.missingMovies, lists.missingSeries);
    expect(missing.length).toBeGreaterThan(0);
    expect(await hrefs(tiles(page, 'Fehlt noch'))).toEqual(missing);
    expect(missing).toContain(`/filme/${await movieId(page.request, LIBRARY, 'Fehlt Sehr')}`);
    expect(missing).toContain(`/serien/${await seriesId(page.request, LIBRARY, 'Kleine Insel')}`);
    // „Demnächst“: the calendar data of seed.mjs, each title once.
    const upcoming = tiles(page, 'Demnächst');
    await expect(upcoming.getByRole('link', { name: /^Kalenderfilm, / })).toHaveCount(1);
    await expect(upcoming.getByRole('link', { name: /^Kleine Insel, / })).toHaveCount(1);
  });

  test('„Demnächst“ shows the titles of „Die nächsten 7 Tage“, soonest first, at most 20', async ({
    page,
  }) => {
    const days = [await localDay(page, 0), await localDay(page, 1), await localDay(page, 2)];
    const real = await realOverview(page);
    const many = Array.from({ length: 25 }, (_, i) =>
      movieEntry(days[2] ?? '', 1000 + i, `Spät ${i}`, {}),
    );
    const upcoming = [
      movieEntry(days[1] ?? '', 7, 'Zweimal', { release: 'cinema' }),
      movieEntry(days[0] ?? '', 7, 'Zweimal', { release: 'digital' }),
      episodeEntry(days[1] ?? '', 9, 2, 2),
      episodeEntry(days[1] ?? '', 9, 2, 1),
      episodeEntry(days[2] ?? '', 9, 2, 3),
      ...many,
    ];
    await serveOverview(page, () => ({ ...real, upcoming }));
    await page.goto(`${LIBRARY}/`);
    const links = tiles(page, 'Demnächst').getByRole('link');
    await expect(links).toHaveCount(20);
    await expect(links.nth(0)).toHaveAccessibleName('Zweimal, 2026, Heute');
    await expect(links.nth(1)).toHaveAccessibleName('Kleine Insel, 2020, Morgen, S02E01');
    await expect(links.nth(2)).toHaveAccessibleName(/^Spät 0, 2026, /);
    // The box keeps every entry.
    const box = page.getByRole('region', { name: 'Die nächsten 7 Tage' });
    await expect(box.getByTestId('calendar-entry')).toHaveCount(upcoming.length);
  });
});

test.describe('tiles of „Demnächst“ (AC-02)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('day and episode code in the overlay and the link name; poster or placeholder', async ({
    page,
  }) => {
    const days = [await localDay(page, 0), await localDay(page, 1), await localDay(page, 3)];
    const boot = await movieId(page.request, LIBRARY, 'Das Boot');
    const insel = await seriesId(page.request, LIBRARY, 'Kleine Insel');
    const real = await realOverview(page);
    await serveOverview(page, () => ({
      ...real,
      upcoming: [
        movieEntry(days[0] ?? '', boot, 'Das Boot', { year: 1981, has_poster: true }),
        { ...episodeEntry(days[1] ?? '', insel, 3, 5), has_poster: false },
        movieEntry(days[2] ?? '', 4242, 'Ohne Plakat', {}),
      ],
    }));
    await page.goto(`${LIBRARY}/`);
    const all = tiles(page, 'Demnächst');
    await expect(all).toHaveCount(3);
    const [, m, d] = (days[2] ?? '').split('-');
    const weekday = /^(Montag|Dienstag|Mittwoch|Donnerstag|Freitag|Samstag|Sonntag)$/;
    await expect(all.nth(0).getByRole('link')).toHaveAccessibleName('Das Boot, 1981, Heute');
    await expect(all.nth(1).getByRole('link')).toHaveAccessibleName(
      'Kleine Insel, 2020, Morgen, S03E05',
    );
    const third = await all.nth(2).getByRole('link').getAttribute('aria-label');
    const [title, year, day, date] = (third ?? '').split(', ');
    expect([title, year, date]).toEqual(['Ohne Plakat', '2026', `${d}.${m}.`]);
    expect(day).toMatch(weekday);
    // The overlay: one line below the year.
    const overlay = all.nth(1).getByTestId('tile-overlay');
    await expect(overlay.locator('.meta')).toHaveText(['2020', 'Morgen · S03E05']);
    await expect(all.nth(0).getByTestId('tile-overlay').locator('.meta')).toHaveText([
      '1981',
      'Heute',
    ]);
    // Poster from sofarr's image cache, else the placeholder of the library's posters.
    await expect(all.nth(0).locator('img')).toHaveAttribute(
      'src',
      `/api/images/movie/${boot}/list`,
    );
    await expect(all.nth(1).getByTestId('poster-placeholder')).toHaveText('K');
    await expect(all.nth(2).getByTestId('poster-placeholder')).toHaveText('O');
    await expect(all.nth(0).getByRole('link')).toHaveAttribute('href', `/filme/${boot}`);
    await expect(all.nth(1).getByRole('link')).toHaveAttribute('href', `/serien/${insel}`);
  });
});

test.describe('„Alle anzeigen“ (AC-03)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('leads to the calendar, the movie list by added date and „Fehlend“', async ({ page }) => {
    await page.goto(`${LIBRARY}/`);
    await moreLink(page, 'Demnächst').click();
    await expect(page).toHaveURL(`${LIBRARY}/kalender`);
    await expect(page.getByRole('button', { name: 'Woche', exact: true })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await page.goBack();
    await moreLink(page, 'Zuletzt hinzugefügt').click();
    await expect(page).toHaveURL(`${LIBRARY}/filme?sortierung=hinzugefuegt&richtung=ab`);
    await expect(page.getByRole('heading', { level: 1, name: 'Filme' })).toBeVisible();
    await page.goBack();
    await moreLink(page, 'Fehlt noch').click();
    await expect(page).toHaveURL(`${LIBRARY}/fehlend`);
    await expect(page.getByRole('heading', { level: 1, name: 'Fehlend' })).toBeVisible();
  });
});

test.describe('empty rows (AC-04)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a row without titles does not show', async ({ page }) => {
    const lists = await realLists(page);
    await serveLists(page, () => ({ ...lists, missingMovies: [], missingSeries: [] }));
    const real = await realOverview(page);
    await serveOverview(page, () => ({ ...real, upcoming: [] }));
    await page.goto(`${LIBRARY}/`);
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    await expect(row(page, 'Demnächst')).toHaveCount(0);
    await expect(row(page, 'Fehlt noch')).toHaveCount(0);
    await expect(page.getByText(ALL_QUIET)).toHaveCount(0);
  });

  test('no rows and no boxes: the quiet line', async ({ page }) => {
    await serveLists(page, () => EMPTY_LISTS);
    const real = await realOverview(page);
    await serveOverview(page, () => EMPTY_BOXES(real));
    await page.goto(`${LIBRARY}/`);
    await expect(page.getByText(ALL_QUIET)).toBeVisible();
    for (const name of ROWS) await expect(row(page, name)).toHaveCount(0);
  });

  test('a row hidden by a failure does not count as empty', async ({ page }) => {
    await serveLists(
      page,
      () => EMPTY_LISTS,
      () => ['missingSeries'],
    );
    const real = await realOverview(page);
    await serveOverview(page, () => EMPTY_BOXES(real));
    await page.goto(`${LIBRARY}/`);
    await expect(page.getByRole('list', { name: 'Kennzahlen' })).toBeVisible();
    await page.waitForLoadState('networkidle');
    await expect(page.getByText(ALL_QUIET)).toHaveCount(0);
    for (const name of ROWS) await expect(row(page, name)).toHaveCount(0);
  });
});

test.describe('refresh (AC-05)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('rows show the new state, keep their scroll position and the focused tile', async ({
    page,
  }) => {
    const real = await realLists(page);
    let lists: Lists = real;
    await serveLists(page, () => lists);
    await page.clock.install();
    await page.goto(`${LIBRARY}/`);
    const added = tiles(page, 'Zuletzt hinzugefügt');
    await expect(added).toHaveCount(20);
    const strip = page.getByRole('list', { name: 'Zuletzt hinzugefügt', exact: true });
    await strip.evaluate((el) => (el.scrollLeft = 300));
    const focused = added.nth(5).getByRole('link');
    const focusedHref = await focused.getAttribute('href');
    await focused.focus();
    // New state: one title less at the front, the others move one place on.
    const dropped = real.movies[0];
    const droppedSeries = real.series[0];
    lists = {
      ...real,
      movies: real.movies.filter((m) => m !== dropped),
      series: real.series.filter((s) => s !== droppedSeries),
    };
    await page.clock.runFor(16_000);
    await expect.poll(() => hrefs(added)).toEqual(newest(lists.movies, lists.series));
    expect(await strip.evaluate((el) => el.scrollLeft)).toBe(300);
    await expect(page.locator(`a[href="${focusedHref}"]`).first()).toBeFocused();

    // The focused tile leaves its row: the focus moves to the row's „Alle anzeigen“.
    lists = {
      ...lists,
      movies: lists.movies.filter((m) => `/filme/${m.id}` !== focusedHref),
      series: lists.series.filter((s) => `/serien/${s.id}` !== focusedHref),
    };
    await page.clock.runFor(16_000);
    await expect(page.locator(`main a[href="${focusedHref}"]`)).toHaveCount(0);
    await expect(moreLink(page, 'Zuletzt hinzugefügt')).toBeFocused();

    // The row goes: the focus moves to the page's heading.
    await tiles(page, 'Zuletzt hinzugefügt').first().getByRole('link').focus();
    lists = { ...lists, movies: [], series: [] };
    await page.clock.runFor(16_000);
    await expect(row(page, 'Zuletzt hinzugefügt')).toHaveCount(0);
    await expect(page.getByRole('heading', { level: 1, name: 'Übersicht' })).toBeFocused();
  });
});

test.describe('failures (AC-06)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a row whose data cannot be loaded does not show; the next refresh tries again', async ({
    page,
  }) => {
    const lists = await realLists(page);
    let failing: ('movies' | 'missingSeries')[] = ['missingSeries'];
    await serveLists(
      page,
      () => lists,
      () => failing,
    );
    await page.clock.install();
    await page.goto(`${LIBRARY}/`);
    await expect(row(page, 'Zuletzt hinzugefügt')).toBeVisible();
    await expect(row(page, 'Demnächst')).toBeVisible();
    await expect(page.getByRole('region', { name: 'Warteschlange' })).toBeVisible();
    await expect(row(page, 'Fehlt noch')).toHaveCount(0);
    await expect(page.getByText(/Aktualisieren fehlgeschlagen/)).toHaveCount(0);
    failing = ['movies'];
    await page.clock.runFor(16_000);
    await expect(row(page, 'Fehlt noch')).toBeVisible();
    await expect(row(page, 'Zuletzt hinzugefügt')).toHaveCount(0);
    await expect(row(page, 'Demnächst')).toBeVisible();
  });
});

test.describe('loading time (AC-07)', () => {
  test('the rows show within 2 s with 2,501 movies and 23 series', async ({ page, request }) => {
    const movies = (await (
      await request.get(`${LIBRARY}/api/library/movies?page_size=1`)
    ).json()) as ListPage<MovieListItem>;
    const series = (await (
      await request.get(`${LIBRARY}/api/library/series?page_size=1`)
    ).json()) as ListPage<SeriesListItem>;
    expect(movies.total).toBeGreaterThanOrEqual(2501);
    expect(series.total).toBe(23);
    await page.goto(`${LIBRARY}/`);
    for (const name of ROWS) await expect(tiles(page, name).first()).toBeVisible();
    // Milliseconds since the navigation started (the page's time origin).
    const ms = await page.evaluate(() => performance.now());
    const text = `overview rows shown after ${Math.round(ms)} ms`;
    test.info().annotations.push({ type: 'Messung', description: text });
    expect(ms, text).toBeLessThan(2000);
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

test.describe('accessibility in the dark palette (AC-08)', () => {
  test.beforeEach(async ({ page }) => storeTheme(page, 'dark'));

  for (const width of [1440, 360]) {
    test(`axe, named lists and keyboard at ${width} px`, async ({ page }) => {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`${LIBRARY}/`);
      for (const name of ROWS) {
        await expect(page.getByRole('list', { name, exact: true })).toBeVisible();
      }
      await page.waitForLoadState('networkidle');
      await axe(page);
      await expectNoHorizontalScroll(page);
      // From „Alle anzeigen“ the arrows, then the first tile, each with a visible focus.
      const shelf = row(page, 'Zuletzt hinzugefügt');
      await moreLink(page, 'Zuletzt hinzugefügt').focus();
      await page.keyboard.press('Tab');
      const forward = shelf.getByRole('button', { name: 'Weiter in „Zuletzt hinzugefügt“' });
      await expect(forward).toBeFocused();
      expect(await forward.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
      await page.keyboard.press('Tab');
      const first = tiles(page, 'Zuletzt hinzugefügt').first().getByRole('link');
      await expect(first).toBeFocused();
      expect(await first.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
      const href = (await first.getAttribute('href')) ?? '';
      await page.keyboard.press('Enter');
      await expect(page).toHaveURL(`${LIBRARY}${href}`);
    });
  }
});
