import { expect, type Page, type Route } from '@playwright/test';
import type { SystemAnswer } from '../src/lib/api/generated/SystemAnswer';
import { e2ePassword } from './env.mjs';
import { USERNAME } from './servers';

/** Logs in on the login page that is shown now. */
export async function logIn(page: Page, password: string = e2ePassword()): Promise<void> {
  await expect(page.getByRole('heading', { name: 'Bei sofarr anmelden' })).toBeVisible();
  await page.getByLabel('Benutzername').fill(USERNAME);
  await page.getByLabel('Passwort').fill(password);
  await page.getByRole('button', { name: 'Anmelden' }).click();
}

/** Whether the page fits the window width (no horizontal scrolling, AC-09). */
export async function expectNoHorizontalScroll(page: Page): Promise<void> {
  const fits = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
  expect(fits).toBe(true);
}

/** The system heading appears once the page loaded its data. */
export async function expectSystemPage(page: Page): Promise<void> {
  await expect(page.getByRole('heading', { level: 1, name: 'System' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Geplante Aufgaben' })).toBeVisible();
}

/** The overview at `/` (spec web-ui-library AC-01) once it loaded its data. */
export async function expectOverview(page: Page): Promise<void> {
  await expect(page.getByRole('heading', { level: 1, name: 'Übersicht' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Warteschlange' })).toBeVisible();
}

/** IMDb's attribution, linked to imdb.com, and the date of the ratings data (imdb-ratings AC-08). */
export async function expectAttribution(page: Page): Promise<void> {
  const link = page.getByRole('link', { name: /Information courtesy of IMDb/ });
  await expect(link).toHaveAttribute('href', 'https://www.imdb.com');
  await expect(page.locator('.imdb-attribution')).toContainText(
    /Stand der Bewertungen: \d{2}\.\d{2}\.\d{4}, \d{2}:\d{2}:\d{2}/,
  );
}

/** A system answer covering every state the page shows. */
export const SAMPLE_SYSTEM: SystemAnswer = {
  version: '9.8.7',
  mode: 'live',
  jobs: [
    {
      name: 'rss',
      label: 'RSS-Sync',
      interval_minutes: 30,
      last_run: '2026-11-12T10:00:00Z',
      result: { state: 'ok' },
      next_run: '2026-11-12T10:30:00Z',
    },
    {
      name: 'downloads',
      label: 'Download-Prüfung',
      interval_minutes: 2,
      last_run: '2026-11-12T10:02:00Z',
      result: { state: 'error', message: 'nzbget antwortet nicht: Verbindung abgelehnt' },
      next_run: '2026-11-12T10:04:00Z',
    },
    {
      name: 'import-lists',
      label: 'Import-Listen',
      interval_minutes: null,
      last_run: null,
      result: null,
      next_run: null,
    },
    {
      name: 'refresh',
      label: 'Metadaten aktualisieren',
      interval_minutes: 1440,
      last_run: null,
      result: null,
      next_run: '2026-11-13T00:00:00Z',
    },
  ],
  connections: [
    {
      kind: 'download_client',
      name: 'nzbget',
      state: 'error',
      at: '2026-11-12T10:02:00Z',
      message: 'Verbindung abgelehnt',
    },
    { kind: 'indexer', name: 'Indexer A', state: 'ok', at: '2026-11-12T10:00:00Z', message: null },
    { kind: 'indexer', name: 'Indexer B', state: 'no_contact', at: null, message: null },
  ],
  trailers: { active: false, message: 'Schattenmodus: sofarr lädt keine Trailer' },
  config: {
    server: { bind: '0.0.0.0:7676' },
    auth: { enabled: true, password_hash: '***', api_key: '***' },
    indexer: [{ name: 'Indexer A', url: 'https://a.example/api?***', api_key: '***' }],
    nzbget: null,
  },
};

/** Answers a route with JSON. */
export function json(route: Route, body: unknown, status = 200): Promise<void> {
  return route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
}
