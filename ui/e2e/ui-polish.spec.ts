// Spec ui-polish AC-02 .. AC-13: even poster tiles, a quiet pending rating, episode toggles
// in the row, series facts in one row, empty overview boxes hidden, the freshness line at
// the page's end, themed dropdowns, compact empty calendar weeks.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Locator, type Page } from '@playwright/test';
import type { CalendarAnswer } from '../src/lib/api/generated/CalendarAnswer';
import type { CalendarEntry } from '../src/lib/api/generated/CalendarEntry';
import type { MovieListItem } from '../src/lib/api/generated/MovieListItem';
import type { OverviewAnswer } from '../src/lib/api/generated/OverviewAnswer';
import type { Page as ListPage } from '../src/lib/api/generated/Page';
import type { StatesAnswer } from '../src/lib/api/generated/StatesAnswer';
import { DISCOVER, settle } from './discover-helpers';
import { RATINGS, waitForRatings } from './extras-helpers';
import { emptyRows, LIBRARY, items, movieId, seriesId } from './library-helpers';
import { ratingsSettled } from './site/settled.mjs';
import { PALETTES, storeTheme } from './theme-helpers';

const LONG_TITLE =
  'Die außerordentlich lange und verschlungene Geschichte vom Leuchtturmwärter am Ende der Welt';

type Box = { x: number; y: number; width: number; height: number; bottom: number };

const boxes = (all: Locator): Promise<Box[]> =>
  all.evaluateAll((els) =>
    els.map((el) => {
      const r = el.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom };
    }),
  );

const box = async (one: Locator): Promise<Box> => {
  const [b] = await boxes(one);
  if (b === undefined) throw new Error('no element');
  return b;
};

/** Whether all numbers are the same within a pixel. */
const level = (values: readonly number[]): boolean =>
  values.every((v) => Math.abs(v - (values[0] ?? 0)) <= 1);

/** The elements grouped into rows by their top edge, top to bottom. */
function rowsOf(all: readonly Box[]): Box[][] {
  const rows: Box[][] = [];
  for (const b of all.toSorted((a, c) => a.y - c.y)) {
    const last = rows.at(-1);
    if (last !== undefined && Math.abs((last[0]?.y ?? 0) - b.y) <= 2) last.push(b);
    else rows.push([b]);
  }
  return rows;
}

