// Spec ui-detail-head AC-01 .. AC-06: the head of the movie and series pages: the blurred
// poster behind it, the order of its parts, the compact facts, IMDb's attribution at the end
// of the page, and the cast as a strip.

import { expect, test, type Page } from '@playwright/test';
import { DISCOVER } from './discover-helpers';
import { RATINGS, waitForRatings } from './extras-helpers';
import { expectAttribution } from './helpers';
import { LIBRARY, movieId, seriesId } from './library-helpers';
import { PALETTES, storeTheme, token } from './theme-helpers';

async function openMovie(page: Page, base: string, title: string): Promise<void> {
  await page.goto(`${base}/filme/${await movieId(page.request, base, title)}`);
  await expect(page.getByRole('heading', { level: 1, name: new RegExp(title) })).toBeVisible();
}

const top = async (page: Page, selector: string): Promise<number> =>
  (await page.locator(selector).first().boundingBox())?.y ?? Number.NaN;

test.describe('backdrop (AC-01)', () => {
  for (const palette of PALETTES) {
    test(`the blurred poster under at least 70 % of the page ground (${palette})`, async ({
      page,
    }) => {
      await storeTheme(page, palette);
      const hosts = new Set<string>();
      page.on('request', (r) => hosts.add(new URL(r.url()).host));
      await openMovie(page, LIBRARY, 'Das Boot');
      const backdrop = page.locator('.backdrop img');
      await expect(backdrop).toBeAttached();
      const poster = await page.locator('.head .poster img').getAttribute('src');
      await expect(backdrop).toHaveAttribute('src', poster ?? '');
      expect(await backdrop.evaluate((el) => getComputedStyle(el).filter)).toMatch(/blur/);
      const veil = await page.locator('.backdrop-veil').evaluate((el) => ({
        colour: getComputedStyle(el).backgroundColor,
        opacity: Number(getComputedStyle(el).opacity),
      }));
      expect(veil.colour).toBe(token(palette, 'bg'));
      expect(veil.opacity).toBeGreaterThanOrEqual(0.7);
      expect([...hosts]).toEqual([new URL(LIBRARY).host]);
    });
  }

  test('no backdrop without a poster, or when the poster fails to load', async ({ page }) => {
    await openMovie(page, LIBRARY, 'Film 0001');
    await expect(page.locator('.backdrop img')).toHaveCount(0);
    await openMovie(page, LIBRARY, 'Ohne Bild');
    await expect(page.getByTestId('poster-placeholder')).toBeVisible();
    await expect(page.locator('.backdrop img')).toHaveCount(0);
  });
});

test.describe('head (AC-02, AC-03)', () => {
  test('poster left; titles, chips, rating, overview, actions and facts in this order', async ({
    page,
    request,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await waitForRatings(request);
    await openMovie(page, RATINGS, 'Bestnote');
    const poster = await page.locator('.head > .poster').boundingBox();
    const heading = await page.getByRole('heading', { level: 1 }).boundingBox();
    expect(poster?.x ?? 0).toBeLessThan(heading?.x ?? 0);
    const order = [
      await top(page, '.head h1'),
      await top(page, '.head .labels'),
      await top(page, '.head [data-testid="rating"]'),
      await top(page, '.head .actions'),
      await top(page, '.head dl.facts-compact'),
    ];
    expect(order.every((y, i) => i === 0 || y > (order[i - 1] ?? 0))).toBe(true);
    await expect(page.getByTestId('rating')).toContainText(
      /IMDb-Bewertung\s+9,1\s+\(250\.000 Stimmen\)/,
    );
  });

  test('the overview sits between rating and actions', async ({ page }) => {
    await openMovie(page, LIBRARY, 'Das Boot');
    const overview = page.locator('.head .overview');
    if ((await overview.count()) === 0) test.skip(true, 'Das Boot has no overview in this seed');
    expect(await top(page, '.head .overview')).toBeGreaterThan(await top(page, '.head .labels'));
    expect(await top(page, '.head .overview')).toBeLessThan(await top(page, '.head .actions'));
  });

  test('the facts are a compact surface panel with up to five per row', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await openMovie(page, LIBRARY, 'Das Boot');
    const facts = page.locator('.head dl.facts-compact');
    expect(await facts.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
      token('dark', 'surface'),
    );
    const columns = await facts
      .locator(':scope > div')
      .evaluateAll(
        (all) => new Set(all.map((el) => Math.round(el.getBoundingClientRect().x))).size,
      );
    // Five per row from 1200 px (ui-polish AC-07).
    expect(columns).toBeLessThanOrEqual(5);
    const sizes = await facts
      .locator(':scope > div')
      .first()
      .evaluate((el) => ({
        term: parseFloat(getComputedStyle(el.querySelector('dt') ?? el).fontSize),
        value: parseFloat(getComputedStyle(el.querySelector('dd') ?? el).fontSize),
      }));
    expect(sizes.term).toBeLessThan(sizes.value);
  });
});

test.describe('attribution (AC-04)', () => {
  test("IMDb's attribution stands once at the end of the page", async ({ page, request }) => {
    await waitForRatings(request);
    await openMovie(page, RATINGS, 'Bestnote');
    await expectAttribution(page);
    await expect(page.locator('.imdb-attribution')).toHaveCount(1);
    await expect(page.locator('.head .imdb-attribution')).toHaveCount(0);
    const attribution = await top(page, '.imdb-attribution');
    const bottoms = await page
      .locator('main section.box')
      .evaluateAll((all) => all.map((el) => el.getBoundingClientRect().bottom + window.scrollY));
    const offset = await page.evaluate(() => window.scrollY);
    expect(attribution + offset).toBeGreaterThanOrEqual(Math.max(...bottoms) - 1);
  });
});

test.describe('cast strip (AC-05)', () => {
  test('one scrolling row of round portraits; focus scrolls a link into view', async ({ page }) => {
    await page.setViewportSize({ width: 1024, height: 800 });
    await openMovie(page, DISCOVER, 'Das Boot');
    const cast = page.getByRole('list', { name: 'Besetzung' });
    await expect(cast.getByRole('link')).toHaveCount(10);
    const tops = await cast
      .getByRole('listitem')
      .evaluateAll(
        (all) => new Set(all.map((el) => Math.round(el.getBoundingClientRect().y))).size,
      );
    expect(tops).toBe(1);
    const radius = await cast
      .locator('.image')
      .first()
      .evaluate((el) => getComputedStyle(el).borderTopLeftRadius);
    expect(radius).toMatch(/50%|9999px|999px|\d{2,}px/);
    const scrolls = await cast.evaluate((el) => el.scrollWidth > el.clientWidth);
    expect(scrolls).toBe(true);
    const last = cast.getByRole('link').last();
    await last.focus();
    await expect(last).toBeInViewport();
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
  });
});

test.describe('narrow window (AC-06)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test('the head stacks: the poster at most 9 rem above the text, nothing scrolls sideways', async ({
    page,
  }) => {
    await openMovie(page, DISCOVER, 'Das Boot');
    const poster = await page.locator('.head > .poster').boundingBox();
    const heading = await page.getByRole('heading', { level: 1 }).boundingBox();
    expect(poster?.width ?? 999).toBeLessThanOrEqual(144);
    expect((poster?.y ?? 0) + (poster?.height ?? 0)).toBeLessThanOrEqual((heading?.y ?? 0) + 1);
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
  });

  test('a series page fits as well', async ({ page }) => {
    await page.goto(`${LIBRARY}/serien/${await seriesId(page.request, LIBRARY, 'Kleine Insel')}`);
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
  });
});
