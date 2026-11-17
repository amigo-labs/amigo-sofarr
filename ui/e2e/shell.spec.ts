// Spec ui-shell-redesign AC-01 .. AC-09: the sidebar on wide windows, collapsible and
// remembered, the top bar and drawer on narrow ones, and the theme switch's buttons.

import { expect, test, type Page } from '@playwright/test';
import { expectNoHorizontalScroll, expectOverview, expectSystemPage, logIn } from './helpers';
import { SERVERS } from './servers';
import { PALETTES, storeTheme, token } from './theme-helpers';

const ENTRIES = [
  'Übersicht',
  'Entdecken',
  'Filme',
  'Serien',
  'Kalender',
  'Fehlend',
  'Sperrliste',
  'Untertitel',
  'Backups',
  'System',
];
const SIDEBAR_KEY = 'sofarr-sidebar';
const WIDE = { width: 1440, height: 900 };
const NARROW = { width: 360, height: 740 };

const nav = (page: Page) => page.getByRole('navigation', { name: 'Hauptnavigation' });
const box = async (page: Page, selector: string) => {
  const found = await page.locator(selector).first().boundingBox();
  if (found === null) throw new Error(`${selector} is not shown`);
  return found;
};

test.describe('wide window (AC-01 .. AC-05)', () => {
  test.use({ viewport: WIDE });

  test('the sidebar sits at the left with groups, entries, theme switch and logout (AC-01)', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await expect(page.getByRole('button', { name: 'Menü öffnen' })).toBeHidden();
    const sidebar = page.locator('.sidebar');
    await expect(sidebar).toBeVisible();
    const side = await box(page, '.sidebar');
    const main = await box(page, 'main');
    expect(side.x).toBe(0);
    expect(main.x).toBeGreaterThanOrEqual(side.x + side.width);
    expect(side.height).toBeGreaterThanOrEqual(WIDE.height - 1);
    await expect(nav(page).getByRole('link')).toHaveText(ENTRIES);
    // Übersicht and Entdecken first without a heading, then the two headed groups.
    await expect(sidebar.locator('.group-heading')).toHaveText(['Bibliothek', 'Verwaltung']);
    const first = sidebar.locator('.group').first();
    await expect(first.locator('.group-heading')).toHaveCount(0);
    await expect(first.getByRole('link')).toHaveText(['Übersicht', 'Entdecken']);
    // Brand on top; theme switch and logout below the entries.
    const brand = await box(page, '.sidebar .brand');
    const last = await nav(page).getByRole('link', { name: 'System' }).boundingBox();
    const theme = await page.getByRole('group', { name: 'Farbschema' }).boundingBox();
    const logout = await page.getByRole('button', { name: 'Abmelden' }).boundingBox();
    expect(brand.y).toBeLessThan(last?.y ?? 0);
    expect(theme?.y ?? 0).toBeGreaterThan(last?.y ?? 0);
    expect(logout?.y ?? 0).toBeGreaterThan(last?.y ?? 0);
  });

  test('the sidebar also shows on the page-not-found page (AC-01)', async ({ page }) => {
    await page.goto(`${SERVERS.open}/gibt-es-nicht`);
    await expect(page.getByRole('heading', { name: 'Seite nicht gefunden' })).toBeVisible();
    await expect(page.locator('.sidebar')).toBeVisible();
  });

  for (const palette of PALETTES) {
    test(`every entry has an icon; the current one is the accent (AC-02, ${palette})`, async ({
      page,
    }) => {
      await storeTheme(page, palette);
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
      const links = nav(page).getByRole('link');
      await expect(links).toHaveCount(ENTRIES.length);
      for (const link of await links.all()) {
        await expect(link.locator('svg')).toHaveCount(1);
      }
      const current = nav(page).locator('a[aria-current="page"]');
      await expect(current).toHaveText('System');
      expect(
        await current.evaluate((el) => ({
          color: getComputedStyle(el).color,
          background: getComputedStyle(el).backgroundColor,
        })),
      ).toEqual({
        color: token(palette, 'accent-ink'),
        background: token(palette, 'accent-subtle'),
      });
    });
  }

  test('collapses to icons with names and tooltips, and expands again (AC-03)', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    const before = await box(page, 'main');
    const wideSidebar = await box(page, '.sidebar');
    await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
    const expand = page.getByRole('button', { name: 'Seitenleiste ausklappen' });
    await expect(expand).toBeVisible();
    const narrowSidebar = await box(page, '.sidebar');
    expect(narrowSidebar.width).toBeLessThan(wideSidebar.width);
    expect(narrowSidebar.width).toBeLessThanOrEqual(64);
    // The content takes the freed width: it grows and starts further left.
    const after = await box(page, 'main');
    expect(after.width).toBeGreaterThan(before.width);
    expect(after.x).toBeLessThan(before.x);
    await expect(page.locator('.sidebar .group-heading').first()).toBeHidden();
    // Names stay; the label shows as a tooltip on hover and on focus.
    const filme = nav(page).getByRole('link', { name: 'Filme' });
    await expect(filme).toBeVisible();
    const tooltip = page.locator('.nav-tooltip');
    await filme.hover();
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toHaveText('Filme');
    await page.mouse.move(WIDE.width - 10, WIDE.height - 10);
    await expect(tooltip).toBeHidden();
    await nav(page).getByRole('link', { name: 'Serien' }).focus();
    await expect(tooltip).toHaveText('Serien');
    await expect(tooltip).toBeVisible();
    await expand.click();
    await expect(page.getByRole('button', { name: 'Seitenleiste einklappen' })).toBeVisible();
    await expect(page.locator('.sidebar .group-heading').first()).toBeVisible();
    expect((await box(page, '.sidebar')).width).toBe(wideSidebar.width);
  });

  test('the collapsed state is on the document before the app renders (AC-04)', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
    expect(await page.evaluate((key) => localStorage.getItem(key), SIDEBAR_KEY)).toBe('collapsed');
    await page.addInitScript(() => {
      const seen = { done: false, sidebar: null as string | null };
      Object.assign(window, { firstContent: seen });
      new MutationObserver((_, observer) => {
        if (document.body === null || document.body.childNodes.length === 0) return;
        seen.done = true;
        seen.sidebar = document.documentElement.getAttribute('data-sidebar');
        observer.disconnect();
      }).observe(document, { childList: true, subtree: true });
    });
    await page.reload();
    await expectSystemPage(page);
    const seen = await page.evaluate(
      () =>
        (window as unknown as { firstContent: { done: boolean; sidebar: string | null } })
          .firstContent,
    );
    expect(seen).toEqual({ done: true, sidebar: 'collapsed' });
    await expect(page.getByRole('button', { name: 'Seitenleiste ausklappen' })).toBeVisible();
    await page.getByRole('button', { name: 'Seitenleiste ausklappen' }).click();
    expect(await page.evaluate((key) => localStorage.getItem(key), SIDEBAR_KEY)).toBeNull();
    await page.reload();
    await expectSystemPage(page);
    await expect(page.locator('html')).not.toHaveAttribute('data-sidebar', /./);
    await expect(page.getByRole('button', { name: 'Seitenleiste einklappen' })).toBeVisible();
  });

  test('an unknown stored value counts as expanded (AC-04)', async ({ page }) => {
    await page.addInitScript((key) => localStorage.setItem(key, 'zu'), SIDEBAR_KEY);
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await expect(page.getByRole('button', { name: 'Seitenleiste einklappen' })).toBeVisible();
  });

  test('blocked storage: expanded at the start, collapsing works, nothing is remembered (AC-05)', async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.addInitScript(() => {
      Object.defineProperty(window, 'localStorage', {
        get() {
          throw new DOMException('Der Zugriff ist blockiert', 'SecurityError');
        },
      });
    });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
    await expect(page.getByRole('button', { name: 'Seitenleiste ausklappen' })).toBeVisible();
    await page.reload();
    await expectSystemPage(page);
    await expect(page.getByRole('button', { name: 'Seitenleiste einklappen' })).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('storage that refuses writes: collapsed until the reload (AC-05)', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.addInitScript(() => {
      Storage.prototype.setItem = () => {
        throw new DOMException('Der Speicher ist voll', 'QuotaExceededError');
      };
    });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
    await expect(page.getByRole('button', { name: 'Seitenleiste ausklappen' })).toBeVisible();
    await page.reload();
    await expectSystemPage(page);
    await expect(page.getByRole('button', { name: 'Seitenleiste einklappen' })).toBeVisible();
    expect(errors).toEqual([]);
  });
});

