// Spec ui-cinema-tiles AC-01 .. AC-09: poster tiles that are only their poster at rest, a
// state mark in the poster's corner, and an overlay with the title's information on hover or
// keyboard focus (always on touch devices).

import { AxeBuilder } from '@axe-core/playwright';
import { devices, expect, test, type Locator, type Page } from '@playwright/test';
import type { MovieListItem } from '../src/lib/api/generated/MovieListItem';
import type { Page as ListPage } from '../src/lib/api/generated/Page';
import type { SeriesListItem } from '../src/lib/api/generated/SeriesListItem';
import { DISCOVER, PLAIN, cards, settle } from './discover-helpers';
import { RATINGS, waitForRatings } from './extras-helpers';
import { LIBRARY, items } from './library-helpers';
import { storeTheme, token } from './theme-helpers';

const { defaultBrowserType: _browser, ...touch } = devices['Pixel 7'];

const DARK = { accent: token('dark', 'accent'), danger: token('dark', 'danger') };

/** Shows the movie and series lists as poster grids. */
async function posterView(page: Page): Promise<void> {
  await page.addInitScript(() => {
    localStorage.setItem('sofarr-ansicht-filme', 'poster');
    localStorage.setItem('sofarr-ansicht-serien', 'poster');
  });
}

/** The library tile of a title (its link's tooltip is the full title). */
const tileOf = (page: Page, title: string): Locator =>
  items(page).filter({ has: page.locator(`a[title="${title}"]`) });

/** The library tile with this id. */
const byId = (page: Page, id: number): Locator =>
  page.locator(`[data-testid="library-items"] > li[data-id="${id}"]`);

const overlayOf = (tile: Locator) => tile.getByTestId('tile-overlay');
const markOf = (tile: Locator) => tile.getByTestId('state-mark');
const frameOf = (tile: Locator) => tile.locator('.frame');
const posterOf = (tile: Locator) => tile.locator('.poster-wrap');

/** The opacity an element is painted with: its own times its ancestors'. */
const shown = (l: Locator): Promise<number> =>
  l.evaluate((el) => {
    let product = 1;
    for (let node: Element | null = el; node !== null; node = node.parentElement) {
      product *= Number(getComputedStyle(node).opacity);
    }
    return product;
  });

/** How much a tile is grown: its painted width over its layout width. */
const growth = (tile: Locator): Promise<number> =>
  frameOf(tile).evaluate(
    (el) => el.getBoundingClientRect().width / parseFloat(getComputedStyle(el).width),
  );

type Box = { x: number; y: number; width: number; height: number; bottom: number; right: number };

const rect = (l: Locator): Promise<Box> =>
  l.evaluate((el) => {
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom, right: r.right };
  });

const rects = (l: Locator): Promise<Box[]> =>
  l.evaluateAll((els) =>
    els.map((el) => {
      const r = el.getBoundingClientRect();
      return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom, right: r.right };
    }),
  );

/**
 * The texts of a tile that are painted (opacity above 0), apart from its controls' names and
 * the poster (a placeholder shows the title's initial).
 */
const paintedTexts = (tile: Locator): Promise<string[]> =>
  tile.evaluate((li) => {
    const out: string[] = [];
    const walker = document.createTreeWalker(li, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      const text = (node.textContent ?? '').trim();
      const el = node.parentElement;
      if (text === '' || el === null) continue;
      if (el.closest('.sr-only') !== null || el.closest('.poster-wrap') !== null) continue;
      let opacity = 1;
      for (let e: Element | null = el; e !== null; e = e.parentElement) {
        opacity *= Number(getComputedStyle(e).opacity);
      }
      if (opacity > 0) out.push(text);
    }
    return out;
  });

/** Nothing hovered or focused: every tile at rest. */
async function rest(page: Page): Promise<void> {
  await page.mouse.move(1, 1);
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
}

