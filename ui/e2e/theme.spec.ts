// Spec amigo-theme AC-02, AC-03, AC-10 .. AC-12: the UI is drawn in the shared theme's
// tokens, in Inter, with the theme's control sizes and corners, and without Primer's styles.
// The palette choice itself (AC-04 .. AC-08) is tested in layout.spec.ts.

import { expect, test } from '@playwright/test';
import { SAMPLE_SYSTEM, expectSystemPage, json, logIn } from './helpers';
import { LIBRARY } from './library-helpers';
import { SERVERS } from './servers';
import { builtStylesheets } from './theme-files.mjs';
import { PALETTES, storeTheme, styleOf, token, tokenValue } from './theme-helpers';

for (const palette of PALETTES) {
  test.describe(`${palette} palette`, () => {
    test.beforeEach(async ({ page }) => {
      await storeTheme(page, palette);
    });

    test('grounds, text and hairlines are the tokens (AC-02)', async ({ page }) => {
      await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
      expect(await styleOf(page, 'body', 'background-color')).toBe(token(palette, 'bg'));
      expect(await styleOf(page, 'body', 'color')).toBe(token(palette, 'text'));
      expect(await styleOf(page, '.sidebar', 'background-color')).toBe(token(palette, 'surface'));
      expect(await styleOf(page, 'main .box', 'background-color')).toBe(token(palette, 'surface'));
      expect(await styleOf(page, 'main .box', 'border-top-color')).toBe(token(palette, 'border'));
      expect(await styleOf(page, 'main .muted', 'color')).toBe(token(palette, 'text-secondary'));
      // The theme switch is a raised control.
      const group = page.getByRole('group', { name: 'Farbschema' });
      expect(await group.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
        token(palette, 'surface-raised'),
      );
    });

    test('fields are surface-raised with muted placeholders, lists are surface (AC-02)', async ({
      page,
    }) => {
      await page.goto(`${LIBRARY}/filme`);
      const search = page.getByPlaceholder('Titel, Original- oder Alternativtitel');
      await expect(search).toBeVisible();
      const field = await search.evaluate((el) => ({
        background: getComputedStyle(el).backgroundColor,
        color: getComputedStyle(el).color,
        placeholder: getComputedStyle(el, '::placeholder').color,
      }));
      expect(field).toEqual({
        background: token(palette, 'surface-raised'),
        color: token(palette, 'text'),
        placeholder: token(palette, 'text-muted'),
      });
      await expect(page.locator('main .items').first()).toBeVisible();
      expect(await styleOf(page, 'main .items', 'background-color')).toBe(
        token(palette, 'surface'),
      );
      expect(await styleOf(page, 'main .items', 'border-top-color')).toBe(token(palette, 'border'));
    });

    test('links, the active navigation entry and the focus ring are the accent (AC-03)', async ({
      page,
    }) => {
      await page.goto(`${SERVERS.open}/gibt-es-nicht`);
      const link = page.getByRole('link', { name: 'Zur Startseite' });
      await expect(link).toBeVisible();
      expect(await link.evaluate((el) => getComputedStyle(el).color)).toBe(
        token(palette, 'accent-ink'),
      );
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
      const active = page
        .getByRole('navigation', { name: 'Hauptnavigation' })
        .locator('a[aria-current="page"]');
      await expect(active).toHaveText('System');
      expect(await active.evaluate((el) => getComputedStyle(el).color)).toBe(
        token(palette, 'accent-ink'),
      );
      // Keyboard focus, in the header and in the page.
      await page.goto(`${SERVERS.open}/gibt-es-nicht`);
      for (const name of ['sofarr', 'Zur Startseite']) {
        const target = page.getByRole('link', { name, exact: true });
        await target.focus();
        await page.keyboard.press('Shift+Tab');
        await page.keyboard.press('Tab');
        await expect(target).toBeFocused();
        const outline = await target.evaluate((el) => ({
          color: getComputedStyle(el).outlineColor,
          style: getComputedStyle(el).outlineStyle,
        }));
        expect(outline, `focus ring on ${name}`).toEqual({
          color: token(palette, 'focus'),
          style: 'solid',
        });
      }
    });

    test('the primary button is accent-solid with on-accent (AC-03)', async ({ page }) => {
      await page.goto(`${SERVERS.auth}/anmelden`);
      const button = page.getByRole('button', { name: 'Anmelden' });
      await expect(button).toBeVisible();
      expect(
        await button.evaluate((el) => ({
          background: getComputedStyle(el).backgroundColor,
          color: getComputedStyle(el).color,
        })),
      ).toEqual({
        background: token(palette, 'accent-solid'),
        color: token(palette, 'on-accent'),
      });
    });

    test('status messages show their words in the status ink on its subtle fill (AC-03)', async ({
      page,
    }) => {
      await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
      expect(await styleOf(page, 'main .label-success', 'color')).toBe(
        token(palette, 'success-ink'),
      );
      expect(await styleOf(page, 'main .label-danger', 'color')).toBe(token(palette, 'danger-ink'));
      // A failed login is an error message.
      await page.goto(`${SERVERS.auth}/anmelden`);
      await page.route('**/api/auth/login', (route) =>
        json(route, { error: 'Benutzername oder Passwort falsch' }, 401),
      );
      await logIn(page, 'falsch');
      const alert = page.getByRole('alert');
      await expect(alert).toBeVisible();
      expect(
        await alert.evaluate((el) => ({
          background: getComputedStyle(el).backgroundColor,
          color: getComputedStyle(el.querySelector('p') ?? el).color,
        })),
      ).toEqual({
        background: token(palette, 'danger-subtle'),
        color: token(palette, 'danger-ink'),
      });
    });

    test('buttons and fields are 32 px high with a 6 px corner, cards have 8 px (AC-11)', async ({
      page,
    }) => {
      expect(tokenValue(palette, 'control-md')).toBe('32px');
      expect(tokenValue(palette, 'radius-sm')).toBe('6px');
      expect(tokenValue(palette, 'radius-md')).toBe('8px');
      await page.goto(`${SERVERS.auth}/anmelden`);
      const box = async (name: 'button' | 'field') => {
        const target =
          name === 'button'
            ? page.getByRole('button', { name: 'Anmelden' })
            : page.getByLabel('Benutzername');
        await expect(target).toBeVisible();
        return target.evaluate((el) => ({
          height: el.getBoundingClientRect().height,
          radius: getComputedStyle(el).borderTopLeftRadius,
        }));
      };
      expect(await box('button')).toEqual({ height: 32, radius: '6px' });
      expect(await box('field')).toEqual({ height: 32, radius: '6px' });
      // The login form is a card; so are the boxes of the system page and the lists.
      expect(await styleOf(page, 'main form.box', 'border-top-left-radius')).toBe('8px');
      await page.goto(`${SERVERS.open}/system`);
      await expectSystemPage(page);
      expect(await styleOf(page, 'main .box', 'border-top-left-radius')).toBe('8px');
      // A default button: the action of a full-page error.
      await page.route('**/api/system', (route) =>
        json(route, { error: 'Datenbank gesperrt' }, 503),
      );
      await page.goto(`${SERVERS.open}/system`);
      const button = page.getByRole('button', { name: 'Erneut versuchen' });
      await expect(button).toBeVisible();
      expect(
        await button.evaluate((el) => ({
          height: el.getBoundingClientRect().height,
          radius: getComputedStyle(el).borderTopLeftRadius,
        })),
      ).toEqual({ height: 32, radius: '6px' });
      await page.unroute('**/api/system');
      await page.goto(`${LIBRARY}/filme`);
      await expect(page.locator('main .items').first()).toBeVisible();
      expect(await styleOf(page, 'main .items', 'border-top-left-radius')).toBe('8px');
    });
  });
}

