// Spec ui-cinema-backdrops AC-05 .. AC-08, AC-10: the sharp backdrop behind the head of
// the movie, series and preview pages, its scrim, the fallbacks and the unchanged layout.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { DISCOVER, settle } from './discover-helpers';
import { LIBRARY, movieId, seriesId } from './library-helpers';
import { storeTheme } from './theme-helpers';

const BACKDROP = '/api/images/movie/1/backdrop?v=test';
const OVERVIEW =
  'Eine Meteorologin überwintert allein auf einer Messstation am Polarkreis – bis ein Fremder mit einem Funkgerät vor ihrer Tür steht.';

/** A picture of one colour, 16:9. */
function picture(colour: 'white' | 'black' | 'grey'): string {
  const fill = { white: '#ffffff', black: '#000000', grey: '#808080' }[colour];
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720"><rect width="1280" height="720" fill="${fill}"/></svg>`;
}

type Served = { colour: 'white' | 'black' | 'grey'; fail?: boolean; hold?: Promise<void> };

/** Answers the backdrop requests of the library pages and of the preview page. */
async function serveBackdrop(page: Page, served: Served): Promise<void> {
  const answer = async (route: import('@playwright/test').Route) => {
    if (served.hold !== undefined) await served.hold;
    if (served.fail === true) return route.fulfill({ status: 404, body: '' });
    return route.fulfill({ contentType: 'image/svg+xml', body: picture(served.colour) });
  };
  await page.route('**/api/images/*/*/backdrop*', answer);
  await page.route('**/api/images/tmdb/w1280/**', answer);
}

/** Serves a library title with (or without) a backdrop, an overview and other titles. */
async function titleWith(page: Page, kind: 'movies' | 'series', backdrop: string | null) {
  await page.route(`**/api/library/${kind}/*`, async (route) => {
    const answer = await route.fetch();
    const title = (await answer.json()) as Record<string, unknown>;
    title['backdrop'] = backdrop;
    title['has_backdrop'] = backdrop !== null;
    title['overview'] = OVERVIEW;
    title['alternative_titles'] = ['Der andere Titel'];
    await route.fulfill({ response: answer, json: title });
  });
}