async function openMovies(page: Page, base = LIBRARY, query = ''): Promise<MovieListItem[]> {
  const answer = page.waitForResponse((r) => r.url().includes('/api/library/movies?'));
  await page.goto(`${base}/filme${query}`);
  const list = (await (await answer).json()) as ListPage<MovieListItem>;
  await expect(items(page).first()).toBeVisible();
  return list.items;
}

async function openSeries(page: Page, base = LIBRARY): Promise<SeriesListItem[]> {
  const answer = page.waitForResponse((r) => r.url().includes('/api/library/series?'));
  await page.goto(`${base}/serien`);
  const list = (await (await answer).json()) as ListPage<SeriesListItem>;
  await expect(items(page).first()).toBeVisible();
  return list.items;
}

async function openDiscover(page: Page, base = DISCOVER): Promise<void> {
  await page.goto(`${base}/entdecken`);
  await settle(page);
  await expect(page.getByTestId('title-card').first()).toBeVisible();
}

/** The first shelf's tiles. */
const shelfTiles = (page: Page) => page.getByTestId('shelf').first().getByTestId('title-card');

test.beforeEach(async ({ page }) => storeTheme(page, 'dark'));

test.describe('at rest on a hover device (AC-01)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a library tile is only its poster, with its controls and its mark', async ({ page }) => {
    await posterView(page);
    await openMovies(page, LIBRARY, '?suche=Film%20000');
    await rest(page);
    const all = items(page);
    const count = await all.count();
    expect(count).toBeGreaterThanOrEqual(3);
    for (let i = 0; i < count; i += 1) {
      const tile = all.nth(i);
      expect(await paintedTexts(tile), `tile ${i}`).toEqual([]);
      expect(await shown(overlayOf(tile))).toBe(0);
      const [li, poster] = [await rect(tile), await rect(posterOf(tile))];
      expect(Math.abs(li.height - poster.height)).toBeLessThanOrEqual(1);
      expect(await growth(tile)).toBeCloseTo(1, 3);
    }
    // The monitor toggle stays; the checkbox waits for hover or focus.
    const first = tileOf(page, 'Film 0001');
    expect(await shown(first.getByRole('button', { name: /^Überwachen/ }))).toBe(1);
    expect(await shown(first.locator('.select'))).toBe(0);
  });

  test('a discover tile is only its poster and its mark', async ({ page }) => {
    await openDiscover(page);
    await rest(page);
    const tiles = shelfTiles(page);
    const count = await tiles.count();
    for (let i = 0; i < Math.min(count, 8); i += 1) {
      const tile = tiles.nth(i);
      expect(await paintedTexts(tile), `tile ${i}`).toEqual([]);
      expect(await shown(overlayOf(tile))).toBe(0);
      const [li, poster] = [await rect(tile), await rect(posterOf(tile))];
      expect(Math.abs(li.height - poster.height)).toBeLessThanOrEqual(1);
    }
  });
});

/** The colour of a mark's dot. */
const markColour = (mark: Locator) => mark.evaluate((el) => getComputedStyle(el).backgroundColor);

async function expectMark(tile: Locator, text: string | null, tone?: 'accent' | 'danger') {
  const mark = markOf(tile);
  if (text === null) {
    await expect(mark).toHaveCount(0);
    return;
  }
  await expect(mark).toHaveAccessibleName(text);
  await expect(mark).toHaveAttribute('title', text);
  expect(await markColour(mark)).toBe(DARK[tone ?? 'accent']);
  // In the lower left corner of the poster, on it.
  const [m, p] = [await rect(mark), await rect(posterOf(tile))];
  expect(m.x).toBeGreaterThanOrEqual(p.x);
  expect(m.right).toBeLessThan(p.x + p.width / 3);
  expect(m.bottom).toBeLessThanOrEqual(p.bottom);
  expect(m.y).toBeGreaterThan(p.bottom - p.height / 6);
  expect(await shown(mark)).toBe(1);
}

