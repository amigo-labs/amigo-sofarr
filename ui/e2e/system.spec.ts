// AC-12, AC-13: the system page, its refresh, and the handling of failed requests.

import { expect, test } from '@playwright/test';
import { SAMPLE_SYSTEM, expectSystemPage, json } from './helpers';
import { SECRETS, SERVERS } from './servers';

const SYSTEM = `${SERVERS.open}/system`;

test.describe('system page (AC-12)', () => {
  test('shows version, mode, jobs, connections and the configuration of the server', async ({
    page,
    request,
  }) => {
    const health = (await (await request.get(`${SERVERS.open}/api/health`)).json()) as {
      version: string;
    };
    await page.goto(SYSTEM);
    await expectSystemPage(page);
    await expect(page.getByTestId('version')).toHaveText(health.version);
    await expect(page.getByTestId('mode')).toContainText('Shadow');
    for (const label of ['RSS-Sync', 'Download-Prüfung', 'Import-Listen', 'Sicherung']) {
      await expect(page.getByRole('heading', { level: 3, name: label })).toBeVisible();
    }
    // Import lists are switched off and never ran in this configuration.
    const importLists = page.locator('[data-job="import-lists"]');
    await expect(importLists.getByText('aus')).toHaveCount(2);
    await expect(importLists).toContainText('noch nicht gelaufen');
    await expect(page.getByRole('heading', { name: 'Download-Client nzbget' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Indexer Testindexer' })).toBeVisible();
    const config = page.locator('[data-config]');
    await expect(config.getByRole('heading', { name: 'indexer 1' })).toBeVisible();
    await expect(config).toContainText('***');
    const text = await page.locator('body').innerText();
    for (const secret of [...SECRETS, 'indexer-pass']) expect(text).not.toContain(secret);
  });

  test('shows every job result and connection state in German', async ({ page }) => {
    await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
    await page.goto(SYSTEM);
    await expectSystemPage(page);
    await expect(page.getByTestId('version')).toHaveText('9.8.7');
    await expect(page.getByTestId('mode')).toContainText('Live');
    const rss = page.locator('[data-job="rss"]');
    await expect(rss).toContainText('OK');
    await expect(rss).toContainText('alle 30 Minuten');
    await expect(rss).toContainText('12.11.2026, 11:00:00');
    await expect(rss).toContainText('12.11.2026, 11:30:00');
    const downloads = page.locator('[data-job="downloads"]');
    await expect(downloads).toContainText('Fehler');
    await expect(downloads).toContainText('nzbget antwortet nicht: Verbindung abgelehnt');
    await expect(page.locator('[data-job="refresh"]')).toContainText('täglich');
    await expect(page.locator('[data-job="refresh"]')).toContainText('noch nicht gelaufen');
    const nzbget = page.locator('[data-connection="download_client"]');
    await expect(nzbget).toContainText('Fehler');
    await expect(nzbget).toContainText('Verbindung abgelehnt');
    await expect(nzbget).toContainText('Letzter Kontakt: 12.11.2026, 11:02:00');
    await expect(page.getByText('erreichbar')).toBeVisible();
    await expect(page.getByText('noch kein Kontakt')).toBeVisible();
  });

  test('shows "nicht konfiguriert" for a missing download client and indexer', async ({ page }) => {
    await page.route('**/api/system', (route) =>
      json(route, {
        ...SAMPLE_SYSTEM,
        connections: [
          {
            kind: 'download_client',
            name: 'nzbget',
            state: 'not_configured',
            at: null,
            message: null,
          },
          { kind: 'indexer', name: null, state: 'not_configured', at: null, message: null },
        ],
      }),
    );
    await page.goto(SYSTEM);
    await expect(page.getByText('nicht konfiguriert')).toHaveCount(2);
  });

  test('refreshes every 15 seconds while open', async ({ page }) => {
    await page.clock.install();
    let requests = 0;
    page.on('request', (r) => {
      if (r.url().endsWith('/api/system')) requests += 1;
    });
    await page.goto(SYSTEM);
    await expectSystemPage(page);
    expect(requests).toBe(1);
    await page.route('**/api/system', (route) =>
      json(route, { ...SAMPLE_SYSTEM, version: '2.0.0' }),
    );
    await page.clock.runFor(15_000);
    await expect(page.getByTestId('version')).toHaveText('2.0.0');
    expect(requests).toBe(2);
  });
});

test.describe('failed requests (AC-13)', () => {
  test('a first load with a server error shows a full-page error with a retry', async ({
    page,
  }) => {
    await page.route('**/api/system', (route) => json(route, { error: 'Datenbank gesperrt' }, 503));
    await page.goto(SYSTEM);
    const error = page.getByRole('alert');
    await expect(error).toContainText('Der Systemstatus ist nicht verfügbar');
    await expect(error).toContainText('Datenbank gesperrt');
    await page.unroute('**/api/system');
    await error.getByRole('button', { name: 'Erneut versuchen' }).click();
    await expectSystemPage(page);
  });

  test('an unreachable API on the first load says so in German', async ({ page }) => {
    await page.route('**/api/system', (route) => route.abort('connectionrefused'));
    await page.goto(SYSTEM);
    await expect(page.getByRole('alert')).toContainText('Der Server ist nicht erreichbar.');
  });

  test('a failed refresh keeps the data, offers a retry and retries by itself', async ({
    page,
  }) => {
    await page.clock.install();
    await page.route('**/api/system', (route) => json(route, SAMPLE_SYSTEM));
    await page.goto(SYSTEM);
    await expect(page.getByTestId('version')).toHaveText('9.8.7');

    await page.route('**/api/system', (route) => route.abort('connectionrefused'));
    await page.clock.runFor(15_000);
    const notice = page.getByRole('alert');
    await expect(notice).toContainText(
      'Aktualisieren fehlgeschlagen: Der Server ist nicht erreichbar.',
    );
    await expect(notice.getByRole('button', { name: 'Erneut versuchen' })).toBeVisible();
    await expect(page.getByTestId('version')).toHaveText('9.8.7');

    await page.route('**/api/system', (route) => json(route, { error: 'Zeitüberschreitung' }, 504));
    await notice.getByRole('button', { name: 'Erneut versuchen' }).click();
    await expect(notice).toContainText('Zeitüberschreitung');

    await page.route('**/api/system', (route) =>
      json(route, { ...SAMPLE_SYSTEM, version: '9.8.8' }),
    );
    await page.clock.runFor(30_000);
    await expect(page.getByTestId('version')).toHaveText('9.8.8');
    await expect(page.getByRole('alert')).toHaveCount(0);
  });

  test('an unreachable API at the start shows a full-page error with a retry', async ({ page }) => {
    await page.route('**/api/auth/me', (route) => json(route, { error: 'Interner Fehler' }, 500));
    await page.goto(SYSTEM);
    const error = page.getByRole('alert');
    await expect(error).toContainText('sofarr ist nicht erreichbar');
    await expect(error).toContainText('Interner Fehler');
    await page.unroute('**/api/auth/me');
    await error.getByRole('button', { name: 'Erneut versuchen' }).click();
    await expectSystemPage(page);
  });

  test('a slow answer shows a loading indicator', async ({ page }) => {
    await page.route('**/api/system', async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 1500));
      await json(route, SAMPLE_SYSTEM);
    });
    await page.goto(SYSTEM);
    await expect(page.getByText('Systemstatus wird geladen …')).toBeVisible();
    await expect(page.getByTestId('version')).toHaveText('9.8.7');
  });
});

test.describe('without JavaScript', () => {
  test.use({ javaScriptEnabled: false });

  test('a German note says that the UI needs JavaScript', async ({ page }) => {
    await page.goto(SYSTEM);
    // Text locators skip <noscript>; the paragraph is found by its role instead.
    const note = page.getByRole('paragraph');
    await expect(note).toBeVisible();
    await expect(note).toContainText('Die Weboberfläche von sofarr benötigt JavaScript.');
  });
});
