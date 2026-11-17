// Spec ui-list-items AC-01 .. AC-08: the cards and rows of the movie and series lists:
// status chips, the monitor icon on the poster, the checkbox on hover, focus and in
// selection mode, dimmed unmonitored posters.

import { devices, expect, test, type Locator, type Page } from '@playwright/test';
import { LIBRARY, items } from './library-helpers';
import { PALETTES, storeTheme, token } from './theme-helpers';

async function showAs(page: Page, view: 'liste' | 'poster'): Promise<void> {
  await page.addInitScript(
    (value: string) => localStorage.setItem('sofarr-ansicht-filme', value),
    view,
  );
}

const item = (page: Page, title: string) =>
  items(page).filter({ has: page.getByRole('link', { name: title, exact: true }) });
const checkbox = (page: Page, title: string) => page.getByLabel(`„${title}“ auswählen`);
const toggle = (page: Page, title: string) =>
  page.getByRole('button', { name: `Überwachen „${title}“` });
const opacity = (locator: Locator) =>
  locator.evaluate((el) => Number(getComputedStyle(el.closest('.select') ?? el).opacity));
const chipTexts = (row: Locator) => row.locator('.label').allTextContents();

/** The red, green and blue of a computed colour (`rgb(…)` or `color(srgb …)`), 0 to 255. */
function channels(text: string): number[] {
  const nums = (text.match(/[\d.]+/g) ?? []).map(Number);
  return text.startsWith('color(') ? nums.slice(0, 3).map((n) => n * 255) : nums.slice(0, 3);
}

function luminance(rgb: number[]): number {
  const [r = 0, g = 0, b = 0] = rgb.map((c) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The WCAG contrast ratio of two colours. */
function contrast(a: string, b: string): number {
  const [x, y] = [luminance(channels(a)), luminance(channels(b))];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

test.describe('cards (AC-01, AC-04)', () => {
  test.beforeEach(async ({ page }) => {
    await showAs(page, 'poster');
  });

  test('a card shows the quality chip in accent, "fehlt" once, labels after it', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/filme?suche=Das%20Boot`);
    const boot = item(page, 'Das Boot');
    await expect(boot).toContainText('1981');
    expect(await chipTexts(boot)).toEqual(['Bluray-1080p', 'Cutoff nicht erreicht']);
    await expect(boot.locator('.label').first()).toHaveClass(/label-accent/);
    await page.goto(`${LIBRARY}/filme?suche=Fehlt%20Sehr`);
    const missing = item(page, 'Fehlt Sehr');
    await expect(missing.locator('.label').first()).toHaveText('fehlt');
    await expect(missing.locator('.label').first()).toHaveClass(/label-danger/);
    expect(await missing.getByText('fehlt', { exact: true }).count()).toBe(1);
  });

  test('a series card shows its files as a neutral chip first', async ({ page }) => {
    await page.addInitScript(() => localStorage.setItem('sofarr-ansicht-serien', 'poster'));
    await page.goto(`${LIBRARY}/serien?suche=Kleine%20Insel`);
    const insel = item(page, 'Kleine Insel');
    await expect(insel).toBeVisible();
    expect(await chipTexts(insel)).toEqual(['1 von 5 Dateien', '1 fehlen', '1 unter Cutoff']);
  });

  test('the monitor toggle is an icon in the top right corner of the poster', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme?suche=Das%20Boot`);
    const button = toggle(page, 'Das Boot');
    await expect(button).toHaveAttribute('aria-pressed', 'true');
    const poster = await item(page, 'Das Boot').locator('.poster-wrap').boundingBox();
    const placed = await button.boundingBox();
    if (poster === null || placed === null) throw new Error('not shown');
    expect(placed.width).toBeLessThanOrEqual(40);
    expect(placed.x + placed.width).toBeLessThanOrEqual(poster.x + poster.width + 1);
    expect(placed.x).toBeGreaterThan(poster.x + poster.width / 2);
    expect(placed.y).toBeLessThan(poster.y + poster.height / 4);
    expect(placed.y).toBeGreaterThanOrEqual(poster.y - 1);
  });

  for (const palette of PALETTES) {
    test(`the toggle's icon has 3:1 against its own ground (AC-04, ${palette})`, async ({
      page,
    }) => {
      await storeTheme(page, palette);
      await page.goto(`${LIBRARY}/filme?suche=Film%20000`);
      for (const title of ['Das Boot', 'Film 0001']) {
        if ((await toggle(page, title).count()) === 0) continue;
        const colours = await toggle(page, title).evaluate((el) => ({
          fg: getComputedStyle(el).color,
          bg: getComputedStyle(el).backgroundColor,
        }));
        const ratio = contrast(colours.fg, colours.bg);
        expect(ratio, title).toBeGreaterThanOrEqual(3);
      }
    });
  }
});

test.describe('rows (AC-02)', () => {
  test.use({ viewport: { width: 1280, height: 800 } });

  test('a row is at most 64 px high; rating and toggle at its end', async ({ page }) => {
    await showAs(page, 'liste');
    await page.goto(`${LIBRARY}/filme?suche=Das%20Boot`);
    const row = item(page, 'Das Boot');
    await expect(row).toBeVisible();
    const box = await row.boundingBox();
    expect(box?.height ?? 99).toBeLessThanOrEqual(64);
    const title = await row.getByRole('link', { name: 'Das Boot' }).boundingBox();
    const button = await toggle(page, 'Das Boot').boundingBox();
    expect(button?.x ?? 0).toBeGreaterThan((box?.x ?? 0) + (box?.width ?? 0) * 0.75);
    expect(button?.x ?? 0).toBeGreaterThan((title?.x ?? 0) + (title?.width ?? 0));
    expect(await chipTexts(row)).toEqual(['Bluray-1080p', 'Cutoff nicht erreicht']);
  });

  test('a selected row has the accent-subtle ground', async ({ page }) => {
    await showAs(page, 'liste');
    await page.goto(`${LIBRARY}/filme?suche=Das%20Boot`);
    await checkbox(page, 'Das Boot').check();
    const row = item(page, 'Das Boot');
    expect(await row.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
      token('dark', 'accent-subtle'),
    );
  });
});

test.describe('selection checkbox on a hover device (AC-05, AC-06)', () => {
  test.beforeEach(async ({ page }) => {
    await showAs(page, 'poster');
  });

  test('shows on hover and focus only, stays in the Tab order', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme?suche=Film%20000`);
    const box = checkbox(page, 'Film 0001');
    await expect(box).toBeAttached();
    await page.mouse.move(1, 1);
    expect(await opacity(box)).toBe(0);
    await item(page, 'Film 0001').hover();
    await expect.poll(() => opacity(box)).toBe(1);
    await page.mouse.move(1, 1);
    await expect.poll(() => opacity(box)).toBe(0);
    // Keyboard: the title link, then the checkbox.
    await item(page, 'Film 0001').getByRole('link').focus();
    await page.keyboard.press('Tab');
    await expect(box).toBeFocused();
    await expect.poll(() => opacity(box)).toBe(1);
  });

  test('in selection mode every checkbox shows; a selected card is outlined', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme?suche=Film%20000`);
    await item(page, 'Film 0001').hover();
    await checkbox(page, 'Film 0001').check();
    await page.mouse.move(1, 1);
    await expect.poll(() => opacity(checkbox(page, 'Film 0002'))).toBe(1);
    const outline = await item(page, 'Film 0001')
      .locator('.poster-wrap')
      .evaluate((el) => ({
        style: getComputedStyle(el).outlineStyle,
        color: getComputedStyle(el).outlineColor,
      }));
    expect(outline).toEqual({ style: 'solid', color: token('dark', 'accent') });
    // Deselecting the last one hides them again, except where the focus is.
    await checkbox(page, 'Film 0001').uncheck();
    await page.mouse.move(1, 1);
    await expect.poll(() => opacity(checkbox(page, 'Film 0002'))).toBe(0);
    await expect.poll(() => opacity(checkbox(page, 'Film 0001'))).toBe(1);
  });
});