async function axe(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  expect(
    result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`),
  ).toEqual([]);
}

/** Serves the movie list with `change` applied to the items of the real answer. */
async function moviesWith(
  page: Page,
  change: (items: MovieListItem[]) => void,
  base = LIBRARY,
): Promise<void> {
  await page.route(`${base}/api/library/movies?*`, async (route) => {
    // The rows above the list (spec ui-cinema-library-rows, 20 titles each) stay as they are.
    if (new URL(route.request().url()).searchParams.get('page_size') === '20') {
      return route.fallback();
    }
    const answer = await route.fetch();
    const list = (await answer.json()) as ListPage<MovieListItem>;
    change(list.items);
    await route.fulfill({ response: answer, json: list });
  });
}

/** Long and short titles, one and two chips, a missing year: the mix AC-04 and AC-05 name. */
function mixTiles(list: MovieListItem[]): void {
  const [a, b, c] = list;
  if (a === undefined || b === undefined || c === undefined) throw new Error('too few movies');
  a.title = LONG_TITLE;
  a.cutoff_unmet = true;
  a.quality = 'WEBDL-1080p';
  b.year = null;
  b.title_de = null;
  b.original_title = b.title;
  c.original_title = 'Star Voyage';
}

async function posterView(page: Page): Promise<void> {
  await page.addInitScript(() => localStorage.setItem('sofarr-ansicht-filme', 'poster'));
}

async function openSeries(page: Page, title = 'Kleine Insel'): Promise<void> {
  await page.goto(`${LIBRARY}/serien/${await seriesId(page.request, LIBRARY, title)}`);
  await expect(page.getByRole('heading', { name: 'Staffeln' })).toBeVisible();
}

/** Serves the discover page with the ratings of the titles with an even TMDB id pending. */
const isPending = (tmdbId: number) => tmdbId % 2 === 0;

async function pendingRatings(page: Page): Promise<void> {
  const PENDING = { state: 'pending', rating: null, votes: null } as const;
  // The shelves carry the first states; the page asks for newer ones while one is pending.
  await page.route('**/api/discover/shelves', async (route) => {
    const answer = await route.fetch();
    const body = (await answer.json()) as {
      shelves: { items: { tmdb_id: number; rating: unknown }[] }[];
    };
    for (const shelf of body.shelves) {
      for (const item of shelf.items) if (isPending(item.tmdb_id)) item.rating = PENDING;
    }
    await route.fulfill({ response: answer, json: body });
  });
  await page.route('**/api/discover/states', async (route) => {
    // The page polls while a rating is pending: a request cut off by the test's end is fine.
    try {
      const answer = await route.fetch();
      const states = (await answer.json()) as StatesAnswer;
      for (const t of states.titles) if (isPending(t.tmdb_id)) t.rating = { ...PENDING };
      await route.fulfill({ response: answer, json: states });
    } catch {
      await route.abort().catch(() => undefined);
    }
  });
}

const EMPTY_OVERVIEW = (answer: OverviewAnswer): OverviewAnswer => ({
  ...answer,
  queue: { ...answer.queue, client_configured: true, notice: null, items: [] },
  activity: [],
  problems: [],
  upcoming: [],
});

async function overviewWith(
  page: Page,
  change: (answer: OverviewAnswer) => OverviewAnswer,
): Promise<void> {
  const real = (await (await page.request.get(`${LIBRARY}/api/overview`)).json()) as OverviewAnswer;
  const answer = change(real);
  await page.route('**/api/overview', (route) => route.fulfill({ json: answer }));
}

const PROBLEM: OverviewAnswer['problems'][number] = {
  kind: 'movie',
  grab_id: 91,
  media_id: 1,
  media_title: 'Film 0001',
  release_title: 'Film.0001.1951.1080p.BluRay.x264-GRP',
  quality: 'Bluray-1080p',
  size: 1_000_000_000,
  status: 'failed',
  message: 'Download fehlgeschlagen',
  episode_ids: [],
  grabbed_at: '2026-11-17T08:00:00Z',
  updated_at: '2026-11-17T08:05:00Z',
};

const entry = (date: string, title: string, state: CalendarEntry['state']): CalendarEntry => ({
  date,
  kind: 'movie',
  release: 'cinema',
  movie_id: 1,
  series_id: null,
  episode_id: null,
  season: null,
  episode: null,
  title,
  episode_title: null,
  year: 2026,
  state,
  monitored: true,
  has_poster: false,
});

const BOXES = [
  'Warteschlange',
  'Die nächsten 7 Tage',
  'Probleme der letzten 7 Tage',
  'Letzte Aktivität',
];
const ALL_QUIET = 'Alles ruhig – keine Downloads, Probleme oder Termine.';

test.describe('discover tiles: pending rating (AC-02)', () => {
  test('a pending rating shows no text, keeps its line and its name', async ({ page }) => {
    await pendingRatings(page);
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const shelf = page.getByTestId('shelf').first();
    const pending = shelf.locator('[data-testid="rating"].pending').first();
    const settled = shelf.locator('[data-testid="rating"]:not(.pending)').first();
    await expect(pending).toBeVisible();
    await expect(settled).toBeVisible();
    expect((await pending.innerText()).trim()).toBe('');
    // Its name is part of the tile link's (ui-cinema-tiles AC-05).
    const tile = shelf
      .getByTestId('title-card')
      .filter({ has: page.locator('[data-testid="rating"].pending') })
      .first();
    await expect(tile.getByRole('link')).toHaveAccessibleName(/, IMDb-Bewertung folgt$/);
    const a = await box(pending);
    const b = await box(settled);
    expect(a.height).toBeGreaterThan(10);
    expect(Math.abs(a.height - b.height)).toBeLessThanOrEqual(1);
    await expect(settled).toHaveText(/^(★ \d,\d|–)$/);
  });
});

test.describe('library cards and rows: year and addition (AC-03)', () => {
  for (const view of ['liste', 'poster'] as const) {
    test(`„ · “ between year and addition, no dot with only one of them (${view})`, async ({
      page,
    }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.addInitScript(
        (value: string) => localStorage.setItem('sofarr-ansicht-filme', value),
        view,
      );
      await moviesWith(page, (list) => {
        const [a, b, c] = list;
        if (a === undefined || b === undefined || c === undefined) throw new Error('too few');
        a.year = 2024;
        a.title_de = null;
        a.original_title = 'Star Voyage';
        b.year = null;
        b.title_de = null;
        b.original_title = 'Nur ein Zusatz';
        c.year = 2020;
        c.title_de = null;
        c.original_title = c.title;
      });
      await page.goto(`${LIBRARY}/filme`);
      await expect(items(page).first()).toBeVisible();
      const meta = items(page).locator('.main .meta');
      // `textContent`, so a missing space is not hidden by white-space collapsing.
      const text = async (n: number) => ((await meta.nth(n).textContent()) ?? '').trim();
      expect(await text(0)).toBe('2024 · Star Voyage');
      expect(await text(1)).toBe('Nur ein Zusatz');
      expect(await text(2)).toBe('2020');
    });
  }
});

// Revised by ui-cinema-tiles: at rest a tile is only its poster; title, year, chips, rating
// and button show in the overlay of the open tile (ui-cinema-tiles AC-01, AC-03, AC-08).
test.describe('poster tiles (AC-04, AC-05)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('library cards: tiles of a row end level; the overlay cuts a long title at two lines', async ({
    page,
    request,
  }) => {
    await waitForRatings(request);
    await posterView(page);
    await moviesWith(page, mixTiles, RATINGS);
    await page.goto(`${RATINGS}/filme`);
    const all = items(page);
    await expect(all.first()).toBeVisible();
    const first = rowsOf(await boxes(all))[0] ?? [];
    expect(first.length).toBeGreaterThanOrEqual(3);
    // A row with one chip and two chips: the tiles end at one edge, as high as their posters.
    const chipCounts = await all.evaluateAll((els) =>
      els.map((el) => el.querySelectorAll('.main .labels .label').length),
    );
    expect(chipCounts.slice(0, first.length)).toContain(2);
    expect(chipCounts.slice(0, first.length)).toContain(1);
    expect(level(first.map((b) => b.bottom))).toBe(true);
    const posters = (await boxes(all.locator('.poster-wrap'))).slice(0, first.length);
    expect(level(posters.map((b) => b.bottom))).toBe(true);

    // The long title: full in the tooltip, cut after two lines in the open tile's overlay.
    const long = all.filter({ has: page.locator(`a[title="${LONG_TITLE}"]`) });
    await long.hover();
    const title = long.locator('.main .title');
    const lines = await title.evaluate(
      (el) => el.clientHeight / parseFloat(getComputedStyle(el).lineHeight),
    );
    expect(Math.abs(lines - 2)).toBeLessThanOrEqual(0.05);
    expect(await title.evaluate((el) => el.scrollHeight > el.clientHeight + 1)).toBe(true);
  });

  test('discover tiles: a shelf ends level; the overlay cuts a long title at two lines', async ({
    page,
  }) => {
    await page.route('**/api/discover/shelves', async (route) => {
      const answer = await route.fetch();
      const body = (await answer.json()) as { shelves: { items: { title: string }[] }[] };
      const title = body.shelves[0]?.items[1];
      if (title !== undefined) title.title = LONG_TITLE;
      await route.fulfill({ response: answer, json: body });
    });
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const shelf = page.getByTestId('shelf').first();
    const tiles = shelf.getByTestId('title-card');
    await expect(tiles.nth(1)).toBeVisible();
    const tileBoxes = await boxes(tiles);
    expect(level(tileBoxes.map((b) => b.bottom))).toBe(true);
    const long = tiles.nth(1);
    await expect(long.locator('a')).toHaveAttribute('title', LONG_TITLE);
    await expect(long.getByRole('link', { name: LONG_TITLE })).toBeVisible();
    await long.hover();
    const title = long.locator('.title');
    const lines = await title.evaluate(
      (el) => el.clientHeight / parseFloat(getComputedStyle(el).lineHeight),
    );
    expect(Math.abs(lines - 2)).toBeLessThanOrEqual(0.05);
    expect(await title.evaluate((el) => el.scrollHeight > el.clientHeight + 1)).toBe(true);
  });
});

const middle = (b: Box) => b.y + b.height / 2;
const facts = (page: Page) => page.locator('dl.facts-compact > div');

test.describe('episode rows (AC-06)', () => {
  test('from 768 px the toggles sit in the main line, before the buttons', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    // One chip per episode, so the main line fits on one line (two chips may wrap it).
    await page.route('**/api/library/series/*', async (route) => {
      const answer = await route.fetch();
      const series = (await answer.json()) as {
        seasons: { episodes: { cutoff_unmet: boolean }[] }[];
      };
      for (const season of series.seasons) {
        for (const episode of season.episodes) episode.cutoff_unmet = false;
      }
      await route.fulfill({ response: answer, json: series });
    });
    await openSeries(page);
    const withFile = page.locator('[data-episode="S01E01"]');
    const without = page.locator('[data-episode="S01E02"]');
    const file = withFile.getByRole('button', { name: /^Datei von/ });
    const subs = withFile.getByRole('button', { name: /^Untertitel von/ });
    const monitor = withFile.locator('button[aria-pressed]');
    const code = await box(withFile.locator('.code'));
    const f = await box(file);
    const s = await box(subs);
    const m = await box(monitor);
    expect(Math.abs(middle(f) - middle(code))).toBeLessThanOrEqual(6);
    expect(Math.abs(middle(s) - middle(code))).toBeLessThanOrEqual(6);
    expect(f.x + f.width).toBeLessThanOrEqual(s.x);
    expect(s.x + s.width).toBeLessThanOrEqual(m.x);
    // Closed, a row with a file is as high as one without.
    expect(
      Math.abs((await box(withFile)).height - (await box(without)).height),
    ).toBeLessThanOrEqual(1);
    await expect(without.getByRole('button', { name: /^Datei von/ })).toHaveCount(0);

    // Opened, the content stands below the main line in the row's full width.
    await file.click();
    await expect(file).toHaveAttribute('aria-expanded', 'true');
    const body = withFile.locator('.disclosure-body');
    await expect(body).toBeVisible();
    const b = await box(body);
    const line = await box(withFile.locator('.line'));
    expect(b.y).toBeGreaterThanOrEqual(m.y + m.height - 1);
    expect(b.width).toBeGreaterThan(line.width * 0.9);
  });

  test('below 768 px the toggles keep their own line', async ({ page }) => {
    await page.setViewportSize({ width: 600, height: 900 });
    await openSeries(page);
    const row = page.locator('[data-episode="S01E01"]');
    const file = await box(row.getByRole('button', { name: /^Datei von/ }));
    const subs = await box(row.getByRole('button', { name: /^Untertitel von/ }));
    const marks = await boxes(row.locator('.code, .episode-title, .air, .labels, .buttons'));
    expect(file.y).toBeGreaterThanOrEqual(Math.max(...marks.map((b) => b.bottom)) - 1);
    expect(Math.abs(file.y - subs.y)).toBeLessThanOrEqual(2);
  });
});

test.describe('head facts (AC-07)', () => {
  test('a series shows its five facts in one row from 1200 px', async ({ page }) => {
    await page.setViewportSize({ width: 1200, height: 900 });
    await openSeries(page);
    await expect(facts(page)).toHaveCount(5);
    expect(rowsOf(await boxes(facts(page))).map((r) => r.length)).toEqual([5]);
  });

  for (const width of [360, 600, 800, 1000, 1199, 1440]) {
    test(`no fact stands alone below a fuller row at ${width} px`, async ({ page }) => {
      await page.setViewportSize({ width, height: 900 });
      const check = async () => {
        await expect(facts(page).first()).toBeVisible();
        const rows = rowsOf(await boxes(facts(page))).map((r) => r.length);
        rows.forEach((n, i) => expect(n === 1 && (rows[i - 1] ?? 0) > 2, `${rows}`).toBe(false));
      };
      await openSeries(page);
      await check();
      await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, 'Film 0001')}`);
      await check();
    });
  }
});

