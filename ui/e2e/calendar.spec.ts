// Spec ui-calendar AC-01 .. AC-04, AC-06: the month as a grid of weeks, the week as seven
// columns, the agenda on narrow windows, compact entries with a state dot.

import { expect, test, type Page } from '@playwright/test';
import { dayTitle, monthGrid, today } from '../src/lib/library/calendar';
import { LIBRARY } from './library-helpers';

const STATES = ['vorhanden', 'noch nicht erschienen', 'fehlt', 'neutral'];

async function open(page: Page, query: string): Promise<void> {
  await page.goto(`${LIBRARY}/kalender${query}`);
  await expect(page.getByTestId('range')).toBeVisible();
  await page.waitForFunction(
    () => document.querySelector('ol.days, [data-testid="calendar-note"]') !== null,
  );
}

const columnsOf = (page: Page, selector: string) =>
  page
    .locator(selector)
    .evaluateAll((all) => new Set(all.map((el) => Math.round(el.getBoundingClientRect().x))).size);

test.describe('month grid (AC-01)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('seven columns headed Mo .. So, one row per week, every day of the month', async ({
    page,
  }) => {
    await open(page, '?ansicht=monat');
    await expect(page.locator('.weekdays > *')).toHaveText([
      'Mo',
      'Di',
      'Mi',
      'Do',
      'Fr',
      'Sa',
      'So',
    ]);
    const weeks = monthGrid(today());
    const cells = page.locator('ol.days > li[data-day]');
    await expect(cells).toHaveCount(weeks.length * 7);
    expect(await columnsOf(page, 'ol.days > li[data-day]')).toBe(7);
    await expect(cells.first()).toHaveAttribute('data-day', weeks[0]?.[0]?.day ?? '');
    for (const cell of weeks.flat().filter((c) => !c.outside)) {
      await expect(page.locator(`li[data-day="${cell.day}"]`)).not.toHaveAttribute('data-outside');
    }
  });

  test('outside days are dimmed and empty; today is marked and named', async ({ page }) => {
    await open(page, '?ansicht=monat&datum=2026-11-16');
    const outside = page.locator('li[data-day="2026-10-26"]');
    await expect(outside).toHaveAttribute('data-outside', '');
    await expect(outside.getByTestId('calendar-entry')).toHaveCount(0);
    expect(Number(await outside.evaluate((el) => getComputedStyle(el).opacity))).toBeLessThan(1);
    await expect(page.locator('li[data-day="2026-11-16"] h3')).toHaveAccessibleName(
      dayTitle('2026-11-16') + (today() === '2026-11-16' ? ' · heute' : ''),
    );
    await open(page, '?ansicht=monat');
    const mark = page.locator(`li[data-day="${today()}"]`);
    await expect(mark).toHaveClass(/today/);
    await expect(mark.locator('h3')).toHaveAccessibleName(`${dayTitle(today())} · heute`);
  });

  test('entries in a month cell are cut to one line with the full text as tooltip', async ({
    page,
  }) => {
    await open(page, '?ansicht=monat');
    const entry = page.getByTestId('calendar-entry').first();
    await expect(entry).toBeVisible();
    const link = entry.locator('a, .title').first();
    const style = await link.evaluate((el) => ({
      wrap: getComputedStyle(el).whiteSpace,
      overflow: getComputedStyle(el).textOverflow,
    }));
    expect(style).toEqual({ wrap: 'nowrap', overflow: 'ellipsis' });
    await expect(link).toHaveAttribute('title', (await link.textContent())?.trim() ?? '');
  });
});

test.describe('week columns (AC-02)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('seven days side by side, headed with weekday and date, empty days say Nichts.', async ({
    page,
  }) => {
    await open(page, '');
    const days = page.locator('ol.days > li[data-day]');
    await expect(days).toHaveCount(7);
    expect(await columnsOf(page, 'ol.days > li[data-day]')).toBe(7);
    const tops = await days.evaluateAll(
      (all) => new Set(all.map((el) => Math.round(el.getBoundingClientRect().y))).size,
    );
    expect(tops).toBe(1);
    await expect(page.locator(`li[data-day="${today()}"] h3`)).toContainText('heute');
    const empty = days.filter({ hasNot: page.getByTestId('calendar-entry') });
    if ((await empty.count()) > 0) await expect(empty.first()).toContainText('Nichts.');
  });
});

test.describe('agenda (AC-03)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test('the week stacks all seven days; the month only days with entries', async ({ page }) => {
    await open(page, '');
    const days = page.locator('ol.days > li[data-day]');
    await expect(days).toHaveCount(7);
    expect(await columnsOf(page, 'ol.days > li[data-day]')).toBe(1);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
    await open(page, '?ansicht=monat');
    const shown = page.locator('ol.days > li[data-day]:visible');
    const withEntries = page.locator('ol.days > li[data-day]').filter({
      has: page.getByTestId('calendar-entry'),
    });
    await expect(shown).toHaveCount(await withEntries.count());
    await expect(page.locator('.weekdays')).toBeHidden();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  });

  test('an empty month shows only the note', async ({ page }) => {
    await open(page, '?ansicht=monat&datum=2090-02-01');
    await expect(page.getByTestId('calendar-note')).toBeVisible();
    await expect(page.locator('ol.days > li[data-day]:visible')).toHaveCount(0);
  });
});

test.describe('entries (AC-04)', () => {
  test('title, detail and a state dot named by its German text', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await open(page, '');
    const entries = page.getByTestId('calendar-entry');
    await expect(entries.first()).toBeVisible();
    for (const entry of await entries.all()) {
      const dot = entry.getByRole('img');
      await expect(dot).toHaveCount(1);
      const name = await dot.getAttribute('aria-label');
      expect(STATES).toContain(name);
      await expect(dot).toHaveAttribute('title', name ?? '');
    }
    await expect(entries.filter({ hasText: 'Kalenderfilm' }).first()).toContainText('Kino');
  });
});

test.describe('toolbar (AC-06)', () => {
  test('Woche and Monat are a segmented control; the range changes as before', async ({ page }) => {
    await open(page, '');
    const group = page.getByRole('group', { name: 'Zeitraum' });
    await expect(group).toHaveClass(/am-seg/);
    await expect(group.getByRole('button', { name: 'Woche' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await group.getByRole('button', { name: 'Monat' }).click();
    await expect(page).toHaveURL(/ansicht=monat/);
    await expect(group.getByRole('button', { name: 'Monat' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });
});