async function openMovie(page: Page, backdrop: string | null = BACKDROP): Promise<void> {
  await titleWith(page, 'movies', backdrop);
  await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, 'Das Boot')}`);
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
}

async function openSeries(page: Page, backdrop: string | null = BACKDROP): Promise<void> {
  await titleWith(page, 'series', backdrop);
  await page.goto(`${LIBRARY}/serien/${await seriesId(page.request, LIBRARY, 'Kleine Insel')}`);
  await expect(page.getByRole('heading', { name: 'Staffeln' })).toBeVisible();
}

async function openPreview(page: Page): Promise<void> {
  await page.goto(`${DISCOVER}/entdecken/filme/502`);
  await settle(page);
}

const PAGES = [
  ['movie page', openMovie],
  ['series page', openSeries],
  ['preview page', openPreview],
] as const;

const backdropOf = (page: Page) => page.getByTestId('head-backdrop');
const headOf = (page: Page) => page.locator('section.head');

/** Waits until the backdrop's image has loaded. */
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

/**
 * The weakest contrast of the head's free-standing texts against what is painted behind
 * them: every text is made transparent, the page photographed, and each pixel of a text's
 * box compared with the text's colour.
 */
async function weakestContrast(page: Page): Promise<{ text: string; ratio: number }> {
  const texts = headOf(page).locator(
    '.text > h1, .text > p, .text [data-testid="rating"], .text .rating-line',
  );
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
  expect(found.length).toBeGreaterThanOrEqual(2);
  await page.addStyleTag({
    content: 'section.head .text, section.head .text * { color: transparent !important; }',
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
      // The lightest and the darkest pixel are the two that can be weakest.
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

for (const [name, open] of PAGES) {
  test.describe(`${name}: sharp backdrop (AC-05)`, () => {
    test.use({ viewport: { width: 1440, height: 900 } });

    test('fills the head sharp, cropped, anchored to the top, under a scrim', async ({ page }) => {
      await serveBackdrop(page, { colour: 'grey' });
      await open(page);
      await loaded(page);
      const backdrop = backdropOf(page);
      const img = backdrop.locator('img');
      const head = await rect(headOf(page));
      const box = await rect(backdrop);
      // Across the head's full width and height, behind its content.
      expect(Math.abs(box.width - head.width)).toBeLessThanOrEqual(1);
      expect(Math.abs(box.height - head.height)).toBeLessThanOrEqual(1);
      expect(head.height).toBeGreaterThanOrEqual(420);
      const look = await img.evaluate((el) => {
        const s = getComputedStyle(el);
        return { filter: s.filter, fit: s.objectFit, position: s.objectPosition };
      });
      expect(look.filter).toBe('none');
      expect(look.fit).toBe('cover');
      expect(look.position).toMatch(/^50% 0(%|px)$/);
      // Decorative.
      await expect(backdrop).toHaveAttribute('aria-hidden', 'true');
      await expect(img).toHaveAttribute('alt', '');
      // The scrim covers the image and ends in the page ground at the lower edge.
      const scrim = await rect(backdrop.locator('.scrim'));
      expect(Math.abs(scrim.height - box.height)).toBeLessThanOrEqual(1);
      const ground = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
      const strip = await page.screenshot({
        clip: { x: head.x + head.width - 40, y: head.y + head.height - 2, width: 20, height: 1 },
      });
      const lowest = await page.evaluate(async (data) => {
        const image = new Image();
        image.src = `data:image/png;base64,${data}`;
        await image.decode();
        const canvas = document.createElement('canvas');
        canvas.width = 1;
        canvas.height = 1;
        const context = canvas.getContext('2d');
        context?.drawImage(image, 0, 0);
        const p = context?.getImageData(0, 0, 1, 1).data ?? [0, 0, 0];
        return [p[0] ?? 0, p[1] ?? 0, p[2] ?? 0];
      }, strip.toString('base64'));
      const want = ground.match(/\d+/g)?.map(Number) ?? [];
      lowest.forEach((v, i) => expect(Math.abs(v - (want[i] ?? 0))).toBeLessThanOrEqual(6));
      // Poster, title and actions stay.
      await expect(headOf(page).getByRole('heading', { level: 1 })).toBeVisible();
    });

    test('the scrim is strong under the text and light at the top right', async ({ page }) => {
      // Over an all-white image the scrim's share of the page ground can be read from a
      // pixel: 0 is the white image, 1 the page ground.
      await serveBackdrop(page, { colour: 'white' });
      await open(page);
      await loaded(page);
      await page.addStyleTag({
        content: 'section.head .text, section.head .text * { color: transparent !important; }',
      });
      const ground = (
        (await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).match(
          /\d+/g,
        ) ?? []
      ).map(Number);
      const share = async (x: number, y: number): Promise<number> => {
        const shot = await page.screenshot({ clip: { x, y, width: 1, height: 1 } });
        const pixel = await page.evaluate(async (data) => {
          const image = new Image();
          image.src = `data:image/png;base64,${data}`;
          await image.decode();
          const canvas = document.createElement('canvas');
          canvas.width = 1;
          canvas.height = 1;
          const context = canvas.getContext('2d');
          context?.drawImage(image, 0, 0);
          return [...(context?.getImageData(0, 0, 1, 1).data ?? [0, 0, 0])].slice(0, 3);
        }, shot.toString('base64'));
        const shares = pixel.map((v, i) => (255 - v) / (255 - (ground[i] ?? 0)));
        return shares.reduce((a, b) => a + b, 0) / shares.length;
      };
      const head = await rect(headOf(page));
      const title = await rect(headOf(page).getByRole('heading', { level: 1 }));
      expect(await share(head.x + head.width - 8, head.y + 8)).toBeLessThanOrEqual(0.45);
      // Under the title: at its start and at the end of the text column's measure.
      expect(await share(title.x + 2, title.y + 2)).toBeGreaterThanOrEqual(0.8);
      expect(await share(title.x + title.width - 2, title.y + 2)).toBeGreaterThanOrEqual(0.8);
    });

    test('below 1024 px the head is as high as its content', async ({ page }) => {
      await serveBackdrop(page, { colour: 'grey' });
      await page.setViewportSize({ width: 800, height: 900 });
      await open(page);
      await loaded(page);
      const min = await headOf(page).evaluate((el) => getComputedStyle(el).minHeight);
      expect(['0px', 'auto']).toContain(min);
    });
  });

  test.describe(`${name}: contrast over any image (AC-07)`, () => {
    for (const width of [1440, 360]) {
      for (const colour of ['white', 'black'] as const) {
        test(`at least 4.5:1 over an all-${colour} backdrop at ${width} px`, async ({ page }) => {
          await page.setViewportSize({ width, height: 900 });
          await serveBackdrop(page, { colour });
          await open(page);
          await loaded(page);
          const weakest = await weakestContrast(page);
          expect(weakest.ratio, `„${weakest.text}“`).toBeGreaterThanOrEqual(4.5);
        });
      }
    }
  });

  test.describe(`${name}: the head does not wait or move (AC-08)`, () => {
    test.use({ viewport: { width: 1440, height: 900 } });

    test('text first; same height and text position when the image arrives', async ({ page }) => {
      let release: (() => void) | undefined;
      const hold = new Promise<void>((resolve) => {
        release = resolve;
      });
      await serveBackdrop(page, { colour: 'grey', hold });
      await open(page);
      const title = headOf(page).getByRole('heading', { level: 1 });
      await expect(title).toBeVisible();
      await page.waitForLoadState('domcontentloaded');
      await page.waitForTimeout(300);
      const before = { head: await rect(headOf(page)), title: await rect(title) };
      release?.();
      await loaded(page);
      const after = { head: await rect(headOf(page)), title: await rect(title) };
      expect(after).toEqual(before);
    });
  });

  test.describe(`${name}: accessibility (AC-10)`, () => {
    for (const width of [1440, 360]) {
      test(`no violation and a visible focus at ${width} px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await serveBackdrop(page, { colour: 'white' });
        await open(page);
        await loaded(page);
        const result = await new AxeBuilder({ page })
          .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
          .analyze();
        expect(
          result.violations.map(
            (v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
          ),
        ).toEqual([]);
        // Every control of the head takes the keyboard focus and shows it.
        await page.waitForLoadState('networkidle');
        const controls = headOf(page).locator('a[href], button:not([disabled])');
        const count = await controls.count();
        expect(count).toBeGreaterThan(0);
        for (let i = 0; i < count; i += 1) {
          const control = controls.nth(i);
          await control.focus();
          await page.keyboard.press('Shift+Tab');
          await page.keyboard.press('Tab');
          await expect(control).toBeFocused();
          expect(await control.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe(
            'none',
          );
        }
      });
    }
  });
}