test.describe('touch device (AC-05, AC-08)', () => {
  const { defaultBrowserType: _browser, ...pixel } = devices['Pixel 7'];
  test.use(pixel);

  test('checkboxes always show; targets are at least 32 px; nothing scrolls sideways', async ({
    page,
  }) => {
    await showAs(page, 'poster');
    await page.goto(`${LIBRARY}/filme?suche=Film%20000`);
    const box = checkbox(page, 'Film 0001');
    await expect(box).toBeAttached();
    expect(await opacity(box)).toBe(1);
    const target = await item(page, 'Film 0001').locator('.select').boundingBox();
    const button = await toggle(page, 'Film 0001').boundingBox();
    expect(target?.width ?? 0).toBeGreaterThanOrEqual(32);
    expect(target?.height ?? 0).toBeGreaterThanOrEqual(32);
    expect(button?.width ?? 0).toBeGreaterThanOrEqual(32);
    expect(button?.height ?? 0).toBeGreaterThanOrEqual(32);
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
  });
});

test.describe('monitored or not (AC-07)', () => {
  test('an unmonitored poster is dimmed, its controls are not', async ({ page }) => {
    await showAs(page, 'poster');
    await page.goto(`${LIBRARY}/filme?suche=Film%20000`);
    const film = item(page, 'Film 0001');
    await expect(toggle(page, 'Film 0001')).toHaveAttribute('aria-pressed', 'false');
    const dimmed = await film
      .locator('.poster-wrap')
      .evaluate((el) => Number(getComputedStyle(el).opacity));
    expect(dimmed).toBeLessThan(0.8);
    const control = await toggle(page, 'Film 0001').evaluate((el) => {
      let product = 1;
      for (let node: Element | null = el; node !== null; node = node.parentElement) {
        product *= Number(getComputedStyle(node).opacity);
      }
      return product;
    });
    expect(control).toBe(1);
    await page.goto(`${LIBRARY}/filme?suche=Das%20Boot`);
    const boot = await item(page, 'Das Boot')
      .locator('.poster-wrap')
      .evaluate((el) => Number(getComputedStyle(el).opacity));
    expect(boot).toBe(1);
  });
});