const figure = (page: Page, label: string) =>
  page.getByRole('list', { name: 'Kennzahlen' }).getByRole('listitem').filter({ hasText: label });

test.describe('overview: empty boxes (AC-08)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });
  test('all boxes empty: no box, plain figures and the quiet line', async ({ page }) => {
    await emptyRows(page);
    await overviewWith(page, EMPTY_OVERVIEW);
    await page.goto(`${LIBRARY}/`);
    await expect(page.getByText(ALL_QUIET)).toBeVisible();
    for (const name of BOXES) {
      await expect(page.getByRole('heading', { name, exact: true })).toHaveCount(0);
    }
    await expect(page.getByText('Nichts in der Warteschlange.')).toHaveCount(0);
    await expect(page.getByText('Keine Probleme in den letzten 7 Tagen.')).toHaveCount(0);
    for (const label of ['Warteschlange', 'Probleme (7 Tage)', 'Nächste 7 Tage']) {
      await expect(figure(page, label)).toContainText('0');
      await expect(figure(page, label).getByRole('link')).toHaveCount(0);
    }
  });

  test('a queue box with the no-client line shows; the quiet line does not', async ({ page }) => {
    await overviewWith(page, (a) => {
      const empty = EMPTY_OVERVIEW(a);
      return { ...empty, queue: { ...empty.queue, client_configured: false } };
    });
    await page.goto(`${LIBRARY}/`);
    await expect(page.getByRole('region', { name: 'Warteschlange' })).toContainText(
      'Kein Download-Client eingerichtet',
    );
    await expect(page.getByText(ALL_QUIET)).toHaveCount(0);
    await expect(figure(page, 'Warteschlange').getByRole('link')).toHaveCount(1);
    await expect(page.getByRole('heading', { name: 'Letzte Aktivität' })).toHaveCount(0);
  });

  test('a queue box with an unreachable client shows its warning', async ({ page }) => {
    await overviewWith(page, (a) => {
      const empty = EMPTY_OVERVIEW(a);
      return { ...empty, queue: { ...empty.queue, notice: 'Download-Client nicht erreichbar' } };
    });
    await page.goto(`${LIBRARY}/`);
    await expect(page.getByRole('region', { name: 'Warteschlange' })).toContainText(
      'Download-Client nicht erreichbar',
    );
  });

  test('only problems: that box takes the full width and its figure links to it', async ({
    page,
  }) => {
    await overviewWith(page, (a) => ({ ...EMPTY_OVERVIEW(a), problems: [PROBLEM] }));
    await page.goto(`${LIBRARY}/`);
    const problems = page.getByRole('region', { name: 'Probleme der letzten 7 Tage' });
    await expect(problems).toBeVisible();
    await expect(page.getByText(ALL_QUIET)).toHaveCount(0);
    await expect(page.getByRole('heading', { name: 'Warteschlange' })).toHaveCount(0);
    await expect(figure(page, 'Probleme (7 Tage)').getByRole('link')).toHaveAttribute(
      'href',
      '#problems-title',
    );
    await expect(figure(page, 'Warteschlange').getByRole('link')).toHaveCount(0);
    const figures = await box(page.getByRole('list', { name: 'Kennzahlen' }));
    expect((await box(problems)).width).toBeGreaterThan(figures.width - 2);
  });

  test('a box hides and shows again with the refreshes', async ({ page }) => {
    const real = (await (
      await page.request.get(`${LIBRARY}/api/overview`)
    ).json()) as OverviewAnswer;
    let withProblem = true;
    await emptyRows(page);
    await page.route('**/api/overview', (route) =>
      route.fulfill({
        json: { ...EMPTY_OVERVIEW(real), problems: withProblem ? [PROBLEM] : [] },
      }),
    );
    await page.clock.install();
    await page.goto(`${LIBRARY}/`);
    const heading = page.getByRole('heading', { name: 'Probleme der letzten 7 Tage' });
    await expect(heading).toBeVisible();
    withProblem = false;
    await page.clock.runFor(16_000);
    await expect(heading).toHaveCount(0);
    await expect(page.getByText(ALL_QUIET)).toBeVisible();
    withProblem = true;
    await page.clock.runFor(16_000);
    await expect(heading).toBeVisible();
    await expect(page.getByText(ALL_QUIET)).toHaveCount(0);
  });
});

