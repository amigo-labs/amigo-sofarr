// Spec ui-consistency: the accent only for states, the place and the main action; discover
// like the library; German wording and date-times to the minute; calmer episode rows; paths
// that break at their separators; sofarr's monogram.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { DISCOVER, cards, settle } from './discover-helpers';
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

test.describe('discover tiles (AC-03, AC-04)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('a tile shows the compact rating with the full text as tooltip and name', async ({
    page,
  }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const trending = page.getByTestId('shelf').first();
    const star = cards(trending, 'movie/502').getByTestId('rating');
    await expect(star).toHaveText('★ 7,8');
    await expect(star).toHaveAccessibleName('IMDb 7,8 (12.345 Stimmen)');
    await expect(star).toHaveAttribute('title', 'IMDb 7,8 (12.345 Stimmen)');
    const unrated = cards(page, 'movie/505').first().getByTestId('rating');
    await expect(unrated).toHaveText('–');
    await expect(unrated).toHaveAccessibleName('IMDb –');
    // One line, also at 360 px.
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 900 });
      const box = await star.boundingBox();
      const lineHeight = await star.evaluate((e) => parseFloat(getComputedStyle(e).lineHeight));
      expect(box?.height ?? 99).toBeLessThan(lineHeight * 1.5);
    }
  });

  test('the library state chip and the add button have the same height', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const shelf = page.getByTestId('shelf').first();
    const chip = shelf.locator('[data-testid="library-state"] > *').first();
    const add = shelf.getByRole('button', { name: /^Hinzufügen/ }).first();
    await expect(chip).toBeVisible();
    await expect(add).toBeVisible();
    const a = await chip.boundingBox();
    const b = await add.boundingBox();
    expect(Math.abs((a?.height ?? 0) - (b?.height ?? 99))).toBeLessThan(1);
  });
});

test.describe('discover search (AC-05, AC-06)', () => {
  test('a magnifier sits at the start of the search field', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const field = await page.getByLabel('Suchbegriff').boundingBox();
    const icons = await page
      .getByRole('search')
      .locator('svg')
      .evaluateAll((all) => all.map((e) => e.getBoundingClientRect().toJSON() as DOMRect));
    const inside = icons.filter(
      (r) =>
        r.x >= (field?.x ?? 0) &&
        r.x + r.width <= (field?.x ?? 0) + (field?.width ?? 0) / 4 &&
        r.y >= (field?.y ?? 0) &&
        r.y + r.height <= (field?.y ?? 0) + (field?.height ?? 0),
    );
    expect(inside.length).toBe(1);
    await page.getByLabel('Suchbegriff').fill('Sternen');
    await page.keyboard.press('Enter');
    await expect(page).toHaveURL(/\/entdecken\/suche\?begriff=Sternen/);
  });

  for (const width of [360, 390]) {
    test(`at ${width} px the field takes the width, „Suchen“ and „Filter“ share the next line`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 800 });
      await page.goto(`${DISCOVER}/entdecken`);
      await settle(page);
      const field = await page.getByLabel('Suchbegriff').boundingBox();
      const main = await page.locator('main').boundingBox();
      const go = await page.getByRole('button', { name: 'Suchen' }).boundingBox();
      const filter = await page.getByRole('button', { name: /^Filter/ }).boundingBox();
      expect(field?.width ?? 0).toBeGreaterThan((main?.width ?? 0) - 40);
      expect(Math.abs((go?.y ?? 0) - (filter?.y ?? 99))).toBeLessThan(4);
      expect(go?.y ?? 0).toBeGreaterThan((field?.y ?? 0) + (field?.height ?? 0) - 1);
      expect(
        await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
      ).toBe(true);
    });
  }
});

test.describe('wording and date-times (AC-07 to AC-10)', () => {
  test('„Sperrliste“ in navigation, heading and page title; the address stays', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`${LIBRARY}/`);
    await page.getByRole('link', { name: 'Sperrliste' }).click();
    await expect(page).toHaveURL(`${LIBRARY}/blocklist`);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Sperrliste');
    await expect(page).toHaveTitle('Sperrliste – sofarr');
    await expect(page.getByText('Blocklist')).toHaveCount(0);
  });

  test('discover says „Heute beliebt“ and „Diese Woche beliebt“', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    await expect(page.getByRole('heading', { name: 'Heute beliebt' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Diese Woche beliebt' })).toBeVisible();
    await expect(page.getByText(/Trending/)).toHaveCount(0);
    await page.getByRole('link', { name: /^Mehr aus „Heute beliebt“/ }).click();
    await expect(page).toHaveURL(/\/entdecken\/regal\/trending-heute$/);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Heute beliebt');
  });

  test('the series page: type in German, episode counts on two lines, the note without syntax', async ({
    page,
  }) => {
    await openSeries(page);
    const facts = page.locator('dl').filter({ hasText: 'Serientyp' }).first();
    await expect(facts.locator('div').filter({ hasText: 'Serientyp' }).locator('dd')).toHaveText(
      'Standard',
    );
    const counts = page.getByTestId('episode-counts');
    await expect(counts.locator('> *')).toHaveText(['1 von 5 mit Datei', '3 überwacht']);
    const [first, second] = await counts.locator('> *').all();
    expect((await second?.boundingBox())?.y ?? 0).toBeGreaterThan(
      ((await first?.boundingBox())?.y ?? 0) + 4,
    );
    const muted = await resolved(page, 'color', 'var(--fgColor-muted)');
    expect(second ? await colour(second) : '').toBe(muted);
    await expect(page.locator('.flash-info')).toHaveText(
      'Dateiaktionen (Umbenennen, Ordner verschieben, Dateien löschen) gibt es nur im ' +
        'Live-Modus. Überwachung, Profil und Entfernen ohne Dateien funktionieren trotzdem.',
    );
  });

  test('the discover „Stand“ line is a caption to the minute', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const fetched = page.getByTestId('fetched').first();
    await expect(fetched).toHaveText(/^Stand: \d{2}\.\d{2}\.\d{4}, \d{2}:\d{2}$/);
    expect(await colour(fetched)).toBe(await resolved(page, 'color', 'var(--fgColor-muted)'));
    const size = await page.evaluate(() => {
      const probe = document.createElement('span');
      probe.style.fontSize = 'var(--text-body-size-small)';
      document.body.append(probe);
      const out = getComputedStyle(probe).fontSize;
      probe.remove();
      return out;
    });
    expect(await fetched.evaluate((e) => getComputedStyle(e).fontSize)).toBe(size);
  });
});
