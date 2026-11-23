// Spec ui-cinema-hero AC-02 .. AC-05, AC-09: the hero of the discover start page — the
// featured title over its backdrop, its texts' contrast over any image, when it stays
// away, adding the title from it, and accessibility. The discover server features
// „Sternenfahrt“ (TMDB 502): „Das Boot“ and „Kleine Insel“ before it are in the library.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Locator, type Page, type Route } from '@playwright/test';
import type { AddAnswer } from '../src/lib/api/generated/AddAnswer';
import type { Hero } from '../src/lib/api/generated/Hero';
import type { ShelvesAnswer } from '../src/lib/api/generated/ShelvesAnswer';
import { DISCOVER, PLAIN, settle } from './discover-helpers';
import { expectNoHorizontalScroll } from './helpers';
import { storeTheme } from './theme-helpers';

const BACKDROP = '/api/images/tmdb/w1280/b502.png';

/** A picture of one colour, 16:9. */
function picture(colour: 'white' | 'black' | 'grey'): string {
  const fill = { white: '#ffffff', black: '#000000', grey: '#808080' }[colour];
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720"><rect width="1280" height="720" fill="${fill}"/></svg>`;
}

/** Answers the hero's backdrop request with a picture of one colour. */
async function serveBackdrop(page: Page, colour: 'white' | 'black' | 'grey'): Promise<void> {
  await page.route('**/api/images/tmdb/w1280/**', (route) =>
    route.fulfill({ contentType: 'image/svg+xml', body: picture(colour) }),
  );
}

/** Changes the hero of the shelves answer (`null`: none). */
async function heroWith(page: Page, change: (hero: Hero) => Hero | null): Promise<void> {
  await page.route('**/api/discover/shelves', async (route: Route) => {
    const answer = await route.fetch();
    const body = (await answer.json()) as ShelvesAnswer;
    expect(body.hero).not.toBeNull();
    body.hero = body.hero === null ? null : change(body.hero);
    await route.fulfill({ response: answer, json: body });
  });
}

async function openDiscover(page: Page, base = DISCOVER): Promise<void> {
  await page.goto(`${base}/entdecken`);
  await settle(page);
  await expect(page.getByTestId('shelf').first()).toBeVisible();
}

const heroOf = (page: Page) => page.getByTestId('hero');
const backdropOf = (page: Page) => heroOf(page).getByTestId('head-backdrop');

/** Waits until the hero's backdrop has loaded. */
async function loaded(page: Page): Promise<void> {
  await expect(backdropOf(page)).toHaveAttribute('data-kind', 'sharp');
  await expect
    .poll(() =>
      backdropOf(page)
        .locator('img')
        .evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0),
    )
    .toBe(true);
}

const rect = (l: Locator) =>
  l.evaluate((el) => {
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  });

