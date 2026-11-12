// spec web-ui-library AC-01, AC-07, AC-08, AC-18, AC-19 against the seeded library server;
// grabs and blocklist entries cannot be seeded without an indexer, so those answers are
// simulated with page.route where noted.

import { expect, test } from '@playwright/test';
import type { CalendarAnswer } from '../src/lib/api/generated/CalendarAnswer';
import { rangeOf, today } from '../src/lib/library/calendar';
import { expectOverview, json } from './helpers';
import { LIBRARY, apiTotal } from './library-helpers';

const entry = (fields: Record<string, unknown>) => ({
  kind: 'movie',
  grab_id: 1,
  media_id: 1,
  media_title: 'Film 0001',
  release_title: 'Film.0001.1951.1080p.BluRay-GRP',
  quality: 'Bluray-1080p',
  size: 8_000_000_000,
  status: 'imported',
  message: null,
  episode_ids: [],
  grabbed_at: '2026-11-12T08:00:00Z',
  updated_at: '2026-11-12T09:00:00Z',
  ...fields,
});

test.describe('overview (AC-01)', () => {
  test('shows queue notice, upcoming release dates and empty activity of the library', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/`);
    await expectOverview(page);
    await expect(page.getByText('Kein Download-Client konfiguriert')).toBeVisible();
    const upcoming = page.getByRole('region', { name: 'Die nächsten 7 Tage' });
    await expect(upcoming.getByRole('link', { name: 'Kalenderfilm' }).first()).toBeVisible();
    await expect(upcoming.getByText('S01E03 · Abschied')).toBeVisible();
    await expect(page.getByText('Noch keine Grabs oder Importe.')).toBeVisible();
    await expect(page.getByText('Keine fehlgeschlagenen Grabs oder Importprobleme.')).toBeVisible();
  });

  test('shows progress, size, remaining time ("–" when none), activity and problems', async ({
    page,
  }) => {
    await page.clock.install();
    let calls = 0;
    await page.route('**/api/overview', (route) => {
      calls += 1;
      return json(route, {
        queue: {
          items: [
            entry({
              grab_id: 2,
              download_state: 'downloading',
              progress: 0.5,
              remaining_bytes: 2_000_000_000,
              remaining_seconds: 3900,
              status: 'downloading',
            }),
            entry({
              grab_id: 3,
              media_title: 'Film 0002',
              download_state: 'paused',
              progress: 0.1,
              remaining_bytes: null,
              remaining_seconds: null,
              status: 'downloading',
            }),
          ],
          notice: null,
          as_of: '2026-11-12T09:30:00Z',
        },
        activity: [entry({ grab_id: 4 })],
        problems: [
          entry({
            grab_id: 5,
            media_title: 'Film 0005',
            status: 'completed',
            message: 'Import fehlgeschlagen: zu wenig freier Speicher',
          }),
        ],
        upcoming: [],
      });
    });
    await page.goto(`${LIBRARY}/`);
    const items = page.getByTestId('queue-item');
    await expect(items).toHaveCount(2);
    await expect(items.first()).toContainText('50 %');
    await expect(items.first()).toContainText('7,5 GB');
    await expect(items.first().getByTestId('remaining')).toHaveText('1 Std. 5 Min.');
    await expect(items.nth(1).getByTestId('remaining')).toHaveText('–');
    await expect(items.nth(1)).toContainText('pausiert');
    const activity = page.getByRole('region', { name: 'Letzte Aktivität' });
    await expect(activity).toContainText('importiert');
    const problems = page.getByRole('region', { name: 'Probleme der letzten 7 Tage' });
    await expect(problems).toContainText('Import fehlgeschlagen: zu wenig freier Speicher');
    expect(calls).toBe(1);
    await page.clock.runFor(15_000);
    await expect.poll(() => calls).toBe(2);
  });
});

test.describe('calendar (AC-07)', () => {
  test('starts at the current week with one entry per date, marked by state', async ({
    page,
    request,
  }) => {
    const week = rangeOf('woche', today());
    const answer = (await (
      await request.get(`${LIBRARY}/api/calendar?from=${week.from}&to=${week.to}`)
    ).json()) as CalendarAnswer;
    await page.goto(`${LIBRARY}/kalender`);
    await expect(page.getByTestId('range')).toContainText('KW');
    const shown = page.getByTestId('calendar-entry');
    await expect(shown).toHaveCount(answer.entries.length);
    for (const [i, e] of answer.entries.entries()) {
      await expect(shown.nth(i)).toHaveAttribute('data-state', e.state);
    }
    const cinema = page
      .locator(`[data-day="${today()}"]`)
      .getByTestId('calendar-entry')
      .filter({ hasText: 'Kalenderfilm' });
    await expect(cinema).toContainText('Kino');
  });

  test('a movie with cinema, digital and disc dates appears three times in its month', async ({
    page,
    request,
  }) => {
    const answer = (await (
      await request.get(`${LIBRARY}/api/calendar?from=${today()}&to=${today()}`)
    ).json()) as CalendarAnswer;
    expect(answer.entries.some((e) => e.title === 'Kalenderfilm')).toBe(true);
    let found = 0;
    // Disc and digital dates may fall into the next month: count over two months.
    const shown = () =>
      page.waitForFunction(
        () => document.querySelector('ol.days, [data-testid="calendar-note"]') !== null,
      );
    const loaded = page.waitForResponse((r) => r.url().includes('/api/calendar?'));
    await page.goto(`${LIBRARY}/kalender?ansicht=monat`);
    await loaded;
    await shown();
    found += await page.getByTestId('calendar-entry').filter({ hasText: 'Kalenderfilm' }).count();
    const next = page.waitForResponse((r) => r.url().includes('/api/calendar?'));
    await page.getByRole('button', { name: 'Nächster Monat' }).click();
    await next;
    await shown();
    found += await page.getByTestId('calendar-entry').filter({ hasText: 'Kalenderfilm' }).count();
    expect(found).toBe(3);
  });

  test('steps back and forth; an empty month shows a German note', async ({ page }) => {
    await page.goto(`${LIBRARY}/kalender`);
    const title = await page.getByTestId('range').textContent();
    await page.getByRole('button', { name: 'Nächste Woche' }).click();
    await expect(page.getByTestId('range')).not.toHaveText(title ?? '');
    await expect(page).toHaveURL(/datum=/);
    await page.getByRole('button', { name: 'Vorherige Woche' }).click();
    await expect(page.getByTestId('range')).toHaveText(title ?? '');
    await page.goto(`${LIBRARY}/kalender?ansicht=monat&datum=2090-02-01`);
    await expect(page.getByTestId('range')).toHaveText('Februar 2090');
    await expect(page.getByTestId('calendar-note')).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
  });
});

test.describe('missing page (AC-08, AC-09)', () => {
  test('lists missing and cutoff-unmet items in two tabs with profile and reason', async ({
    page,
    request,
  }) => {
    await page.goto(`${LIBRARY}/fehlend`);
    const missing = await apiTotal(request, '/api/library/wanted?list=missing&media=movies');
    await expect(page.getByTestId('wanted-total')).toHaveText(`${missing} Filme`);
    await expect(page.getByTestId('wanted-item').filter({ hasText: 'Fehlt Sehr' })).toContainText(
      'Profil 1080p',
    );
    await expect(page.getByTestId('reason').first()).toHaveText('fehlt');

    await page.getByRole('tab', { name: 'Cutoff nicht erreicht' }).click();
    await expect(page).toHaveURL(/liste=cutoff/);
    const cutoff = await apiTotal(request, '/api/library/wanted?list=cutoff_unmet&media=movies');
    await expect(page.getByTestId('wanted-total')).toHaveText(`${cutoff} Filme`);
    await expect(page.getByTestId('reason').first()).not.toHaveText('fehlt');

    await page.getByRole('button', { name: 'Episoden' }).click();
    const episodes = await apiTotal(
      request,
      '/api/library/wanted?list=cutoff_unmet&media=episodes',
    );
    await expect(page.getByTestId('wanted-total')).toHaveText(
      `${episodes} ${episodes === 1 ? 'Episode' : 'Episoden'}`,
    );
    await expect(page.getByTestId('wanted-item').first()).toContainText('Kleine Insel S01E01');
  });

  test('searches one item, the selected ones and all of the tab', async ({ page }) => {
    const bodies: unknown[] = [];
    await page.route('**/api/command/search-bulk', async (route) => {
      bodies.push(route.request().postDataJSON());
      await route.continue();
    });
    await page.goto(`${LIBRARY}/fehlend?art=episoden`);
    const row = page.getByTestId('wanted-item').filter({ hasText: 'Kleine Insel S01E02' });
    await row.getByRole('button', { name: /suchen/ }).click();
    await expect(page.getByText(/Kleine Insel S01E02.*: Suche/)).toBeVisible();

    await page.goto(`${LIBRARY}/fehlend`);
    const rows = page.getByTestId('wanted-item');
    await rows.nth(0).getByRole('checkbox').check();
    await rows.nth(1).getByRole('checkbox').check();
    await page.getByRole('button', { name: 'Ausgewählte suchen (2)' }).click();
    await expect(page.getByText(/^Ausgewählte: Suche beendet: 2 Suchen/)).toBeVisible();
    await page.getByRole('button', { name: /^Alle \d+ suchen$/ }).click();
    await expect(page.getByText(/^Alle: Suche beendet:/)).toBeVisible();
    expect(bodies).toEqual([
      { items: [expect.any(Object), expect.any(Object)], all: null },
      { items: null, all: { list: 'missing', media: 'movies' } },
    ]);
  });
});

const blocked = (id: number) => ({
  id,
  movie_id: 1,
  series_id: null,
  media_title: 'Film 0001',
  title: `Film.0001.Release.${id}-GRP`,
  indexer: 'Testindexer',
  reason: 'Download fehlgeschlagen',
  blocklisted_at: '2026-11-10T08:00:00Z',
});

test.describe('blocklist (AC-18)', () => {
  test('lists the entries page by page and removes one after confirmation', async ({ page }) => {
    const deleted: string[] = [];
    await page.route('**/api/blocklist?*', (route) => {
      const pageNumber = Number(new URL(route.request().url()).searchParams.get('page'));
      const entries =
        pageNumber === 1
          ? Array.from({ length: 50 }, (_, i) => blocked(i + 1))
          : [blocked(51), blocked(52)];
      return json(route, { page: pageNumber, page_size: 50, total: 52, entries });
    });
    await page.route('**/api/blocklist/*', (route) => {
      deleted.push(route.request().method() + ' ' + new URL(route.request().url()).pathname);
      return route.fulfill({ status: 204 });
    });
    await page.goto(`${LIBRARY}/blocklist`);
    await expect(page.getByTestId('blocklist-entry')).toHaveCount(50);
    const first = page.getByTestId('blocklist-entry').first();
    await expect(first).toContainText('Film 0001');
    await expect(first).toContainText('Film.0001.Release.1-GRP');
    await expect(first).toContainText('Download fehlgeschlagen');
    await expect(first).toContainText('10.11.2026');
    await page.getByRole('button', { name: 'Weiter' }).click();
    await expect(page).toHaveURL(/seite=2/);
    await expect(page.getByTestId('blocklist-entry')).toHaveCount(2);
    await page
      .getByRole('button', { name: '„Film.0001.Release.52-GRP“ von der Blocklist entfernen' })
      .click();
    const dialog = page.getByRole('dialog', { name: 'Von der Blocklist entfernen' });
    await expect(dialog).toContainText('Film.0001.Release.52-GRP');
    expect(deleted).toEqual([]);
    await dialog.getByRole('button', { name: 'Entfernen' }).click();
    await expect(
      page.getByText('„Film.0001.Release.52-GRP“ ist nicht mehr gesperrt.'),
    ).toBeVisible();
    expect(deleted).toEqual(['DELETE /api/blocklist/52']);
  });

  test('an empty blocklist says so', async ({ page }) => {
    await page.goto(`${LIBRARY}/blocklist`);
    await expect(page.getByText('Die Blocklist ist leer.')).toBeVisible();
  });
});

test.describe('backups (AC-19)', () => {
  test.describe.configure({ mode: 'serial' });

  test('create, download, choose for restore with its time, cancel, delete', async ({ page }) => {
    await page.goto(`${LIBRARY}/backups`);
    const rows = page.getByTestId('backup');
    await expect(rows.first()).toBeVisible();
    const before = await rows.count();
    await page.getByRole('button', { name: 'Sicherung erstellen' }).click();
    await expect(page.getByText(/^Sicherung „.*“ erstellt\.$/)).toBeVisible();
    await expect(rows).toHaveCount(before + 1);
    const row = rows.filter({ hasText: 'manuell' }).first();
    const name = (await row.locator('.name').textContent()) ?? '';

    const download = page.waitForEvent('download');
    await row.getByRole('link', { name: `„${name}“ herunterladen` }).click();
    expect((await download).suggestedFilename()).toBe(name);

    await row.getByRole('button', { name: 'Wiederherstellen …' }).click();
    const dialog = page.getByRole('dialog', { name: 'Sicherung einspielen' });
    await expect(dialog).toContainText(/Die Sicherung vom \d{2}\.\d{2}\.\d{4}, \d{2}:\d{2}:\d{2}/);
    await dialog.getByRole('button', { name: 'Beim nächsten Start einspielen' }).click();
    await expect(page.getByTestId('pending-restore')).toContainText(name);
    await page.getByRole('button', { name: 'Wiederherstellung abbrechen' }).click();
    await expect(page.getByTestId('pending-restore')).toHaveCount(0);

    await row.getByRole('button', { name: `„${name}“ löschen` }).click();
    await page
      .getByRole('dialog', { name: 'Sicherung löschen' })
      .getByRole('button', { name: 'Löschen' })
      .click();
    await expect(page.getByText(`„${name}“ gelöscht.`)).toBeVisible();
    await expect(rows).toHaveCount(before);
  });

  test('the last backup error is shown', async ({ page }) => {
    await page.route('**/api/backups', (route) =>
      json(route, {
        backups: [],
        pending_restore: null,
        last_error: {
          time: '2026-11-12T03:00:00Z',
          message: 'Sicherungsordner nicht beschreibbar',
        },
      }),
    );
    await page.goto(`${LIBRARY}/backups`);
    await expect(page.getByTestId('backup-error')).toContainText(
      'Sicherungsordner nicht beschreibbar',
    );
    await expect(page.getByText('Es gibt noch keine Sicherung.')).toBeVisible();
  });
});