test.describe('state mark (AC-02)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a movie: accent with a file, danger while missing, none otherwise', async ({ page }) => {
    await posterView(page);
    await openMovies(page, LIBRARY, '?suche=Das%20Boot');
    await expectMark(tileOf(page, 'Das Boot'), 'vorhanden', 'accent');
    await openMovies(page, LIBRARY, '?suche=Fehlt%20Sehr');
    await expectMark(tileOf(page, 'Fehlt Sehr'), 'fehlt', 'danger');
    // Every tile of a page follows the list API.
    const list = await openMovies(page, LIBRARY, '?suche=Film%20000');
    for (const movie of list) {
      const tile = byId(page, movie.id);
      if (movie.quality !== null) await expectMark(tile, 'vorhanden', 'accent');
      else if (movie.missing) await expectMark(tile, 'fehlt', 'danger');
      else await expectMark(tile, null);
    }
    expect(list.some((m) => m.quality === null && !m.missing)).toBe(true);
  });

  test('a series: danger while episodes are missing, accent when complete', async ({ page }) => {
    await posterView(page);
    const list = await openSeries(page);
    for (const series of list) {
      const tile = byId(page, series.id);
      if (series.missing > 0) {
        const text = series.missing === 1 ? '1 fehlt' : `${series.missing} fehlen`;
        await expectMark(tile, text, 'danger');
      } else if (series.complete) await expectMark(tile, 'komplett', 'accent');
      else await expectMark(tile, null);
    }
    expect(list.some((s) => s.missing > 0)).toBe(true);
  });

  test('a discover tile: in the library, excluded, or none', async ({ page }) => {
    await openDiscover(page);
    await expectMark(cards(page, 'movie/501').first(), 'In der Bibliothek', 'accent');
    await expectMark(cards(page, 'movie/502').first(), null);
    await page.goto(`${DISCOVER}/entdecken/suche?begriff=Ausgeschlossen`);
    await settle(page);
    await expectMark(cards(page, 'movie/503').first(), 'Ausgeschlossen', 'danger');
  });
});

/** Hovers a tile and checks that it opens over its neighbour and closes again (AC-03). */
async function expectOpens(page: Page, tile: Locator, neighbour: Locator, texts: RegExp[]) {
  await rest(page);
  const before = await rect(tile);
  await tile.hover();
  await page.waitForTimeout(200);
  const grown = await growth(tile);
  expect(grown).toBeGreaterThanOrEqual(1.05);
  expect(grown).toBeLessThanOrEqual(1.1);
  expect(await shown(overlayOf(tile))).toBe(1);
  // Above its neighbours.
  const [z, zn] = [
    await frameOf(tile).evaluate((el) => Number(getComputedStyle(el).zIndex) || 0),
    await frameOf(neighbour).evaluate((el) => Number(getComputedStyle(el).zIndex) || 0),
  ];
  expect(z).toBeGreaterThan(zn);
  // Over the lower part of the poster, inside the tile.
  const [o, p] = [await rect(overlayOf(tile)), await rect(posterOf(tile))];
  expect(o.y).toBeGreaterThan(p.y);
  expect(o.bottom).toBeLessThanOrEqual(p.bottom + 1);
  expect(o.x).toBeGreaterThanOrEqual(p.x - 1);
  expect(o.right).toBeLessThanOrEqual(p.right + 1);
  const painted = (await paintedTexts(tile)).join(' | ');
  for (const text of texts) expect(painted).toMatch(text);
  // The title in at most two lines.
  const title = overlayOf(tile).locator('.title');
  const lines = await title.evaluate(
    (el) => el.getBoundingClientRect().height / parseFloat(getComputedStyle(el).lineHeight),
  );
  expect(lines).toBeLessThanOrEqual(2.05);
  // Back at rest.
  await page.mouse.move(1, 1);
  await expect.poll(() => shown(overlayOf(tile))).toBe(0);
  await expect.poll(() => growth(tile)).toBeCloseTo(1, 3);
  expect(await rect(tile)).toEqual(before);
}

