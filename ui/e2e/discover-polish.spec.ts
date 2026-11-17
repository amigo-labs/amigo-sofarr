// Spec ui-discover-polish AC-01 .. AC-06: the discover start page's search row, filter
// panel and chips, the shelf arrows, the preview head with its backdrop.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { DISCOVER, settle } from './discover-helpers';
import { PALETTES, storeTheme, token } from './theme-helpers';

const filterButton = (page: Page) => page.getByRole('button', { name: /^Filter( \(\d+\))?$/ });
const chips = (page: Page) => page.getByRole('list', { name: 'Aktive Filter' }).getByRole('button');

async function openFilters(page: Page): Promise<void> {
  if ((await filterButton(page).getAttribute('aria-expanded')) !== 'true') {
    await filterButton(page).click();
  }
}

test.describe('search row and filter panel (AC-01, AC-02)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('search, Suchen and the filter button in one row; the panel closed at load', async ({
    page,
  }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const search = await page.getByLabel('Suchbegriff').boundingBox();
    const go = await page.getByRole('button', { name: 'Suchen' }).boundingBox();
    const filter = await filterButton(page).boundingBox();
    for (const b of [go, filter]) {
      expect(Math.abs((b?.y ?? 0) - (search?.y ?? 0))).toBeLessThan(8);
    }
    await expect(filterButton(page)).toHaveText('Filter');
    await expect(filterButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(page.getByLabel('Art')).toBeHidden();
    await openFilters(page);
    await expect(filterButton(page)).toHaveAttribute('aria-expanded', 'true');
    await expect(page.getByLabel('Art')).toBeVisible();
    await page.getByLabel('Art').selectOption({ label: 'Filme' });
    await expect(page).toHaveURL(/art=filme/);
    await expect(filterButton(page)).toHaveText('Filter (1)');
    await page.reload();
    await settle(page);
    await expect(filterButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(chips(page)).toHaveText(['Art: Filme']);
  });

  test('chips name the filters; pressing one removes it and moves the focus', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken?art=filme&von=1990&bis=2000&sprache=da`);
    await settle(page);
    await expect(chips(page)).toHaveText([
      'Art: Filme',
      'Jahr von: 1990',
      'Jahr bis: 2000',
      'Originalsprache: Dänisch',
    ]);
    await expect(filterButton(page)).toHaveText('Filter (4)');
    await chips(page).nth(1).click();
    await expect(page).not.toHaveURL(/von=/);
    await expect(page).toHaveURL(/bis=2000/);
    await expect(chips(page)).toHaveText([
      'Art: Filme',
      'Jahr bis: 2000',
      'Originalsprache: Dänisch',
    ]);
    await expect(chips(page).nth(1)).toBeFocused();
  });
});

test.describe('shelf arrows (AC-03)', () => {
  test.use({ viewport: { width: 1024, height: 800 } });

  test('back and forward scroll the shelf; disabled at its ends', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const shelf = page.getByTestId('shelf').first();
    const name = (await shelf.getByRole('heading').textContent())?.trim() ?? '';
    const back = shelf.getByRole('button', { name: `Zurück in „${name}“` });
    const forward = shelf.getByRole('button', { name: `Weiter in „${name}“` });
    await expect(back).toBeDisabled();
    const strip = shelf.locator('ul.tiles');
    const wide = await strip.evaluate((el) => el.scrollWidth > el.clientWidth + 4);
    if (!wide) {
      await expect(forward).toBeDisabled();
      return;
    }
    await expect(forward).toBeEnabled();
    await forward.click();
    await expect.poll(() => strip.evaluate((el) => el.scrollLeft)).toBeGreaterThan(100);
    await expect(back).toBeEnabled();
    for (let i = 0; i < 20 && (await forward.isEnabled()); i += 1) {
      await forward.click();
      await page.waitForTimeout(400);
    }
    await expect(forward).toBeDisabled();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  });
});

test.describe('preview head (AC-04)', () => {
  test('the backdrop blurred under at least 70 % of the page ground; compact facts', async ({
    page,
  }) => {
    await page.goto(`${DISCOVER}/entdecken/filme/502`);
    await settle(page);
    const backdrop = page.locator('[data-testid="preview"] .backdrop img');
    await expect(backdrop).toBeAttached();
    expect(await backdrop.evaluate((el) => getComputedStyle(el).filter)).toMatch(/blur/);
    const veil = await page.locator('[data-testid="preview"] .backdrop-veil').evaluate((el) => ({
      colour: getComputedStyle(el).backgroundColor,
      opacity: Number(getComputedStyle(el).opacity),
    }));
    expect(veil.colour).toBe(token('dark', 'bg'));
    expect(veil.opacity).toBeGreaterThanOrEqual(0.7);
    await expect(page.locator('[data-testid="preview"] dl.facts-compact')).toBeVisible();
  });

  test('no backdrop when it fails to load', async ({ page }) => {
    await page.route('**/api/images/**/backdrop*', (route) => route.fulfill({ status: 404 }));
    await page.route(/\/b\d+\.png/, (route) => route.fulfill({ status: 404 }));
    await page.goto(`${DISCOVER}/entdecken/filme/502`);
    await settle(page);
    await expect(page.locator('[data-testid="preview"] .backdrop img')).toHaveCount(0);
  });
});

test.describe('narrow window (AC-05)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test('the search takes the full width, two filter columns, arrows visible', async ({ page }) => {
    await page.goto(`${DISCOVER}/entdecken`);
    await settle(page);
    const search = await page.getByLabel('Suchbegriff').boundingBox();
    const main = await page.locator('main').boundingBox();
    expect(search?.width ?? 0).toBeGreaterThan((main?.width ?? 0) - 40);
    await openFilters(page);
    const columns = await page
      .getByRole('form', { name: 'Filter' })
      .locator('label')
      .evaluateAll(
        (all) => new Set(all.map((el) => Math.round(el.getBoundingClientRect().x))).size,
      );
    expect(columns).toBeLessThanOrEqual(2);
    const shelf = page.getByTestId('shelf').first();
    await expect(shelf.getByRole('button', { name: /^Weiter in/ })).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  });
});

async function expectNoViolations(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  expect(
    result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`),
  ).toEqual([]);
}

for (const palette of PALETTES) {
  test(`axe: start page with panel and chips, preview with backdrop (AC-06, ${palette})`, async ({
    page,
  }) => {
    await storeTheme(page, palette);
    for (const width of [1440, 360]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`${DISCOVER}/entdecken?art=filme&sprache=da`);
      await settle(page);
      await openFilters(page);
      await expectNoViolations(page);
      await page.goto(`${DISCOVER}/entdecken/filme/502`);
      await settle(page);
      await expectNoViolations(page);
    }
  });
}