test.describe('overview: upcoming entries (AC-09)', () => {
  test('no chip for entries that are not out yet; other states keep theirs', async ({ page }) => {
    const day = await page.evaluate(() => new Date().toISOString().slice(0, 10));
    await overviewWith(page, (a) => ({
      ...a,
      upcoming: [entry(day, 'Kommt noch', 'upcoming'), entry(day, 'Fehlt schon', 'missing')],
    }));
    await page.goto(`${LIBRARY}/`);
    const upcoming = page.getByRole('region', { name: 'Die nächsten 7 Tage' });
    const lines = upcoming.getByTestId('calendar-entry');
    await expect(lines).toHaveCount(2);
    await expect(lines.nth(0)).toContainText('Kommt noch');
    await expect(upcoming.getByText('noch nicht erschienen')).toHaveCount(0);
    await expect(lines.nth(1)).toContainText('fehlt');
  });
});

/** The line is there once, and no tile, box or heading follows it. */
async function expectAtEnd(page: Page): Promise<void> {
  const line = page.getByTestId('fetched');
  await expect(line).toHaveCount(1);
  await expect(line).toHaveText(/^Stand: \d{2}\.\d{2}\.\d{4}, \d{2}:\d{2}$/);
  const after = await line.evaluate((el) => {
    const main = el.closest('main') ?? document.body;
    return [...main.querySelectorAll('[data-testid="title-card"], .box, h2, input, form')].filter(
      (other) => (el.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
    ).length;
  });
  expect(after).toBe(0);
  const attribution = page.locator('.imdb-attribution');
  if ((await attribution.count()) > 0) {
    expect((await box(line)).bottom).toBeLessThanOrEqual((await box(attribution)).y + 1);
  }
}

test.describe('freshness line (AC-10)', () => {
  for (const [name, path] of [
    ['discover', '/entdecken'],
    ['filtered list', '/entdecken?art=filme&genre=18'],
    ['search', '/entdecken/suche?begriff=Insel'],
    ['preview', '/entdecken/filme/502'],
    ['person', '/entdecken/personen/9000'],
  ] as const) {
    test(`${name}: once, at the end of the page`, async ({ page }) => {
      await page.goto(`${DISCOVER}${path}`);
      await settle(page);
      await expectAtEnd(page);
    });
  }

  test('shelf page: once, at the end of the page', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    await page.getByRole('link', { name: /^Mehr/ }).first().click();
    await settle(page);
    await expect(page.getByTestId('title-card').first()).toBeVisible();
    await expectAtEnd(page);
  });

  test('library movie: the cast’s line stands at the end, not in the cast box', async ({
    page,
  }) => {
    await page.goto(`${DISCOVER}/filme/${await movieId(page.request, DISCOVER, 'Das Boot')}`);
    await expect(page.getByRole('list', { name: 'Besetzung' })).toBeVisible();
    await expect(page.locator('.cast').getByTestId('fetched')).toHaveCount(0);
    await expectAtEnd(page);
  });

  test('library series: the cast’s line stands at the end, not in the cast box', async ({
    page,
  }) => {
    // The discover library's series has no TMDB id and so no cast: its cast request is
    // answered with the cast answer of the library movie.
    const movie = await movieId(page.request, DISCOVER, 'Das Boot');
    const cast: unknown = await (
      await page.request.get(`${DISCOVER}/api/library/movies/${movie}/cast`)
    ).json();
    await page.route('**/api/library/series/*/cast', (route) => route.fulfill({ json: cast }));
    await page.goto(`${DISCOVER}/serien/${await seriesId(page.request, DISCOVER, 'Kleine Insel')}`);
    await expect(page.getByRole('list', { name: 'Besetzung' })).toBeVisible();
    await expect(page.locator('.cast').getByTestId('fetched')).toHaveCount(0);
    await expectAtEnd(page);
  });

  test('a library page without a cast shows no freshness line', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, 'Film 0001')}`);
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
    await page.waitForLoadState('networkidle');
    await expect(page.getByRole('list', { name: 'Besetzung' })).toHaveCount(0);
    await expect(page.getByTestId('fetched')).toHaveCount(0);
  });

  test('a stale answer keeps its warning on top and the line at the end', async ({ page }) => {
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
    await settle(page);
    const notice = page.getByTestId('stale-notice');
    await expect(notice).toBeVisible();
    expect((await box(notice)).y).toBeLessThan((await box(page.getByTestId('shelf').first())).y);
    await expectAtEnd(page);
  });
});

const look = (l: Locator) =>
  l.evaluate((el) => {
    const s = getComputedStyle(el);
    return {
      appearance: s.appearance,
      image: s.backgroundImage,
      repeat: s.backgroundRepeat,
      paddingEnd: parseFloat(s.paddingRight),
      height: el.getBoundingClientRect().height,
      radius: s.borderTopLeftRadius,
      border: s.borderTopColor,
      ground: s.backgroundColor,
      opacity: s.opacity,
      outline: s.outlineStyle,
    };
  });

test.describe('dropdowns (AC-11)', () => {
  for (const palette of PALETTES) {
    test(`a select has the field look and a chevron of its own, ${palette}`, async ({ page }) => {
      await storeTheme(page, palette);
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(`${LIBRARY}/filme`);
      const select = page.getByRole('toolbar', { name: 'Liste' }).getByRole('combobox');
      const field = page.getByRole('toolbar', { name: 'Liste' }).getByRole('searchbox');
      await expect(select).toBeVisible();
      const s = await look(select);
      const f = await look(field);
      expect(s.appearance).toBe('none');
      expect(s.image).toContain('url(');
      expect(s.repeat).toBe('no-repeat');
      expect(s.paddingEnd).toBeGreaterThanOrEqual(28);
      expect(Math.abs(s.height - f.height)).toBeLessThanOrEqual(1);
      expect(s.radius).toBe(f.radius);
      expect(s.border).toBe(f.border);
      expect(s.ground).toBe(f.ground);
      // The chevron is drawn in the palette's own colour.
      const other = palette === 'dark' ? 'light' : 'dark';
      await page.evaluate((t) => document.documentElement.setAttribute('data-theme', t), other);
      expect((await look(select)).image).not.toBe(s.image);
    });
  }

  test('focused by keyboard a select shows the focus ring; disabled it is dimmed', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, 'Film 0001')}`);
    const select = page.getByLabel('Qualitätsprofil');
    await expect(select).toBeVisible();
    expect((await look(select)).appearance).toBe('none');
    // The boxes before the select load their buttons one by one; a button that arrives
    // between the two key presses would take the focus. Wait until the page is at rest.
    await page.waitForLoadState('networkidle');
    await select.focus();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    await expect(select).toBeFocused();
    expect((await look(select)).outline).not.toBe('none');
    await select.evaluate((el) => ((el as HTMLSelectElement).disabled = true));
    expect(Number((await look(select)).opacity)).toBeLessThan(1);
  });
});