test.describe('font (AC-10)', () => {
  test('text is set in Inter from sofarr itself; nothing comes from another host', async ({
    page,
  }) => {
    const origin = new URL(SERVERS.open).origin;
    const foreign: string[] = [];
    const fonts: { url: string; status: number; type: string }[] = [];
    page.on('request', (request) => {
      const url = new URL(request.url());
      if (url.protocol !== 'data:' && url.origin !== origin) foreign.push(request.url());
    });
    page.on('response', (response) => {
      if (response.request().resourceType() === 'font') {
        fonts.push({
          url: response.url(),
          status: response.status(),
          type: response.headers()['content-type'] ?? '',
        });
      }
    });
    await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
    await page.goto(`${SERVERS.open}/system`, { waitUntil: 'networkidle' });
    await expectSystemPage(page);
    const family = await styleOf(page, 'body', 'font-family');
    expect(family.startsWith('"Inter Variable"')).toBe(true);
    const loaded = await page.evaluate(async () => {
      await document.fonts.ready;
      return [...document.fonts]
        .filter((face) => face.status === 'loaded')
        .map((face) => face.family.replaceAll('"', ''));
    });
    expect(loaded).toContain('Inter Variable');
    expect(fonts.length).toBeGreaterThan(0);
    for (const font of fonts) {
      expect(new URL(font.url).origin).toBe(origin);
      expect(font.status).toBe(200);
      expect(font.type).toBe('font/woff2');
    }
    expect(foreign).toEqual([]);
  });
});

test.describe('Primer primitives are gone (AC-12)', () => {
  test('the built UI ships none of their stylesheets', () => {
    const sheets = builtStylesheets();
    expect(sheets.length).toBeGreaterThan(0);
    for (const { file, css } of sheets) {
      // Marks of Primer's theme and size files, which the UI's own aliases never carry.
      expect(css, file).not.toContain('data-color-mode');
      expect(css, file).not.toContain('--base-size-112');
      expect(css, file).not.toContain('--fgColor-sponsors');
    }
  });
});
