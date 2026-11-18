// AC-08 .. AC-11 (keyboard), AC-14: navigation, narrow screens, theme, German texts.

import { expect, test, type Page } from '@playwright/test';
import {
  SAMPLE_SYSTEM,
  expectNoHorizontalScroll,
  expectOverview,
  expectSystemPage,
  json,
} from './helpers';
import { e2ePassword } from './env.mjs';
import { SERVERS } from './servers';
import { THEME_KEY, pickTheme, storeTheme, themeButton, token } from './theme-helpers';

const DARK_BG = token('dark', 'bg');
const LIGHT_BG = token('light', 'bg');

const background = (page: Page) =>
  page.evaluate(() => getComputedStyle(document.body).backgroundColor);

test.describe('navigation (AC-08)', () => {
  test('lists the built pages in order and marks the current one', async ({ page }) => {
    await page.goto(`${SERVERS.open}/`);
    await expectOverview(page);
    const nav = page.getByRole('navigation', { name: 'Hauptnavigation' });
    await expect(nav.getByRole('link')).toHaveText([
      'Übersicht',
      'Entdecken',
      'Filme',
      'Serien',
      'Kalender',
      'Fehlend',
      'Sperrliste',
      'Untertitel',
      'Neu encodieren',
      'Backups',
      'System',
    ]);
    await expect(nav.getByRole('link', { name: 'Übersicht' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    await nav.getByRole('link', { name: 'System' }).click();
    await expectSystemPage(page);
    await expect(nav.getByRole('link', { name: 'System' })).toHaveAttribute('aria-current', 'page');
    await expect(nav.getByRole('link', { name: 'Übersicht' })).not.toHaveAttribute('aria-current');
  });

  test('every page reaches every other page through the navigation', async ({ page }) => {
    await page.goto(`${SERVERS.open}/gibt-es-nicht`);
    await expect(page.getByRole('heading', { name: 'Seite nicht gefunden' })).toBeVisible();
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('link', { name: 'System' })
      .click();
    await expectSystemPage(page);
  });
});

test.describe('narrow screens (AC-09)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test('the navigation sits behind a menu button', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    const button = page.getByRole('button', { name: 'Menü öffnen' });
    const link = page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('link', { name: 'System' });
    await expect(link).toBeHidden();
    await button.click();
    await expect(page.getByRole('button', { name: 'Menü schließen' })).toHaveAttribute(
      'aria-expanded',
      'true',
    );
    await expect(link).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(link).toBeHidden();
    await expect(page.getByRole('button', { name: 'Menü öffnen' })).toBeFocused();
  });

  for (const [name, url] of [
    ['system page', `${SERVERS.open}/system`],
    ['login page', `${SERVERS.auth}/anmelden`],
    ['page not found', `${SERVERS.open}/gibt-es-nicht`],
  ] as const) {
    test(`the ${name} fits 360 px without horizontal scrolling`, async ({ page }) => {
      await page.goto(url);
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
      await expectNoHorizontalScroll(page);
    });
  }

  test('very long names and messages wrap instead of scrolling', async ({ page }) => {
    const long = 'Sehr-lange-Fehlermeldung-ohne-Leerzeichen-'.repeat(12);
    await page.route('**/api/system', (route) =>
      json(route, {
        ...SAMPLE_SYSTEM,
        jobs: [
          { ...SAMPLE_SYSTEM.jobs[1], label: long, result: { state: 'error', message: long } },
        ],
        connections: [{ kind: 'indexer', name: long, state: 'error', at: null, message: long }],
        config: { pfad: { [long]: long } },
      }),
    );
    await page.goto(`${SERVERS.open}/system`);
    await expect(page.getByRole('heading', { name: 'Verbindungen' })).toBeVisible();
    await expectNoHorizontalScroll(page);
  });
});

test.describe('wide screens', () => {
  test('show the navigation without a menu button', async ({ page }) => {
    await page.setViewportSize({ width: 1024, height: 768 });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await expect(page.getByRole('button', { name: 'Menü öffnen' })).toBeHidden();
    await expect(
      page
        .getByRole('navigation', { name: 'Hauptnavigation' })
        .getByRole('link', { name: 'System' }),
    ).toBeVisible();
  });
});

