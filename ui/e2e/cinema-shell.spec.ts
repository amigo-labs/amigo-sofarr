// Spec ui-cinema-shell: no theme switch, always the dark palette, a quieter shell (the
// sidebar on the page ground, the current entry as an accent bar), flat boxes, the calendar's
// month as a grid of tiles and its week with posters.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import type { CalendarAnswer } from '../src/lib/api/generated/CalendarAnswer';
import type { CalendarEntry } from '../src/lib/api/generated/CalendarEntry';
import { daysOf, rangeOf, today } from '../src/lib/library/calendar';
import {
  SAMPLE_SYSTEM,
  expectNoHorizontalScroll,
  expectOverview,
  expectSystemPage,
  json,
  logIn,
} from './helpers';
import { LIBRARY } from './library-helpers';
import { SERVERS } from './servers';
import { THEME_KEY, storeTheme, styleOf, token } from './theme-helpers';

const WIDE = { width: 1440, height: 900 };
const NARROW = { width: 360, height: 740 };
const TRANSPARENT = 'rgba(0, 0, 0, 0)';

const nav = (page: Page) => page.getByRole('navigation', { name: 'Hauptnavigation' });
const background = (page: Page) =>
  page.evaluate(() => getComputedStyle(document.body).backgroundColor);

/** No theme switch and none of its buttons anywhere on the page. */
async function expectNoSwitch(page: Page): Promise<void> {
  await expect(page.getByRole('group', { name: 'Farbschema' })).toHaveCount(0);
  for (const name of ['Dunkel', 'Hell', 'System']) {
    await expect(page.getByRole('button', { name, exact: true })).toHaveCount(0);
  }
  await expect(page.locator('.theme-switch')).toHaveCount(0);
}

test.describe('no theme switch (AC-01)', () => {
  test('not in the sidebar, expanded or collapsed', async ({ page }) => {
    await page.setViewportSize(WIDE);
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await expect(page.locator('.sidebar')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Abmelden' })).toBeVisible();
    await expectNoSwitch(page);
    await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
    await expect(page.getByRole('button', { name: 'Seitenleiste ausklappen' })).toBeVisible();
    await expectNoSwitch(page);
  });

  test('not in the drawer', async ({ page }) => {
    await page.setViewportSize(NARROW);
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Menü öffnen' }).click();
    const drawer = page.locator('.drawer');
    await expect(drawer).toBeVisible();
    await expect(drawer.getByRole('button', { name: 'Abmelden' })).toBeVisible();
    await expectNoSwitch(page);
  });

  test('not in the top bar of the login page', async ({ page }) => {
    for (const size of [NARROW, WIDE]) {
      await page.setViewportSize(size);
      await page.goto(`${SERVERS.auth}/anmelden`);
      await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible();
      await expect(page.locator('.topbar .brand')).toBeVisible();
      await expectNoSwitch(page);
    }
  });
});

/** Records `data-theme` at the moment the body first has content. */
async function watchFirstContent(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const seen = { done: false, theme: null as string | null };
    Object.assign(window, { firstContent: seen });
    new MutationObserver((_, observer) => {
      if (document.body === null || document.body.childNodes.length === 0) return;
      seen.done = true;
      seen.theme = document.documentElement.getAttribute('data-theme');
      observer.disconnect();
    }).observe(document, { childList: true, subtree: true });
  });
}

const firstContent = (page: Page) =>
  page.evaluate(
    () =>
      (window as unknown as { firstContent: { done: boolean; theme: string | null } }).firstContent,
  );