test.describe('fallbacks (AC-06)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a library title without a backdrop shows its poster, blurred', async ({ page }) => {
    await openMovie(page, null);
    const backdrop = backdropOf(page);
    await expect(backdrop).toHaveAttribute('data-kind', 'blurred');
    expect(await backdrop.locator('img').evaluate((el) => getComputedStyle(el).filter)).toContain(
      'blur',
    );
    await expect(backdrop.locator('img')).toHaveAttribute(
      'src',
      /\/api\/images\/movie\/\d+\/detail/,
    );
  });

  test('a backdrop that fails to load falls back to the blurred poster', async ({ page }) => {
    await serveBackdrop(page, { colour: 'grey', fail: true });
    await openMovie(page);
    await expect(backdropOf(page)).toHaveAttribute('data-kind', 'blurred');
  });

  test('without backdrop and poster the head has the plain ground', async ({ page }) => {
    await page.route('**/api/library/movies/*', async (route) => {
      const answer = await route.fetch();
      const title = (await answer.json()) as Record<string, unknown>;
      title['backdrop'] = null;
      title['has_backdrop'] = false;
      title['has_poster'] = false;
      await route.fulfill({ response: answer, json: title });
    });
    await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, 'Das Boot')}`);
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
    await expect(backdropOf(page)).toHaveCount(0);
  });

  test('a preview whose backdrop fails to load has the plain ground', async ({ page }) => {
    await serveBackdrop(page, { colour: 'grey', fail: true });
    await openPreview(page);
    await expect(page.getByTestId('preview')).toBeVisible();
    await expect(backdropOf(page)).toHaveCount(0);
  });
});
