// AC-01, AC-02, AC-17: the binary serves the UI itself, every page address opens directly.

import { expect, test } from '@playwright/test';
import { expectOverview, expectSystemPage } from './helpers';
import { SERVERS } from './servers';

test.describe('serving the UI (AC-01, AC-02)', () => {
  test('the binary answers / with the app document and the app starts', async ({ page }) => {
    const response = await page.goto(`${SERVERS.open}/`);
    expect(response?.status()).toBe(200);
    expect(response?.headers()['content-type']).toContain('text/html');
    await expect(page.locator('html')).toHaveAttribute('lang', 'de');
    await expectOverview(page);
  });

  test('a page address opens directly and survives a reload', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    await page.reload();
    await expectSystemPage(page);
    expect(new URL(page.url()).pathname).toBe('/system');
  });

  test('an unknown address shows "Seite nicht gefunden" with a link to the start', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.open}/gibt-es-nicht`);
    await expect(page.getByRole('heading', { name: 'Seite nicht gefunden' })).toBeVisible();
    await page.getByRole('link', { name: 'Zur Startseite' }).click();
    await expectOverview(page);
  });

  test('unknown assets and API paths are errors, not the app document', async ({ request }) => {
    const asset = await request.get(`${SERVERS.open}/_app/immutable/fehlt.js`);
    expect(asset.status()).toBe(404);
    const api = await request.get(`${SERVERS.open}/api/gibt-es-nicht`);
    expect(api.status()).toBe(404);
    expect(await api.json()).toEqual({ error: expect.any(String) });
    const locked = await request.get(`${SERVERS.auth}/api/gibt-es-nicht`);
    expect(locked.status()).toBe(401);
  });
});

test.describe('caching (AC-17)', () => {
  test('assets are cached for a year, the app document is always checked', async ({ page }) => {
    const assets: string[] = [];
    page.on('response', (response) => {
      if (new URL(response.url()).pathname.startsWith('/_app/immutable/')) {
        assets.push(response.headers()['cache-control'] ?? '');
      }
    });
    const document = await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(document?.headers()['cache-control']).toBe('no-cache');
    expect(assets.length).toBeGreaterThan(0);
    for (const header of assets) expect(header).toContain('max-age=31536000');
  });
});