test.describe('narrow window (AC-06 .. AC-08)', () => {
  test.use({ viewport: NARROW });

  test('a top bar with menu button and brand instead of the sidebar (AC-06)', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await expect(page.getByRole('button', { name: 'Menü öffnen' })).toBeVisible();
    await expect(page.locator('.topbar .brand')).toBeVisible();
    await expect(page.locator('.sidebar')).toBeHidden();
    await expect(nav(page)).toHaveCount(0);
    await expectNoHorizontalScroll(page);
  });

  test('the login page has a top bar with brand and theme switch, no menu (AC-06)', async ({
    page,
  }) => {
    for (const size of [NARROW, WIDE]) {
      await page.setViewportSize(size);
      await page.goto(`${SERVERS.auth}/anmelden`);
      await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible();
      await expect(page.locator('.topbar .brand')).toBeVisible();
      await expect(page.getByRole('group', { name: 'Farbschema' })).toBeVisible();
      await expect(page.getByRole('button', { name: 'Menü öffnen' })).toHaveCount(0);
      await expect(page.locator('.sidebar')).toHaveCount(0);
      await expectNoHorizontalScroll(page);
    }
  });

  test('the drawer slides in from the left with everything and keeps the focus (AC-07)', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.auth}/system`);
    await logIn(page);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Menü öffnen' }).click();
    const drawer = page.locator('.drawer');
    await expect(drawer).toBeVisible();
    await expect(page.getByRole('button', { name: 'Menü schließen' })).toHaveAttribute(
      'aria-expanded',
      'true',
    );
    await expect(nav(page).getByRole('link')).toHaveText(ENTRIES);
    await expect(drawer.getByRole('group', { name: 'Farbschema' })).toBeVisible();
    await expect(drawer.getByRole('button', { name: 'Abmelden' })).toBeVisible();
    await drawer.evaluate((el) => Promise.all(el.getAnimations().map((a) => a.finished)));
    const placed = await box(page, '.drawer');
    expect(placed.x).toBe(0);
    await expect(nav(page).getByRole('link', { name: 'Übersicht' })).toBeFocused();
    // Tab never leaves the drawer.
    for (let i = 0; i < 25; i += 1) {
      await page.keyboard.press('Tab');
      const inside = await page.evaluate(() => document.activeElement?.closest('.drawer') !== null);
      expect(inside, `focus inside the drawer after ${i + 1} Tabs`).toBe(true);
    }
  });

  test('the drawer opens without sliding where reduced motion is asked for (AC-07)', async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Menü öffnen' }).click();
    const animations = await page
      .locator('.drawer')
      .evaluate((el) => el.getAnimations().filter((a) => a.playState === 'running').length);
    expect(animations).toBe(0);
  });

  test('Escape and a click on the dimmed page close the drawer and return the focus (AC-08)', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    const open = page.getByRole('button', { name: 'Menü öffnen' });
    await open.click();
    await expect(page.locator('.drawer')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('.drawer')).toBeHidden();
    await expect(open).toBeFocused();
    await open.click();
    await expect(page.locator('.drawer')).toBeVisible();
    await page.mouse.click(NARROW.width - 10, NARROW.height / 2);
    await expect(page.locator('.drawer')).toBeHidden();
    await expect(open).toBeFocused();
  });

  test('a page change closes the drawer (AC-08)', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Menü öffnen' }).click();
    await nav(page).getByRole('link', { name: 'Übersicht' }).click();
    await expectOverview(page);
    await expect(page.locator('.drawer')).toBeHidden();
    await page.getByRole('button', { name: 'Menü öffnen' }).click();
    await expect(page.locator('.drawer')).toBeVisible();
    await page.goBack();
    await expectSystemPage(page);
    await expect(page.locator('.drawer')).toBeHidden();
  });

  test('growing the window to wide closes the drawer and shows the sidebar', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.getByRole('button', { name: 'Menü öffnen' }).click();
    await expect(page.locator('.drawer')).toBeVisible();
    await page.setViewportSize(WIDE);
    await expect(page.locator('.drawer')).toBeHidden();
    await expect(page.locator('.sidebar')).toBeVisible();
  });
});

test.describe('breakpoint', () => {
  test('1024 px shows the sidebar, 1023 px the menu button', async ({ page }) => {
    await page.setViewportSize({ width: 1024, height: 768 });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await expect(page.locator('.sidebar')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Menü öffnen' })).toBeHidden();
    await page.setViewportSize({ width: 1023, height: 768 });
    await expect(page.locator('.sidebar')).toBeHidden();
    await expect(page.getByRole('button', { name: 'Menü öffnen' })).toBeVisible();
  });
});

test.describe('theme switch (AC-09)', () => {
  test('three buttons named Farbschema, the pressed one marked, a pick applies and is remembered', async ({
    page,
  }) => {
    await page.setViewportSize(WIDE);
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    const group = page.getByRole('group', { name: 'Farbschema' });
    await expect(group.getByRole('button')).toHaveText(['Dunkel', 'Hell', 'System']);
    await expect(group.getByRole('button', { name: 'Dunkel' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await group.getByRole('button', { name: 'Hell' }).click();
    await expect(group.getByRole('button', { name: 'Hell' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await expect(group.getByRole('button', { name: 'Dunkel' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
    expect(await page.evaluate(() => localStorage.getItem('sofarr-theme'))).toBe('light');
    // Collapsed, the three choices stay reachable by name.
    await page.getByRole('button', { name: 'Seitenleiste einklappen' }).click();
    await expect(group.getByRole('button', { name: 'System' })).toBeVisible();
  });
});