// web-ui-shell AC-10 as revised by spec amigo-theme (its AC-04 .. AC-08).
test.describe('theme (AC-10)', () => {
  test('is dark while nothing is stored, whatever the system prefers (AC-04)', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(await background(page)).toBe(DARK_BG);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    await expect(themeButton(page, 'Dunkel')).toHaveAttribute('aria-pressed', 'true');
    await page.emulateMedia({ colorScheme: 'dark' });
    expect(await background(page)).toBe(DARK_BG);
    expect(await page.evaluate((key) => localStorage.getItem(key), THEME_KEY)).toBeNull();
  });

  test('the switch offers Dunkel, Hell and System; a pick applies at once and is remembered (AC-05)', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await expect(page.getByRole('group', { name: 'Farbschema' }).getByRole('button')).toHaveText([
      'Dunkel',
      'Hell',
      'System',
    ]);
    await pickTheme(page, 'Hell');
    expect(await background(page)).toBe(LIGHT_BG);
    expect(await page.evaluate((key) => localStorage.getItem(key), THEME_KEY)).toBe('light');
    await page.reload();
    await expectSystemPage(page);
    expect(await background(page)).toBe(LIGHT_BG);
    await expect(themeButton(page, 'Hell')).toHaveAttribute('aria-pressed', 'true');
    // The system changing does not move a fixed choice.
    await page.emulateMedia({ colorScheme: 'dark' });
    expect(await background(page)).toBe(LIGHT_BG);
    await pickTheme(page, 'Dunkel');
    expect(await background(page)).toBe(DARK_BG);
    expect(await page.evaluate((key) => localStorage.getItem(key), THEME_KEY)).toBe('dark');
    await page.emulateMedia({ colorScheme: 'light' });
    expect(await background(page)).toBe(DARK_BG);
  });

  test('System follows the system setting, also when it changes (AC-06)', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await pickTheme(page, 'System');
    expect(await background(page)).toBe(LIGHT_BG);
    await page.emulateMedia({ colorScheme: 'dark' });
    expect(await background(page)).toBe(DARK_BG);
    await page.reload();
    await expectSystemPage(page);
    await expect(themeButton(page, 'System')).toHaveAttribute('aria-pressed', 'true');
    expect(await background(page)).toBe(DARK_BG);
    await page.emulateMedia({ colorScheme: 'light' });
    expect(await background(page)).toBe(LIGHT_BG);
  });

  // What the document carries at the moment its body first has content: `data-theme`.
  for (const [stored, expected] of [
    ['light', 'light'],
    ['dark', 'dark'],
    ['system', null],
    ['auto', 'dark'],
    ['', 'dark'],
  ] as const) {
    test(`a stored "${stored}" is on the document before the app renders (AC-07)`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: 'light' });
      await storeTheme(page, stored);
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
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
      const seen = await page.evaluate(
        () =>
          (window as unknown as { firstContent: { done: boolean; theme: string | null } })
            .firstContent,
      );
      expect(seen).toEqual({ done: true, theme: expected });
      expect(await background(page)).toBe(expected === 'dark' ? DARK_BG : LIGHT_BG);
    });
  }

  test('blocked storage: dark at the start, a pick applies but is not remembered (AC-08)', async ({
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
    await page.emulateMedia({ colorScheme: 'light' });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(await background(page)).toBe(DARK_BG);
    await pickTheme(page, 'Hell');
    expect(await background(page)).toBe(LIGHT_BG);
    await page
      .getByRole('navigation', { name: 'Hauptnavigation' })
      .getByRole('link', { name: 'Übersicht', exact: true })
      .click();
    await expectOverview(page);
    expect(await background(page)).toBe(LIGHT_BG);
    await page.reload();
    await expectOverview(page);
    expect(await background(page)).toBe(DARK_BG);
    expect(errors).toEqual([]);
  });

  test('storage that refuses writes: a pick applies until the reload (AC-08)', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.addInitScript(() => {
      Storage.prototype.setItem = () => {
        throw new DOMException('Der Speicher ist voll', 'QuotaExceededError');
      };
    });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await pickTheme(page, 'Hell');
    expect(await background(page)).toBe(LIGHT_BG);
    await page.reload();
    await expectSystemPage(page);
    expect(await background(page)).toBe(DARK_BG);
    expect(errors).toEqual([]);
  });

  test('without JavaScript the notice shows in the dark palette', async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false, colorScheme: 'light' });
    const page = await context.newPage();
    await page.goto(`${SERVERS.open}/system`);
    const notice = page.locator('p.noscript');
    await expect(notice).toBeVisible();
    await expect(notice).toContainText('benötigt JavaScript');
    expect(await notice.evaluate((el) => getComputedStyle(el).color)).toBe(token('dark', 'text'));
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    expect(await background(page)).toBe(DARK_BG);
    await context.close();
  });
});