test.describe('open tile on a hover device (AC-03)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a library tile grows and shows title, year, addition, rating and chips', async ({
    page,
    request,
  }) => {
    await waitForRatings(request);
    await posterView(page);
    await openMovies(page, RATINGS);
    const tile = tileOf(page, 'Bestnote');
    const neighbour = items(page)
      .filter({ hasNot: page.locator('a[title="Bestnote"]') })
      .first();
    await expectOpens(page, tile, neighbour, [/^Bestnote/, /\d{4}/, /★ 9,1/]);
    // Its chips of today.
    await tile.hover();
    await expect.poll(() => shown(tile.locator('.labels .label').first())).toBe(1);
  });

  test('a long title is cut after two lines with an ellipsis', async ({ page }) => {
    const long =
      'Die außerordentlich lange und verschlungene Geschichte vom Leuchtturmwärter am Ende der Welt';
    await posterView(page);
    await page.route(`${LIBRARY}/api/library/movies?*`, async (route) => {
      const answer = await route.fetch();
      const list = (await answer.json()) as ListPage<MovieListItem>;
      const first = list.items[0];
      if (first !== undefined) first.title = long;
      await route.fulfill({ response: answer, json: list });
    });
    await openMovies(page);
    const tile = tileOf(page, long);
    await tile.hover();
    const title = overlayOf(tile).locator('.title');
    await expect(title).toHaveText(long);
    expect(await title.evaluate((el) => el.scrollHeight > el.clientHeight + 1)).toBe(true);
    expect(await title.evaluate((el) => getComputedStyle(el).webkitLineClamp)).toBe('2');
  });

  test('a discover tile grows and shows title, year, rating and the add button', async ({
    page,
  }) => {
    await openDiscover(page);
    const one = cards(page.getByTestId('shelf').first(), 'movie/502');
    const neighbour = page
      .getByTestId('shelf')
      .first()
      .locator('[data-testid="title-card"]:not([data-key="movie/502"])')
      .first();
    await expectOpens(page, one, neighbour, [/^Sternenfahrt/, /2024/, /★ 7,8/, /Hinzufügen/]);
    await one.hover();
    const add = one.getByRole('button', { name: 'Hinzufügen „Sternenfahrt“' });
    await expect.poll(() => shown(add)).toBe(1);
  });

  test('one tile at a time: the one under the pointer before the focused one', async ({ page }) => {
    await openDiscover(page);
    const [a, b] = [shelfTiles(page).nth(1), shelfTiles(page).nth(2)];
    await rest(page);
    await a.getByRole('link').focus();
    await expect.poll(() => shown(overlayOf(a))).toBe(1);
    await b.hover();
    await expect.poll(() => shown(overlayOf(b))).toBe(1);
    await expect.poll(() => shown(overlayOf(a))).toBe(0);
    expect(await growth(a)).toBeCloseTo(1, 3);
    // Pointer away: the focused tile is open again.
    await page.mouse.move(1, 1);
    await expect.poll(() => shown(overlayOf(a))).toBe(1);
    await expect.poll(() => shown(overlayOf(b))).toBe(0);
  });

  test('with reduced motion the tile does not grow and the overlay shows at once', async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openDiscover(page);
    const tile = shelfTiles(page).nth(1);
    await rest(page);
    await tile.hover();
    // No waiting: no transition runs.
    expect(await shown(overlayOf(tile))).toBe(1);
    expect(await growth(tile)).toBeCloseTo(1, 3);
  });
});

