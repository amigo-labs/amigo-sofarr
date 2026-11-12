// AC-16: the transfer budget of a first visit; AC-19: recovery from assets of an old build.

import { expect, test, type Browser, type Page } from '@playwright/test';
import { expectOverview, expectSystemPage } from './helpers';
import { SERVERS } from './servers';

const BUDGET_BYTES = 200 * 1024;

/** Encoded (gzip) bytes of every script and style a fresh browser loads for `url`. */
async function scriptAndStyleBytes(browser: Browser, url: string): Promise<number> {
  const context = await browser.newContext();
  const page = await context.newPage();
  const sizes: Promise<number>[] = [];
  page.on('requestfinished', (request) => {
    if (['script', 'stylesheet'].includes(request.resourceType())) {
      sizes.push(request.sizes().then((s) => s.responseBodySize));
    }
  });
  await page.goto(url, { waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  const total = (await Promise.all(sizes)).reduce((a, b) => a + b, 0);
  await context.close();
  return total;
}

test.describe('transfer budget (AC-16)', () => {
  for (const url of [
    `${SERVERS.open}/`,
    `${SERVERS.open}/system`,
    `${SERVERS.auth}/anmelden`,
    `${SERVERS.open}/entdecken`,
  ]) {
    test(`${url} loads at most 200 KB of JavaScript and CSS (gzip)`, async ({ browser }) => {
      const bytes = await scriptAndStyleBytes(browser, url);
      test.info().annotations.push({ type: 'bytes', description: String(bytes) });
      expect(bytes).toBeGreaterThan(0);
      expect(bytes).toBeLessThanOrEqual(BUDGET_BYTES);
    });
  }

  test('scripts and styles arrive gzip-compressed', async ({ page }) => {
    const encodings: string[] = [];
    page.on('response', (response) => {
      const type = response.request().resourceType();
      if (type === 'script' || type === 'stylesheet') {
        encodings.push(response.headers()['content-encoding'] ?? 'none');
      }
    });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(encodings.length).toBeGreaterThan(0);
    expect(new Set(encodings)).toEqual(new Set(['gzip']));
  });
});

/** Page code of the page itself (not the layout `0` or the error page `1`). */
const PAGE_NODE = /\/_app\/immutable\/nodes\/(?![01]\.)[^/]+\.js$/;

function countDocuments(page: Page): () => number {
  let documents = 0;
  page.on('request', (request) => {
    if (request.resourceType() === 'document') documents += 1;
  });
  return () => documents;
}

const GONE = { status: 404, body: 'Datei nicht gefunden' } as const;

test.describe('assets of an old build (AC-19)', () => {
  test('a missing asset reloads the app document once and the page appears', async ({ page }) => {
    const documents = countDocuments(page);
    let failed = 0;
    // The old build's page code is gone until the app document is loaded again, as after
    // an update (the new document names new files).
    await page.route(PAGE_NODE, (route) => {
      if (documents() > 1) return route.continue();
      failed += 1;
      return route.fulfill(GONE);
    });
    await page.goto(`${SERVERS.open}/system`);
    await expectSystemPage(page);
    expect(failed).toBeGreaterThan(0);
    expect(documents()).toBe(2);
  });

  test('an asset that still fails after the reload shows an error instead of looping', async ({
    page,
  }) => {
    const documents = countDocuments(page);
    await page.route(PAGE_NODE, (route) => route.fulfill(GONE));
    await page.goto(`${SERVERS.open}/system`);
    const error = page.getByRole('alert');
    await expect(error).toContainText('Die Oberfläche konnte nicht geladen werden');
    await expect(error).toContainText('Wahrscheinlich wurde sofarr aktualisiert');
    await page.waitForTimeout(1500);
    expect(documents()).toBe(2);

    await page.unroute(PAGE_NODE);
    await error.getByRole('button', { name: 'Neu laden' }).click();
    await expectSystemPage(page);
  });

  test('a page changed by a newer build loads after one reload during navigation', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.open}/entdecken`);
    await expect(page.getByRole('heading', { name: 'Seite nicht gefunden' })).toBeVisible();
    const documents = countDocuments(page);
    let failed = 0;
    await page.route(PAGE_NODE, (route) => {
      if (documents() > 0) return route.continue();
      failed += 1;
      return route.abort('failed');
    });
    await page.getByRole('link', { name: 'Zur Startseite' }).click();
    await expectOverview(page);
    expect(failed).toBeGreaterThan(0);
    // One new app document, for the address the navigation went to.
    expect(documents()).toBe(1);
  });
});