test.describe('month grid: weeks without entries (AC-12)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('an empty week is at most 72 px high; a week with entries grows', async ({ page }) => {
    await page.route('**/api/calendar?*', async (route) => {
      const answer = await route.fetch();
      const body = (await answer.json()) as CalendarAnswer;
      body.note = null;
      body.entries = [
        entry('2026-11-05', 'Stadt der Uhren', 'present'),
        entry('2026-11-05', 'Funkstille', 'missing'),
        entry('2026-11-05', 'Hafenlichter', 'upcoming'),
        entry('2026-11-12', 'Der zwölfte Mond', 'upcoming'),
      ];
      await route.fulfill({ response: answer, json: body });
    });
    await page.goto(`${LIBRARY}/kalender?ansicht=monat&datum=2026-11-16`);
    await expect(page.getByTestId('calendar-entry').first()).toBeVisible();
    const cells = page.locator('.month .cell');
    const rows = rowsOf(await boxes(cells));
    // November 2026: six week rows, entries in the second and third.
    expect(rows.map((r) => r.length)).toEqual([7, 7, 7, 7, 7, 7]);
    const heights = rows.map((r) => r[0]?.height ?? 0);
    for (const i of [0, 3, 4, 5]) expect(heights[i], `row ${i}`).toBeLessThanOrEqual(72);
    expect(heights[2]).toBeGreaterThan(72);
    expect(heights[1]).toBeGreaterThan(heights[2] ?? 0);
    await expect(cells.locator('.number')).toHaveCount(42);
    for (const n of await boxes(cells.locator('.number'))) expect(n.height).toBeGreaterThan(0);
  });
});