test.describe('keyboard (AC-11)', () => {
  test('every control is reachable by Tab with a visible focus', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/system`);
    await page.getByLabel('Benutzername').fill('admin');
    await page.getByLabel('Passwort').fill(e2ePassword());
    await page.keyboard.press('Enter');
    await expectSystemPage(page);
    // A fresh document: the Tab order starts at its beginning.
    await page.reload();
    await expectSystemPage(page);
    const reached: string[] = [];
    // Skip link, brand, ten navigation entries, the theme switch, logout and collapse.
    for (let i = 0; i < 24; i += 1) {
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
      reached.push(focus.name);
    }
    expect(reached).toEqual(
      expect.arrayContaining(['Zum Inhalt springen', 'sofarr', 'System', 'Abmelden']),
    );
    expect(reached.some((name) => name.includes('Hell'))).toBe(true);
  });

  test('the login form works with the keyboard alone', async ({ page }) => {
    await page.goto(`${SERVERS.auth}/anmelden`);
    await page.getByLabel('Benutzername').focus();
    await page.keyboard.type('admin');
    await page.keyboard.press('Tab');
    await page.keyboard.type(e2ePassword());
    await page.keyboard.press('Tab');
    await expect(page.getByRole('button', { name: 'Anmelden' })).toBeFocused();
    await page.keyboard.press('Enter');
    await expectOverview(page);
  });
});

// Words an English UI would show; the configuration (`sofarr.toml` keys) is excluded.
const ENGLISH =
  /\b(Loading|Error|Retry|Login|Log in|Logout|Sign in|Username|Password|Settings|Not found|Menu|Theme|Light|Dark|Submit|Version mismatch|Jobs|Connections|never|disabled|enabled)\b/;

async function visibleText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const clone = document.body.cloneNode(true);
    if (!(clone instanceof HTMLElement)) return '';
    clone.querySelectorAll('[data-config], script, style').forEach((el) => el.remove());
    const labels = [...document.querySelectorAll('[aria-label], [title]')].map(
      (el) => `${el.getAttribute('aria-label') ?? ''} ${el.getAttribute('title') ?? ''}`,
    );
    return `${clone.innerText}\n${labels.join('\n')}\n${document.title}`;
  });
}

test.describe('German texts (AC-14)', () => {
  test('pages, errors and controls speak German', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(await visibleText(page)).not.toMatch(ENGLISH);

    await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
    await page.reload();
    await expect(page.getByText('Verbindung abgelehnt').first()).toBeVisible();
    expect(await visibleText(page)).not.toMatch(ENGLISH);

    await page.goto(`${SERVERS.open}/gibt-es-nicht`);
    await expect(page.getByRole('heading', { name: 'Seite nicht gefunden' })).toBeVisible();
    expect(await visibleText(page)).not.toMatch(ENGLISH);

    await page.goto(`${SERVERS.auth}/anmelden`);
    await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible();
    expect(await visibleText(page)).not.toMatch(ENGLISH);
  });
});