test.describe('name and keyboard (AC-05)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a library tile is named with title, year and rating with votes', async ({
    page,
    request,
  }) => {
    await waitForRatings(request);
    await posterView(page);
    const list = await openMovies(page, RATINGS);
    const best = list.find((m) => m.title === 'Bestnote');
    const none = list.find((m) => m.title === 'Ohne IMDb');
    if (best === undefined || none === undefined) throw new Error('missing titles');
    const link = (title: string) => tileOf(page, title).getByRole('link');
    await expect(link('Bestnote')).toHaveAccessibleName(
      `Bestnote, ${best.year}, IMDb 9,1 (250.000 Stimmen)`,
    );
    await expect(link('Bestnote')).toHaveAttribute('title', 'Bestnote');
    await expect(link('Ohne IMDb')).toHaveAccessibleName(`Ohne IMDb, ${none.year}, IMDb –`);
    // The same while open.
    await tileOf(page, 'Bestnote').hover();
    await expect(link('Bestnote')).toHaveAccessibleName(
      `Bestnote, ${best.year}, IMDb 9,1 (250.000 Stimmen)`,
    );
  });

  test('a discover tile is named with title, year and rating; nothing while off', async ({
    page,
  }) => {
    await openDiscover(page);
    const link = cards(page, 'movie/502').first().getByRole('link');
    await expect(link).toHaveAccessibleName('Sternenfahrt, 2024, IMDb 7,8 (12.345 Stimmen)');
    await expect(link).toHaveAttribute('title', 'Sternenfahrt');
    await expect(cards(page, 'movie/505').first().getByRole('link')).toHaveAccessibleName(
      'Ohne Bewertung, 2022, IMDb –',
    );
    await openDiscover(page, PLAIN);
    await expect(cards(page, 'movie/502').first().getByRole('link')).toHaveAccessibleName(
      'Sternenfahrt, 2024',
    );
  });

  test('Tab into a discover tile opens it; its button shows its focus', async ({ page }) => {
    await openDiscover(page);
    const tile = cards(page.getByTestId('shelf').first(), 'movie/502');
    await rest(page);
    await tile.getByRole('link').focus();
    await page.keyboard.press('Shift+Tab');
    expect(await shown(overlayOf(tile))).toBe(0);
    await page.keyboard.press('Tab');
    await expect(tile.getByRole('link')).toBeFocused();
    await expect.poll(() => shown(overlayOf(tile))).toBe(1);
    await page.keyboard.press('Tab');
    const add = tile.getByRole('button', { name: 'Hinzufügen „Sternenfahrt“' });
    await expect(add).toBeFocused();
    expect(await add.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
    expect(await shown(add)).toBe(1);
  });

  test('Tab into a library tile opens it; checkbox and toggle show their focus', async ({
    page,
  }) => {
    await posterView(page);
    await openMovies(page, LIBRARY, '?suche=Film%20000');
    const tile = tileOf(page, 'Film 0001');
    await rest(page);
    await tile.getByRole('link').focus();
    await expect.poll(() => shown(overlayOf(tile))).toBe(1);
    expect(
      await tile.getByRole('link').evaluate((el) => getComputedStyle(el).outlineStyle),
    ).not.toBe('none');
    await page.keyboard.press('Tab');
    const box = page.getByLabel('„Film 0001“ auswählen');
    await expect(box).toBeFocused();
    expect(await shown(box)).toBe(1);
    expect(await box.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
    await page.keyboard.press('Tab');
    const toggle = tile.getByRole('button', { name: 'Überwachen „Film 0001“' });
    await expect(toggle).toBeFocused();
    expect(await toggle.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
    expect(await shown(overlayOf(tile))).toBe(1);
  });
});

test.describe('touch device (AC-06)', () => {
  test.use(touch);

  test('library and discover tiles show their overlay at rest, without growing', async ({
    page,
  }) => {
    await posterView(page);
    await openMovies(page, LIBRARY, '?suche=Film%20000');
    for (const tile of (await items(page).all()).slice(0, 4)) {
      expect(await shown(overlayOf(tile))).toBe(1);
      expect(await growth(tile)).toBeCloseTo(1, 3);
    }
    await openDiscover(page);
    for (const tile of (await shelfTiles(page).all()).slice(0, 2)) {
      expect(await shown(overlayOf(tile))).toBe(1);
      expect(await growth(tile)).toBeCloseTo(1, 3);
    }
  });
});

test.describe('selection mode (AC-07)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('checkboxes show, the selected tile is outlined, the overlay leaves the controls free', async ({
    page,
  }) => {
    await posterView(page);
    await openMovies(page, LIBRARY, '?suche=Film%20000');
    const one = tileOf(page, 'Film 0001');
    await one.hover();
    await page.getByLabel('„Film 0001“ auswählen').check();
    await rest(page);
    for (const tile of await items(page).all())
      expect(await shown(tile.locator('.select'))).toBe(1);
    const outline = await posterOf(one).evaluate((el) => ({
      style: getComputedStyle(el).outlineStyle,
      color: getComputedStyle(el).outlineColor,
    }));
    expect(outline).toEqual({ style: 'solid', color: DARK.accent });
    // Open: the checkbox and the toggle stay on top and outside the overlay.
    const other = tileOf(page, 'Film 0002');
    for (const tile of [one, other]) {
      await tile.hover();
      await expect.poll(() => shown(overlayOf(tile))).toBe(1);
      const overlay = await rect(overlayOf(tile));
      for (const control of [tile.locator('.select'), tile.locator('.monitor-slot')]) {
        const c = await rect(control);
        expect(c.bottom).toBeLessThanOrEqual(overlay.y);
        const hit = await control.evaluate((el) => {
          const r = el.getBoundingClientRect();
          const top = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
          return top !== null && el.contains(top);
        });
        expect(hit).toBe(true);
      }
    }
  });
});