/** The relative luminance of an sRGB colour (WCAG 2.1). */
function channel(v: number): number {
  const c = v / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance([r, g, b]: readonly number[]): number {
  return 0.2126 * channel(r ?? 0) + 0.7152 * channel(g ?? 0) + 0.0722 * channel(b ?? 0);
}

const contrast = (a: number, b: number): number =>
  (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);

const TEXTS = '[data-testid="hero"] .text';

/**
 * The weakest contrast of the hero's texts against what is painted behind them: every
 * text is made transparent, the page photographed, and the lightest and darkest pixel of
 * each text's box compared with the text's colour.
 */
async function weakestContrast(page: Page): Promise<{ text: string; ratio: number }> {
  const texts = heroOf(page).locator('.text > h2, .text > p, .text .rating');
  const found = await texts.evaluateAll((els) =>
    els
      .filter((el) => (el.textContent ?? '').trim() !== '')
      .map((el) => {
        const r = el.getBoundingClientRect();
        const colour =
          getComputedStyle(el)
            .color.match(/[\d.]+/g)
            ?.map(Number) ?? [];
        return {
          text: (el.textContent ?? '').trim().slice(0, 40),
          colour,
          box: { x: r.x, y: r.y, width: r.width, height: r.height },
        };
      }),
  );
  expect(found.length).toBeGreaterThanOrEqual(4);
  await page.addStyleTag({
    content: `${TEXTS}, ${TEXTS} * { color: transparent !important; }`,
  });
  let weakest = { text: '', ratio: Number.POSITIVE_INFINITY };
  for (const t of found) {
    if (t.box.width < 1 || t.box.height < 1) continue;
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

test.beforeEach(async ({ page }) => storeTheme(page, 'dark'));

test.describe('the hero (AC-01, AC-02)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('the featured title above the first shelf, on its backdrop', async ({ page }) => {
    await serveBackdrop(page, 'grey');
    await openDiscover(page);
    const hero = heroOf(page);
    await loaded(page);
    await expect(hero.getByRole('heading', { level: 2 })).toHaveText('Sternenfahrt');
    await expect(hero.getByTestId('hero-meta')).toHaveText('Film · 2024 · ★ 7,8');
    await expect(hero.getByText('Sternenfahrt: ein Film zum Testen.')).toBeVisible();
    await expect(hero.getByRole('button', { name: 'Hinzufügen' })).toBeVisible();
    await expect(hero.getByRole('link', { name: 'Details' })).toHaveAttribute(
      'href',
      '/entdecken/filme/502',
    );
    await expect(backdropOf(page).locator('img')).toHaveAttribute('src', BACKDROP);

    // Above the first shelf, across the content's width, at least 420 px high.
    const [box, shelf] = [await rect(hero), await rect(page.getByTestId('shelf').first())];
    expect(box.y + box.height).toBeLessThanOrEqual(shelf.y);
    expect(Math.abs(box.width - shelf.width)).toBeLessThanOrEqual(1);
    expect(Math.abs(box.x - shelf.x)).toBeLessThanOrEqual(1);
    expect(box.height).toBeGreaterThanOrEqual(420);

    // The backdrop fills the hero, sharp, under the scrim; the text lies on it.
    const backdrop = await rect(backdropOf(page));
    expect(Math.abs(backdrop.width - box.width)).toBeLessThanOrEqual(1);
    expect(Math.abs(backdrop.height - box.height)).toBeLessThanOrEqual(1);
    expect(
      await backdropOf(page)
        .locator('img')
        .evaluate((el) => getComputedStyle(el).filter),
    ).toBe('none');
    await expect(backdropOf(page).locator('.scrim')).toHaveCount(1);
    const text = await rect(hero.locator('.text'));
    expect(text.y).toBeGreaterThanOrEqual(backdrop.y);
    expect(text.y + text.height).toBeLessThanOrEqual(backdrop.y + backdrop.height + 1);
  });

  test('at 1024 px the hero is at least 420 px high', async ({ page }) => {
    await page.setViewportSize({ width: 1024, height: 800 });
    await serveBackdrop(page, 'grey');
    await openDiscover(page);
    await loaded(page);
    expect((await rect(heroOf(page))).height).toBeGreaterThanOrEqual(420);
  });

  test('a long overview is cut after three lines', async ({ page }) => {
    const long = Array.from({ length: 60 }, (_, i) => `Satz ${i + 1} einer langen Inhaltsangabe.`);
    await heroWith(page, (hero) => ({ ...hero, overview: long.join(' ') }));
    await openDiscover(page);
    const overview = heroOf(page).locator('.overview');
    await expect(overview).toBeVisible();
    expect(await overview.evaluate((el) => getComputedStyle(el).webkitLineClamp)).toBe('3');
    expect(await overview.evaluate((el) => el.scrollHeight > el.clientHeight + 1)).toBe(true);
    const lines = await overview.evaluate(
      (el) => el.clientHeight / parseFloat(getComputedStyle(el).lineHeight),
    );
    expect(Math.round(lines)).toBe(3);
  });

  test('a series without year, rating and overview shows what is there', async ({ page }) => {
    await heroWith(page, (hero) => ({
      ...hero,
      kind: 'series',
      year: null,
      overview: null,
      rating: { state: 'none', rating: null, votes: null },
    }));
    await openDiscover(page);
    await expect(heroOf(page).getByTestId('hero-meta')).toHaveText('Serie · –');
    await expect(heroOf(page).locator('.overview')).toHaveCount(0);
  });

  test('a series with a year', async ({ page }) => {
    await heroWith(page, (hero) => ({ ...hero, kind: 'series', year: 2023 }));
    await openDiscover(page);
    await expect(heroOf(page).getByTestId('hero-meta')).toHaveText(/^Serie · 2023 · /);
  });

  test('without ratings the hero shows no rating', async ({ page }) => {
    // That server has no library: „Das Boot“ is featured.
    await openDiscover(page, PLAIN);
    await expect(heroOf(page).getByRole('heading', { level: 2 })).toHaveText('Das Boot');
    await expect(heroOf(page).getByTestId('hero-meta')).toHaveText('Film · 1981');
  });

  test('„Hinzufügen“ opens the add dialog of today', async ({ page }) => {
    await openDiscover(page);
    await heroOf(page).getByRole('button', { name: 'Hinzufügen' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('heading', { name: '„Sternenfahrt“ hinzufügen' })).toBeVisible();
    await dialog.getByRole('button', { name: 'Abbrechen' }).click();
    await expect(dialog).toBeHidden();
  });

  test('below 768 px the backdrop is a 16:9 band above the text', async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 780 });
    await serveBackdrop(page, 'grey');
    await openDiscover(page);
    await loaded(page);
    const band = await rect(backdropOf(page));
    const hero = await rect(heroOf(page));
    expect(Math.abs(band.width - hero.width)).toBeLessThanOrEqual(1);
    expect(Math.abs(band.height - (band.width * 9) / 16)).toBeLessThanOrEqual(2);
    // Backdrop, then title, data, overview and actions, on the page ground.
    const order = [
      heroOf(page).getByRole('heading', { level: 2 }),
      heroOf(page).getByTestId('hero-meta'),
      heroOf(page).locator('.overview'),
      heroOf(page).getByRole('button', { name: 'Hinzufügen' }),
    ];
    let top = band.y + band.height;
    for (const part of order) {
      const r = await rect(part);
      expect(r.y).toBeGreaterThanOrEqual(top - 1);
      top = r.y;
    }
    await expect(backdropOf(page).locator('.scrim')).toBeHidden();
    await expectNoHorizontalScroll(page);
  });
});