test.describe('website screenshots: the wait for ratings (AC-14)', () => {
  test('ends with an error while a rating stays pending', async ({ page }) => {
    await pendingRatings(page);
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    await expect(ratingsSettled(page, { tries: 2, pauseMs: 100 })).rejects.toThrow(
      /still pending: no picture taken/,
    );
  });

  test('returns once no rating is pending', async ({ page, request }) => {
    await waitForRatings(request);
    await page.goto(`${RATINGS}/filme`);
    await expect(items(page).first()).toBeVisible();
    await ratingsSettled(page, { tries: 2, pauseMs: 100 });
    await expect(page.locator('[data-testid="rating"]').first()).toBeVisible();
  });
});

for (const palette of PALETTES) {
  for (const width of [1440, 360]) {
    test.describe(`accessibility (AC-13), ${palette}, ${width} px`, () => {
      test.beforeEach(async ({ page }) => {
        await storeTheme(page, palette);
        await page.setViewportSize({ width, height: 900 });
      });

      test('overview with all boxes hidden and with only some', async ({ page }) => {
        await emptyRows(page);
        await overviewWith(page, EMPTY_OVERVIEW);
        await page.goto(`${LIBRARY}/`);
        await expect(page.getByText(ALL_QUIET)).toBeVisible();
        await axe(page);
        await page.unroute('**/api/overview');
        await overviewWith(page, (a) => ({ ...EMPTY_OVERVIEW(a), problems: [PROBLEM] }));
        await page.reload();
        await expect(page.getByRole('region', { name: /^Probleme/ })).toBeVisible();
        await axe(page);
      });

      test('a poster grid with long titles and a dropdown', async ({ page }) => {
        await posterView(page);
        await moviesWith(page, mixTiles);
        await page.goto(`${LIBRARY}/filme`);
        await expect(page.locator(`a[title="${LONG_TITLE}"]`)).toBeVisible();
        await axe(page);
      });

      test('a discover shelf with a pending rating', async ({ page }) => {
        await pendingRatings(page);
        await page.goto(`${DISCOVER}/entdecken`);
        await settle(page);
        await expect(page.locator('[data-testid="rating"].pending').first()).toBeVisible();
        await axe(page);
      });

      test('a series page with an opened episode', async ({ page }) => {
        await openSeries(page);
        const file = page.getByRole('button', { name: /^Datei von/ }).first();
        await file.click();
        await expect(page.locator('.disclosure-body').first()).toBeVisible();
        await axe(page);
      });

      test('the month calendar', async ({ page }) => {
        await page.goto(`${LIBRARY}/kalender?ansicht=monat`);
        await expect(page.getByTestId('range')).toBeVisible();
        await page.waitForFunction(
          () => document.querySelector('ol.days, [data-testid="calendar-note"]') !== null,
        );
        await axe(page);
      });
    });
  }
}
