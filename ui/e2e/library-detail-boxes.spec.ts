// Spec ui-detail-boxes AC-01 .. AC-07: the boxes of the movie and series pages in two
// columns, quiet box headers, file paths that wrap, compact season headers and episode rows.

import { expect, test, type Page } from '@playwright/test';
import { LIBRARY, movieId, seriesId } from './library-helpers';
import { token } from './theme-helpers';

const box = (page: Page, name: string) =>
  page.locator('main section.box').filter({
    has: page.getByRole('heading', { name, exact: true }),
  });

async function rect(page: Page, name: string) {
  const found = await box(page, name).first().boundingBox();
  if (found === null) throw new Error(`no box ${name}`);
  return found;
}

async function openMovie(page: Page, title = 'Das Boot'): Promise<void> {
  await page.goto(`${LIBRARY}/filme/${await movieId(page.request, LIBRARY, title)}`);
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Grabs', exact: true })).toBeVisible();
}

async function openSeries(page: Page, title = 'Kleine Insel'): Promise<void> {
  await page.goto(`${LIBRARY}/serien/${await seriesId(page.request, LIBRARY, title)}`);
  await expect(page.getByRole('heading', { name: 'Staffeln' })).toBeVisible();
}

const MAIN = ['Datei', 'Untertitel', 'Grabs', 'Blocklist'];
const SIDE = ['Einstellungen', 'Ordner und Dateiaktionen'];

test.describe('two columns (AC-01)', () => {
  test('from 1200 px: main boxes left, side boxes right, each in its order', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await openMovie(page);
    const main = await Promise.all(MAIN.map((n) => rect(page, n)));
    const side = await Promise.all(SIDE.map((n) => rect(page, n)));
    for (const r of main) expect(Math.round(r.x)).toBe(Math.round(main[0]?.x ?? 0));
    for (const r of side) expect(Math.round(r.x)).toBe(Math.round(side[0]?.x ?? 0));
    expect(side[0]?.x ?? 0).toBeGreaterThan((main[0]?.x ?? 0) + (main[0]?.width ?? 0));
    expect(main[0]?.width ?? 0).toBeGreaterThan((side[0]?.width ?? 0) * 1.5);
    for (const column of [main, side]) {
      for (let i = 1; i < column.length; i += 1) {
        expect(column[i]?.y ?? 0).toBeGreaterThan(column[i - 1]?.y ?? 0);
      }
    }
  });

  test('below 1200 px: one column, main boxes first', async ({ page }) => {
    await page.setViewportSize({ width: 1100, height: 900 });
    await openMovie(page);
    const all = await Promise.all([...MAIN, ...SIDE].map((n) => rect(page, n)));
    for (const r of all) expect(Math.round(r.x)).toBe(Math.round(all[0]?.x ?? 0));
    for (let i = 1; i < all.length; i += 1) {
      expect(all[i]?.y ?? 0).toBeGreaterThan(all[i - 1]?.y ?? 0);
    }
  });

  test('a series page: seasons in the main column, settings at the side', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await openSeries(page);
    const seasons = await page.locator('section.seasons').boundingBox();
    const settings = await rect(page, 'Einstellungen');
    expect(settings.x).toBeGreaterThan((seasons?.x ?? 0) + (seasons?.width ?? 0));
    expect(settings.y).toBeLessThan((seasons?.y ?? 0) + (seasons?.height ?? 0));
  });
});

test.describe('box headers (AC-02)', () => {
  test('16 px semibold heading on the box ground, a hairline below', async ({ page }) => {
    await openMovie(page);
    for (const name of [...MAIN, ...SIDE]) {
      const look = await box(page, name)
        .first()
        .evaluate((section) => {
          const header = section.querySelector('.box-header');
          const heading = header?.matches('h2, h3') ? header : header?.querySelector('h2, h3');
          if (
            header === null ||
            header === undefined ||
            heading === null ||
            heading === undefined
          ) {
            return null;
          }
          return {
            size: getComputedStyle(heading).fontSize,
            weight: getComputedStyle(heading).fontWeight,
            ground: getComputedStyle(header).backgroundColor,
            line: getComputedStyle(header).borderBottomWidth,
          };
        });
      expect(look, name).toEqual({
        size: '16px',
        weight: '600',
        ground: 'rgba(0, 0, 0, 0)',
        line: '1px',
      });
    }
    expect(await box(page, 'Datei').evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
      token('dark', 'surface'),
    );
  });
});

test.describe('file path (AC-03)', () => {
  test('monospace, the full path as tooltip, wraps inside its box', async ({ page }) => {
    const long = `${'Ein-sehr-langer-Dateiname-ohne-Pausen'.repeat(8)}.mkv`;
    await page.route('**/api/library/movies/*', async (route) => {
      const answer = await route.fetch();
      const movie = (await answer.json()) as { file: { relative_path: string } | null };
      if (movie.file !== null) movie.file.relative_path = long;
      await route.fulfill({ response: answer, json: movie });
    });
    await page.setViewportSize({ width: 1440, height: 900 });
    await openMovie(page);
    const path = page.getByTestId('file-path');
    await expect(path).toHaveText(long);
    await expect(path).toHaveAttribute('title', long);
    expect(await path.evaluate((el) => getComputedStyle(el).fontFamily)).toMatch(/mono/i);
    const inner = await path.boundingBox();
    const outer = await rect(page, 'Datei');
    expect((inner?.x ?? 0) + (inner?.width ?? 0)).toBeLessThanOrEqual(outer.x + outer.width);
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
  });
});