test.describe('always the dark palette (AC-02)', () => {
  const PAGES = [
    { name: 'System', url: `${SERVERS.open}/system`, ready: expectSystemPage },
    { name: 'Übersicht', url: `${SERVERS.open}/`, ready: expectOverview },
    {
      name: 'Anmelden',
      url: `${SERVERS.auth}/anmelden`,
      ready: (page: Page) =>
        expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible(),
    },
  ] as const;

  for (const stored of ['light', 'system', 'dark', null] as const) {
    test(`a stored ${stored ?? 'nothing'} under a light system preference: dark from the start`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: 'light' });
      if (stored !== null) await storeTheme(page, stored);
      await watchFirstContent(page);
      for (const target of PAGES) {
        await page.goto(target.url);
        await target.ready(page);
        expect(await firstContent(page), target.name).toEqual({ done: true, theme: 'dark' });
        expect(await background(page), target.name).toBe(token('dark', 'bg'));
        expect(await styleOf(page, 'body', 'color'), target.name).toBe(token('dark', 'text'));
        // The stored choice stays in the browser, untouched.
        expect(await page.evaluate((key) => localStorage.getItem(key), THEME_KEY)).toBe(stored);
      }
      // The system preference changing moves nothing.
      await page.emulateMedia({ colorScheme: 'dark' });
      expect(await background(page)).toBe(token('dark', 'bg'));
      await page.emulateMedia({ colorScheme: 'light' });
      expect(await background(page)).toBe(token('dark', 'bg'));
    });
  }

  test('storage that throws: dark, and nothing fails', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.addInitScript(() => {
      Object.defineProperty(window, 'localStorage', {
        get() {
          throw new DOMException('Der Zugriff ist blockiert', 'SecurityError');
        },
      });
    });
    await page.emulateMedia({ colorScheme: 'light' });
    await watchFirstContent(page);
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(await firstContent(page)).toEqual({ done: true, theme: 'dark' });
    expect(await background(page)).toBe(token('dark', 'bg'));
    expect(errors).toEqual([]);
  });

  test('the document asks the browser for dark controls only', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await expect(page.locator('meta[name="color-scheme"]')).toHaveAttribute('content', 'dark');
    expect(await styleOf(page, 'html', 'color-scheme')).toBe('dark');
  });
});

/** The current entry's accent bar: the `::before` of the link. */
const bar = (page: Page, scope: string) =>
  page.locator(`${scope} nav a[aria-current="page"]`).evaluate((el) => {
    const before = getComputedStyle(el, '::before');
    return {
      content: before.content,
      position: before.position,
      left: before.left,
      width: before.width,
      background: before.backgroundColor,
      // The bar is placed against the entry itself.
      anchor: getComputedStyle(el).position,
    };
  });

