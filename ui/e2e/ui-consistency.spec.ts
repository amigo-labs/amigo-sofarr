// Spec ui-consistency: the accent only for states, the place and the main action; discover
// like the library; German wording and date-times to the minute; calmer episode rows; paths
// that break at their separators; sofarr's monogram.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { LIBRARY, seriesId } from './library-helpers';
import { PALETTES, storeTheme } from './theme-helpers';

/** The computed colour of a CSS colour expression in the page, e.g. `var(--fgColor-accent)`. */
async function resolved(page: Page, property: 'color' | 'background-color', value: string) {
  return page.evaluate(
    ([prop, val]) => {
      const probe = document.createElement('span');
      probe.style.setProperty(prop, val);
      document.body.append(probe);
      const out = getComputedStyle(probe).getPropertyValue(prop);
      probe.remove();
      return out;
    },
    [property, value] as const,
  );
}

const colour = (l: Locator) => l.evaluate((e) => getComputedStyle(e).color);
const decoration = (l: Locator) => l.evaluate((e) => getComputedStyle(e).textDecorationLine);

async function openSeries(page: Page, title = 'Kleine Insel'): Promise<void> {
  await page.goto(`${LIBRARY}/serien/${await seriesId(page.request, LIBRARY, title)}`);
  await expect(page.getByRole('heading', { name: 'Staffeln' })).toBeVisible();
}

for (const palette of PALETTES) {
  test.describe(`title links and notices (AC-01, AC-02), ${palette}`, () => {
    test.beforeEach(async ({ page }) => storeTheme(page, palette));

    test('a title link rests in the text colour and turns to the accent on hover', async ({
      page,
    }) => {
      await page.goto(`${LIBRARY}/filme?suche=Film%200001`);
      const link = page.getByRole('link', { name: 'Film 0001', exact: true }).first();
      await expect(link).toBeVisible();
      const text = await resolved(page, 'color', 'var(--fgColor-default)');
      const accent = await resolved(page, 'color', 'var(--fgColor-accent)');
      expect(await colour(link)).toBe(text);
      expect(await decoration(link)).toBe('none');
      await link.hover();
      await expect.poll(() => colour(link)).toBe(accent);
      expect(await decoration(link)).toBe('underline');
    });

    test('the active navigation entry keeps the accent', async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.goto(`${LIBRARY}/filme`);
      const active = page.locator('nav a[aria-current="page"]').first();
      const accent = await resolved(page, 'color', 'var(--fgColor-accent)');
      expect(await colour(active)).toBe(accent);
    });

    test('an info notice has the neutral muted ground and the text colour', async ({ page }) => {
      await openSeries(page);
      const note = page.locator('.flash-info').filter({ hasText: 'Live-Modus' });
      await expect(note).toBeVisible();
      const ground = await resolved(page, 'background-color', 'var(--bgColor-muted)');
      const text = await resolved(page, 'color', 'var(--fgColor-default)');
      expect(await note.evaluate((e) => getComputedStyle(e).backgroundColor)).toBe(ground);
      expect(await colour(note)).toBe(text);
    });
  });
}

test.describe('episode disclosures (AC-11)', () => {
  test('„Datei“ and „Untertitel“ share one muted line; both open in the full row width', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await openSeries(page);
    const file = page.getByRole('button', { name: /^Datei von Kleine Insel/ }).first();
    const subs = page.getByRole('button', { name: /^Untertitel von Kleine Insel/ }).first();
    await expect(file).toBeVisible();
    const muted = await resolved(page, 'color', 'var(--fgColor-muted)');
    expect(await colour(file)).toBe(muted);
    expect(await colour(subs)).toBe(muted);
    const a = await file.boundingBox();
    const b = await subs.boundingBox();
    expect(Math.abs((a?.y ?? 0) - (b?.y ?? 99))).toBeLessThan(4);
    expect(b?.x ?? 0).toBeGreaterThan((a?.x ?? 0) + (a?.width ?? 0));

    const row = page.locator('li.episode').filter({ has: file });
    const rowBox = await row.boundingBox();
    await expect(file).toHaveAttribute('aria-expanded', 'false');
    await file.click();
    await subs.click();
    await expect(file).toHaveAttribute('aria-expanded', 'true');
    await expect(subs).toHaveAttribute('aria-expanded', 'true');
    const bodies = row.locator('.disclosure-body');
    await expect(bodies).toHaveCount(2);
    for (const body of await bodies.all()) {
      const box = await body.boundingBox();
      expect(box?.width ?? 0).toBeGreaterThan((rowBox?.width ?? 0) * 0.8);
    }
  });
});