test.describe('contrast over any image (AC-03)', () => {
  for (const width of [1440, 1024]) {
    for (const colour of ['white', 'black'] as const) {
      test(`at least 4.5:1 over an all-${colour} backdrop at ${width} px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await serveBackdrop(page, colour);
        await openDiscover(page);
        await loaded(page);
        const weakest = await weakestContrast(page);
        expect(weakest.ratio, `„${weakest.text}“`).toBeGreaterThanOrEqual(4.5);
      });
    }
  }
});

test.describe('no hero (AC-04)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('without a featured title the shelves stand as today', async ({ page }) => {
    await heroWith(page, () => null);
    await openDiscover(page);
    await expect(heroOf(page)).toHaveCount(0);
    await expect(page.getByTestId('shelf')).toHaveCount(7);
  });

  test('a filter shows the result list without a hero', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken?art=filme&genre=18`);
    await settle(page);
    await expect(page.getByRole('heading', { name: 'Ergebnisse' })).toBeVisible();
    await expect(heroOf(page)).toHaveCount(0);
  });
});

test.describe('adding from the hero (AC-05)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('the hero shows the library state and links to the library page', async ({ page }) => {
    // The add is answered here: the server's library stays as the other tests expect it.
    await page.route('**/api/discover/add/movie', (route) =>
      route.fulfill({
        json: {
          kind: 'movie',
          id: 4242,
          title: 'Sternenfahrt',
          path: '/filme/Sternenfahrt (2024)',
          files_found: 0,
          searching: false,
        } satisfies AddAnswer,
      }),
    );
    await openDiscover(page);
    const hero = heroOf(page);
    await hero.getByRole('button', { name: 'Hinzufügen' }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: 'Hinzufügen' }).click();
    await expect(dialog).toBeHidden();
    await expect(hero.getByText('In der Bibliothek')).toBeVisible();
    await expect(hero.getByRole('button', { name: 'Hinzufügen' })).toHaveCount(0);
    const library = hero.getByRole('link', { name: 'Zur Bibliothek' });
    await expect(library).toHaveAttribute('href', '/filme/4242');
    await expect(library).toBeFocused();
  });
});

test.describe('accessibility (AC-09)', () => {
  for (const width of [1440, 360]) {
    test(`no violation and a visible focus at ${width} px`, async ({ page }) => {
      await page.setViewportSize({ width, height: 900 });
      await serveBackdrop(page, 'white');
      await openDiscover(page);
      await loaded(page);
      const result = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
        .analyze();
      expect(
        result.violations.map(
          (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
        ),
      ).toEqual([]);
      await page.waitForLoadState('networkidle');
      for (const control of [
        heroOf(page).getByRole('button', { name: 'Hinzufügen' }),
        heroOf(page).getByRole('link', { name: 'Details' }),
      ]) {
        await control.focus();
        await page.keyboard.press('Shift+Tab');
        await page.keyboard.press('Tab');
        await expect(control).toBeFocused();
        expect(await control.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe('none');
      }
    });
  }
});