/** Whether all numbers are the same within a pixel. */
const level = (values: readonly number[]): boolean =>
  values.every((v) => Math.abs(v - (values[0] ?? 0)) <= 1);

test.describe('even tiles (AC-08)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a grid row is as high as its posters; an open tile moves no neighbour', async ({
    page,
  }) => {
    await posterView(page);
    await openMovies(page);
    await rest(page);
    const all = items(page);
    const tiles = await rects(all);
    const firstRow = tiles.filter((t) => Math.abs(t.y - (tiles[0]?.y ?? 0)) <= 2);
    expect(firstRow.length).toBeGreaterThanOrEqual(3);
    const posters = (await rects(all.locator('.poster-wrap'))).slice(0, firstRow.length);
    expect(level(firstRow.map((t) => t.height))).toBe(true);
    firstRow.forEach((t, i) =>
      expect(Math.abs(t.height - (posters[i]?.height ?? 0))).toBeLessThanOrEqual(1),
    );
    await all.nth(1).hover();
    await page.waitForTimeout(250);
    const after = await rects(all);
    expect(after[0]).toEqual(tiles[0]);
    expect(after[2]).toEqual(tiles[2]);
  });

  test("a shelf's tiles end at one edge", async ({ page }) => {
    await openDiscover(page);
    await rest(page);
    for (const shelf of (await page.getByTestId('shelf').all()).slice(0, 3)) {
      const tiles = await rects(shelf.getByTestId('title-card'));
      expect(level(tiles.map((t) => t.bottom))).toBe(true);
      expect(level(tiles.map((t) => t.height))).toBe(true);
    }
  });
});