test.describe('seasons (AC-04, AC-05)', () => {
  test('a season header: name, progress with numbers, labels, icon buttons at its end', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await openSeries(page);
    const season = page.locator('details.season').first();
    const header = season.locator(':scope > summary');
    await expect(header).toContainText('Staffel 1');
    const counts = await season.getByTestId('season-counts').textContent();
    const [withFile = '', , total = ''] = (counts ?? '').match(/\d+/g) ?? [];
    await expect(season.locator('.progress-text')).toHaveText(`${withFile} von ${total}`);
    await expect(season.getByRole('progressbar')).toHaveAttribute('aria-valuenow', withFile);
    await expect(season.getByRole('progressbar')).toHaveAttribute('aria-valuemax', total);
    const toggle = page.getByRole('button', { name: 'Überwachen „Kleine Insel – Staffel 1“' });
    const search = page.getByRole('button', { name: 'Staffel 1 suchen' });
    const line = await header.boundingBox();
    const name = await header.locator('.season-name').boundingBox();
    for (const button of [toggle, search]) {
      const at = await button.boundingBox();
      expect(at?.width ?? 99).toBeLessThanOrEqual(40);
      expect(
        Math.abs((at?.y ?? 0) + (at?.height ?? 0) / 2 - ((name?.y ?? 0) + (name?.height ?? 0) / 2)),
      ).toBeLessThan(12);
      expect(at?.x ?? 0).toBeGreaterThan((line?.x ?? 0) + (line?.width ?? 0) / 2);
    }
  });

  test('the header opens and closes the season; its buttons do not; a chevron shows it', async ({
    page,
  }) => {
    await openSeries(page);
    const season = page.locator('details.season').first();
    await expect(season).toHaveAttribute('open', '');
    const chevron = season.locator(':scope > summary .chevron');
    const openTurn = await chevron.evaluate((el) => getComputedStyle(el).transform);
    await season.locator(':scope > summary .season-name').click();
    await expect(season).not.toHaveAttribute('open', '');
    // The chevron turns with a short transition.
    await expect
      .poll(() => chevron.evaluate((el) => getComputedStyle(el).transform))
      .not.toBe(openTurn);
    await season.locator(':scope > summary').focus();
    await page.keyboard.press('Enter');
    await expect(season).toHaveAttribute('open', '');
    await page.getByRole('button', { name: 'Staffel 1 suchen' }).click();
    await expect(season).toHaveAttribute('open', '');
  });
});

test.describe('episode rows (AC-06)', () => {
  test('one line: code, title, date, status chip, labels and icon buttons', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await openSeries(page);
    const rows = page.locator('[data-episode]');
    await expect(rows.first()).toBeVisible();
    const chips = await rows.evaluateAll((all) =>
      all.map((row) => {
        const first = row.querySelector('.line .label');
        return {
          text: first?.textContent?.trim() ?? '',
          tone: first?.className.match(/label-(accent|danger)/)?.[1] ?? 'neutral',
          missing: [...row.querySelectorAll('.line .label')].filter(
            (l) => l.textContent?.trim() === 'fehlt',
          ).length,
        };
      }),
    );
    for (const chip of chips) {
      if (chip.text === 'fehlt') expect(chip.tone).toBe('danger');
      else if (chip.text === 'keine Datei') expect(chip.tone).toBe('neutral');
      else expect(chip.tone).toBe('accent');
      expect(chip.missing).toBeLessThanOrEqual(1);
    }
    expect(chips.some((c) => c.text === 'fehlt')).toBe(true);
    const first = rows.first();
    const line = first.locator('.line');
    const parts = await Promise.all(
      ['.code', '.episode-title', '.air', '.label', 'button'].map((s) =>
        line.locator(s).first().boundingBox(),
      ),
    );
    const mids = parts.map((p) => (p?.y ?? 0) + (p?.height ?? 0) / 2);
    expect(Math.max(...mids) - Math.min(...mids)).toBeLessThan(14);
  });
});

test.describe('narrow window (AC-07)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test('boxes, seasons and episodes fit; icon buttons are 32 px', async ({ page }) => {
    await openSeries(page);
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
    const sizes = await page
      .locator('details.season button, [data-episode] .line button')
      .evaluateAll((all) =>
        all.map((el) => [el.getBoundingClientRect().width, el.getBoundingClientRect().height]),
      );
    expect(sizes.length).toBeGreaterThan(0);
    for (const [w = 0, h = 0] of sizes) {
      expect(w).toBeGreaterThanOrEqual(32);
      expect(h).toBeGreaterThanOrEqual(32);
    }
    await openMovie(page);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  });
});