test.describe('a quieter shell (AC-03)', () => {
  test('the sidebar stands on the page ground without a border, expanded and collapsed', async ({
    page,
  }) => {
    await page.setViewportSize(WIDE);
    await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    for (const state of ['expanded', 'collapsed']) {
      expect(await styleOf(page, '.sidebar', 'background-color'), state).toBe(token('dark', 'bg'));
      expect(await styleOf(page, '.sidebar', 'border-right-width'), state).toBe('0px');
      const current = nav(page).locator('a[aria-current="page"]');
      await expect(current).toHaveText('System');
      expect(
        await current.evaluate((el) => ({
          color: getComputedStyle(el).color,
          background: getComputedStyle(el).backgroundColor,
        })),
        state,
      ).toEqual({ color: token('dark', 'accent-ink'), background: TRANSPARENT });
      expect(await bar(page, '.sidebar'), state).toEqual({
        content: '""',
        position: 'absolute',
        left: '0px',
        width: '3px',
        background: token('dark', 'accent-ink'),
        anchor: 'relative',
      });
      if (state === 'expanded') {
        await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
        await expect(page.getByRole('button', { name: 'Seitenleiste ausklappen' })).toBeVisible();
      }
    }
  });

  test('other entries are muted and take the text colour on hover and focus, never a ground', async ({
    page,
  }) => {
    await page.setViewportSize(WIDE);
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    const filme = nav(page).getByRole('link', { name: 'Filme' });
    const look = () =>
      filme.evaluate((el) => ({
        color: getComputedStyle(el).color,
        background: getComputedStyle(el).backgroundColor,
        bar: getComputedStyle(el, '::before').content,
      }));
    expect(await look()).toEqual({
      color: token('dark', 'text-secondary'),
      background: TRANSPARENT,
      bar: 'none',
    });
    await filme.hover();
    await expect
      .poll(look)
      .toEqual({ color: token('dark', 'text'), background: TRANSPARENT, bar: 'none' });
    await page.mouse.move(WIDE.width - 10, WIDE.height - 10);
    await expect.poll(look).toEqual({
      color: token('dark', 'text-secondary'),
      background: TRANSPARENT,
      bar: 'none',
    });
    await nav(page).getByRole('link', { name: 'Serien' }).focus();
    await page.keyboard.press('Shift+Tab');
    await expect(filme).toBeFocused();
    await expect
      .poll(look)
      .toEqual({ color: token('dark', 'text'), background: TRANSPARENT, bar: 'none' });
  });

  test('the drawer stands on the page ground without a border, with the same entries', async ({
    page,
  }) => {
    await page.setViewportSize(NARROW);
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Menü öffnen' }).click();
    await expect(page.locator('.drawer')).toBeVisible();
    expect(await styleOf(page, '.drawer', 'background-color')).toBe(token('dark', 'bg'));
    expect(await styleOf(page, '.drawer', 'border-right-width')).toBe('0px');
    const current = page.locator('.drawer nav a[aria-current="page"]');
    expect(await current.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(TRANSPARENT);
    expect(await bar(page, '.drawer')).toMatchObject({
      position: 'absolute',
      left: '0px',
      width: '3px',
      background: token('dark', 'accent-ink'),
    });
  });
});

test.describe('flat boxes (AC-04)', () => {
  test('no border, the surface ground, 8 px corners, no line below the header; rows keep hairlines', async ({
    page,
  }) => {
    await page.setViewportSize(WIDE);
    await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    const boxes = page.locator('main .box');
    expect(await boxes.count()).toBeGreaterThan(1);
    for (const box of await boxes.all()) {
      expect(
        await box.evaluate((el) => {
          const style = getComputedStyle(el);
          return {
            borders: [
              style.borderTopWidth,
              style.borderRightWidth,
              style.borderBottomWidth,
              style.borderLeftWidth,
            ],
            background: style.backgroundColor,
            radius: style.borderTopLeftRadius,
          };
        }),
      ).toEqual({
        borders: ['0px', '0px', '0px', '0px'],
        background: token('dark', 'surface'),
        radius: '8px',
      });
    }
    const header = page.locator('main .box .box-header').first();
    expect(await header.evaluate((el) => getComputedStyle(el).borderBottomWidth)).toBe('0px');
    // A row after a row keeps its hairline; the first row below the header has none.
    const rows = page.locator('main .box .box-row + .box-row');
    expect(await rows.count()).toBeGreaterThan(0);
    expect(
      await rows.first().evaluate((el) => ({
        width: getComputedStyle(el).borderTopWidth,
        color: getComputedStyle(el).borderTopColor,
      })),
    ).toEqual({ width: '1px', color: token('dark', 'border') });
    const first = page.locator('main .box .box-header + .box-row').first();
    expect(await first.evaluate((el) => getComputedStyle(el).borderTopWidth)).toBe('0px');
  });
});

async function openCalendar(page: Page, query: string): Promise<void> {
  await page.goto(`${LIBRARY}/kalender${query}`);
  await expect(page.getByTestId('range')).toBeVisible();
  await page.waitForFunction(
    () => document.querySelector('ol.days, [data-testid="calendar-note"]') !== null,
  );
}

test.describe('the month as tiles (AC-05)', () => {
  test.use({ viewport: WIDE });

  test('surface days in 2 px gaps of the page ground, outside days on the ground, today ringed', async ({
    page,
  }) => {
    await openCalendar(page, '?ansicht=monat');
    const grid = page.locator('.month ol.days');
    expect(
      await grid.evaluate((el) => ({
        rowGap: getComputedStyle(el).rowGap,
        columnGap: getComputedStyle(el).columnGap,
      })),
    ).toEqual({ rowGap: '2px', columnGap: '2px' });
    // What shows through the gaps: the grid and its container stand on the page ground.
    for (const selector of ['.month', '.month ol.days']) {
      const ground = await styleOf(page, selector, 'background-color');
      expect([TRANSPARENT, token('dark', 'bg')], selector).toContain(ground);
      expect(await styleOf(page, selector, 'border-top-width'), selector).toBe('0px');
    }
    const cells = page.locator('.month ol.days > li[data-day]');
    for (const cell of await cells.all()) {
      const look = await cell.evaluate((el) => {
        const style = getComputedStyle(el);
        return {
          outside: el.hasAttribute('data-outside'),
          background: style.backgroundColor,
          borders: [
            style.borderTopWidth,
            style.borderRightWidth,
            style.borderBottomWidth,
            style.borderLeftWidth,
          ],
        };
      });
      expect(look.borders).toEqual(['0px', '0px', '0px', '0px']);
      expect(look.background).toBe(token('dark', look.outside ? 'bg' : 'surface'));
    }
    // Neighbours are 2 px apart.
    const [a, b] = await cells.evaluateAll((all) =>
      all.slice(0, 2).map((el) => el.getBoundingClientRect()),
    );
    expect(Math.round((b?.left ?? 0) - ((a?.left ?? 0) + (a?.width ?? 0)))).toBe(2);
    const ring = await page
      .locator(`li[data-day="${today()}"]`)
      .evaluate((el) => getComputedStyle(el).boxShadow);
    expect(ring).toBe(`${token('dark', 'accent-ink')} 0px 0px 0px 2px inset`);
    const other = cells.locator(':scope:not(.today)').first();
    expect(await other.evaluate((el) => getComputedStyle(el).boxShadow)).toBe('none');
    expect(await styleOf(page, '.weekdays span', 'border-bottom-width')).toBe('0px');
  });
});

const entry = (date: string, fields: Partial<CalendarEntry>): CalendarEntry => ({
  date,
  kind: 'movie',
  release: 'cinema',
  movie_id: 1,
  series_id: null,
  episode_id: null,
  season: null,
  episode: null,
  title: 'Film',
  episode_title: null,
  year: 2026,
  state: 'upcoming',
  monitored: true,
  has_poster: false,
  ...fields,
});

/** A poster of one colour, 2:3. */
const POSTER =
  '<svg xmlns="http://www.w3.org/2000/svg" width="342" height="513"><rect width="342" height="513" fill="#3060a0"/></svg>';

/**
 * Serves this week's calendar with three titles on its first day: one with a poster, one
 * without, and one whose poster fails to load.
 */
async function weekWithPosters(page: Page): Promise<void> {
  const range = rangeOf('woche', today());
  const day = daysOf(range)[0] ?? today();
  const answer: CalendarAnswer = {
    ...range,
    entries: [
      entry(day, { movie_id: 11, title: 'Mit Plakat', has_poster: true }),
      entry(day, {
        kind: 'episode',
        release: null,
        movie_id: null,
        series_id: 12,
        episode_id: 5,
        season: 1,
        episode: 2,
        title: 'Ohne Plakat',
        has_poster: false,
      }),
      entry(day, { movie_id: 13, title: 'Kaputtes Plakat', has_poster: true }),
    ],
    note: null,
  };
  await page.route('**/api/calendar?*', (route) => json(route, answer));
  await page.route('**/api/images/movie/11/list', (route) =>
    route.fulfill({ contentType: 'image/svg+xml', body: POSTER }),
  );
  await page.route('**/api/images/movie/13/list', (route) =>
    route.fulfill({ status: 404, body: '' }),
  );
}

test.describe('posters in the week (AC-06)', () => {
  test.use({ viewport: WIDE });

  test('each entry shows its poster, 40 px wide and 2:3, before its text; else the placeholder', async ({
    page,
  }) => {
    await weekWithPosters(page);
    await openCalendar(page, '');
    const entries = page.locator('.week [data-testid="calendar-entry"]');
    await expect(entries).toHaveCount(3);
    const posterOf = (title: string) =>
      page
        .locator('.week .entries > li', { has: page.getByText(title, { exact: true }) })
        .locator('.poster');
    await expect(posterOf('Mit Plakat').locator('img')).toHaveAttribute(
      'src',
      '/api/images/movie/11/list',
    );
    await expect
      .poll(() =>
        posterOf('Mit Plakat')
          .locator('img')
          .evaluate((el) => (el as HTMLImageElement).naturalWidth),
      )
      .toBeGreaterThan(0);
    await expect(posterOf('Ohne Plakat').getByTestId('poster-placeholder')).toHaveText('O');
    await expect(posterOf('Kaputtes Plakat').getByTestId('poster-placeholder')).toHaveText('K');
    await expect(posterOf('Kaputtes Plakat').locator('img')).toHaveCount(0);
    for (const title of ['Mit Plakat', 'Ohne Plakat', 'Kaputtes Plakat']) {
      const poster = await posterOf(title).boundingBox();
      const text = await page.getByText(title, { exact: true }).boundingBox();
      expect(poster?.width, title).toBeCloseTo(40, 0);
      expect(poster?.height, title).toBeCloseTo(60, 0);
      expect((poster?.x ?? 0) + (poster?.width ?? 0), title).toBeLessThanOrEqual(text?.x ?? 0);
    }
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

/** Tabs through the page; every stop shows a 2 px focus outline and has a name. */
async function tabStops(page: Page, count: number): Promise<string[]> {
  const reached: string[] = [];
  for (let i = 0; i < count; i += 1) {
    await page.keyboard.press('Tab');
    const focus = await page.evaluate(() => {
      const el = document.activeElement;
      if (!(el instanceof HTMLElement) || el === document.body) return null;
      const style = getComputedStyle(el);
      return {
        name: el.getAttribute('aria-label') ?? el.textContent?.trim() ?? '',
        outline: style.outlineStyle !== 'none' && parseFloat(style.outlineWidth) >= 2,
      };
    });
    if (focus === null) break;
    expect(focus.outline, `visible focus on ${focus.name}`).toBe(true);
    expect(focus.name, 'a name for every control').not.toBe('');
    reached.push(focus.name);
  }
  return reached;
}

test.describe('accessibility in the dark palette (AC-07)', () => {
  for (const size of [WIDE, NARROW]) {
    test(`the week with posters, the month and the login page at ${size.width} px`, async ({
      page,
    }) => {
      await page.setViewportSize(size);
      await weekWithPosters(page);
      await openCalendar(page, '');
      await expect(page.getByTestId('poster-placeholder').first()).toBeVisible();
      await expectNoViolations(page);
      await expectNoHorizontalScroll(page);
      await openCalendar(page, '?ansicht=monat');
      await expectNoViolations(page);
      await expectNoHorizontalScroll(page);
      await page.goto(`${SERVERS.auth}/anmelden`);
      await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible();
      await expectNoViolations(page);
      await expectNoHorizontalScroll(page);
    });
  }

  test('every control of the sidebar is reachable by keyboard with a visible focus', async ({
    page,
  }) => {
    await page.setViewportSize(WIDE);
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await page.reload();
    await expectSystemPage(page);
    const reached = await tabStops(page, 20);
    expect(reached).toEqual(
      expect.arrayContaining([
        'Zum Inhalt springen',
        'sofarr',
        'Übersicht',
        'System',
        'Abmelden',
        'Seitenleiste einklappen',
      ]),
    );
    // Collapsed, the icon-only controls keep German names.
    await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
    await expect(page.getByRole('button', { name: 'Seitenleiste ausklappen' })).toBeVisible();
    for (const link of await nav(page).getByRole('link').all()) {
      expect(await link.evaluate((el) => (el.textContent ?? '').trim())).not.toBe('');
    }
  });

  test('the top bar and the drawer are reachable by keyboard with a visible focus', async ({
    page,
  }) => {
    await page.setViewportSize(NARROW);
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await page.reload();
    await expectSystemPage(page);
    const before = await tabStops(page, 3);
    expect(before).toEqual(expect.arrayContaining(['Zum Inhalt springen', 'Menü öffnen']));
    const menu = page.getByRole('button', { name: 'Menü öffnen' });
    await menu.focus();
    await page.keyboard.press('Enter');
    await expect(page.locator('.drawer')).toBeVisible();
    const inside = await tabStops(page, 20);
    expect(inside).toEqual(expect.arrayContaining(['System', 'Abmelden', 'sofarr']));
  });
});