/** A poster of one colour, 2:3. */
const poster = (colour: 'white' | 'black'): string =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="342" height="513"><rect width="342" height="513" fill="${colour === 'white' ? '#ffffff' : '#000000'}"/></svg>`;

/** Every poster of the library and of TMDB is one colour; every library title has one. */
async function servePosters(page: Page, colour: 'white' | 'black', base: string): Promise<void> {
  await page.route('**/api/images/**', (route) =>
    route.fulfill({ contentType: 'image/svg+xml', body: poster(colour) }),
  );
  await page.route(`${base}/api/library/movies?*`, async (route) => {
    const answer = await route.fetch();
    const list = (await answer.json()) as ListPage<MovieListItem>;
    for (const item of list.items) item.has_poster = true;
    await route.fulfill({ response: answer, json: list });
  });
}

/** One sRGB channel, linear. */
function channel(v: number): number {
  const c = v / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** The relative luminance of an sRGB colour (WCAG 2.1). */
function luminance([r, g, b]: readonly number[]): number {
  return 0.2126 * channel(r ?? 0) + 0.7152 * channel(g ?? 0) + 0.0722 * channel(b ?? 0);
}

const contrast = (a: number, b: number): number =>
  (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);

/**
 * The weakest contrast of the texts of the overlays shown in the viewport against what is
 * painted behind them: every overlay text is made transparent, the box of each text
 * photographed, and its lightest and darkest pixel compared with the text's colour.
 */
async function weakestOverlayContrast(page: Page): Promise<{ text: string; ratio: number }> {
  const found = await page.evaluate(() => {
    const out: {
      text: string;
      colour: number[];
      box: { x: number; y: number; width: number; height: number };
    }[] = [];
    for (const overlay of document.querySelectorAll('[data-testid="tile-overlay"]')) {
      let opacity = 1;
      for (let e: Element | null = overlay; e !== null; e = e.parentElement) {
        opacity *= Number(getComputedStyle(e).opacity);
      }
      if (opacity === 0) continue;
      const walker = document.createTreeWalker(overlay, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
        const text = (node.textContent ?? '').trim();
        const el = node.parentElement;
        if (text === '' || el === null || el.closest('.sr-only') !== null) continue;
        const range = document.createRange();
        range.selectNodeContents(node);
        for (const r of range.getClientRects()) {
          const inside =
            r.width >= 1 &&
            r.top >= 0 &&
            r.left >= 0 &&
            r.bottom <= innerHeight &&
            r.right <= innerWidth;
          if (!inside) continue;
          out.push({
            text: text.slice(0, 40),
            colour: (getComputedStyle(el).color.match(/[\d.]+/g) ?? []).map(Number),
            box: { x: r.x, y: r.y, width: r.width, height: r.height },
          });
        }
      }
    }
    return out;
  });
  expect(found.length).toBeGreaterThanOrEqual(3);
  await page.addStyleTag({
    content:
      '[data-testid="tile-overlay"], [data-testid="tile-overlay"] * { color: transparent !important; }',
  });
  let weakest = { text: '', ratio: Number.POSITIVE_INFINITY };
  for (const t of found) {
    const shot = await page.screenshot({ clip: t.box });
    const pixels = await page.evaluate(async (data) => {
      const img = new Image();
      img.src = `data:image/png;base64,${data}`;
      await img.decode();
      const canvas = document.createElement('canvas');
      canvas.width = img.naturalWidth;
      canvas.height = img.naturalHeight;
      const context = canvas.getContext('2d');
      if (context === null) return [];
      context.drawImage(img, 0, 0);
      const { data: rgba } = context.getImageData(0, 0, canvas.width, canvas.height);
      let light = [0, 0, 0];
      let dark = [255, 255, 255];
      for (let i = 0; i < rgba.length; i += 4) {
        const p = [rgba[i] ?? 0, rgba[i + 1] ?? 0, rgba[i + 2] ?? 0];
        const sum = p.reduce((a, c) => a + c, 0);
        if (sum > light.reduce((a, c) => a + c, 0)) light = p;
        if (sum < dark.reduce((a, c) => a + c, 0)) dark = p;
      }
      return [light, dark];
    }, shot.toString('base64'));
    for (const pixel of pixels) {
      const ratio = contrast(luminance(t.colour), luminance(pixel));
      if (ratio < weakest.ratio) weakest = { text: t.text, ratio };
    }
  }
  return weakest;
}

/** Waits until the posters in the viewport have loaded (the others load lazily). */
async function postersLoaded(page: Page): Promise<void> {
  await page.waitForFunction(() =>
    [...document.querySelectorAll<HTMLImageElement>('.poster-wrap img')]
      .filter((img) => {
        const r = img.getBoundingClientRect();
        return r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth;
      })
      .every((img) => img.complete && img.naturalWidth > 0),
  );
}

for (const colour of ['white', 'black'] as const) {
  test.describe(`contrast over an all-${colour} poster (AC-04)`, () => {
    test.describe('1440 px, an open tile', () => {
      test.use({ viewport: { width: 1440, height: 900 } });

      test('library tile', async ({ page, request }) => {
        await waitForRatings(request);
        await posterView(page);
        await servePosters(page, colour, RATINGS);
        await openMovies(page, RATINGS);
        const tile = tileOf(page, 'Bestnote');
        await tile.hover();
        await expect.poll(() => shown(overlayOf(tile))).toBe(1);
        await postersLoaded(page);
        const weakest = await weakestOverlayContrast(page);
        expect(weakest.ratio, `„${weakest.text}“`).toBeGreaterThanOrEqual(4.5);
      });

      for (const key of ['movie/502', 'movie/501']) {
        test(`discover tile ${key}`, async ({ page }) => {
          await servePosters(page, colour, DISCOVER);
          await openDiscover(page);
          const tile = cards(page.getByTestId('shelf').first(), key);
          await tile.hover();
          await expect.poll(() => shown(overlayOf(tile))).toBe(1);
          await postersLoaded(page);
          const weakest = await weakestOverlayContrast(page);
          expect(weakest.ratio, `„${weakest.text}“`).toBeGreaterThanOrEqual(4.5);
        });
      }
    });

    test.describe('360 px, a touch device', () => {
      test.use({ ...touch, viewport: { width: 360, height: 780 } });

      test('library tiles', async ({ page, request }) => {
        await waitForRatings(request);
        await posterView(page);
        await servePosters(page, colour, RATINGS);
        await openMovies(page, RATINGS);
        await postersLoaded(page);
        const weakest = await weakestOverlayContrast(page);
        expect(weakest.ratio, `„${weakest.text}“`).toBeGreaterThanOrEqual(4.5);
      });

      test('discover tiles', async ({ page }) => {
        await servePosters(page, colour, DISCOVER);
        await openDiscover(page);
        await page.getByTestId('shelf').first().scrollIntoViewIfNeeded();
        await postersLoaded(page);
        const weakest = await weakestOverlayContrast(page);
        expect(weakest.ratio, `„${weakest.text}“`).toBeGreaterThanOrEqual(4.5);
      });
    });
  });
}

type Target = { readonly name: string; readonly url: string; readonly poster?: boolean };

const AXE_TARGETS: readonly Target[] = [
  { name: 'movie list in poster view', url: `${LIBRARY}/filme`, poster: true },
  { name: 'series list in poster view', url: `${LIBRARY}/serien`, poster: true },
  { name: 'discover start page', url: `${DISCOVER}/entdecken` },
  { name: 'search', url: `${DISCOVER}/entdecken/suche?begriff=Stern` },
  { name: 'shelf page', url: `${DISCOVER}/entdecken/regal/beliebte-serien` },
  { name: 'person page', url: `${DISCOVER}/entdecken/personen/9000` },
];

async function expectNoViolations(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  expect(
    result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`),
  ).toEqual([]);
}

test.describe('accessibility in the dark palette (AC-09)', () => {
  for (const target of AXE_TARGETS) {
    for (const width of [1440, 360]) {
      test(`${target.name} at ${width} px, at rest and with a tile open`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        if (target.poster === true) await posterView(page);
        await page.goto(target.url);
        await settle(page);
        const tiles = page.locator(
          '[data-testid="library-items"] > li, [data-testid="title-card"]',
        );
        await expect(tiles.first()).toBeVisible();
        await rest(page);
        await expectNoViolations(page);
        const tile = tiles.first();
        await tile.hover();
        await expect.poll(() => shown(overlayOf(tile))).toBe(1);
        await page.waitForTimeout(250);
        await expectNoViolations(page);
      });
    }
  }
});
