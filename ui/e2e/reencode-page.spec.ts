// spec ui-reencode in the browser. The `library` server knows no oversized file and runs
// without `[reencode] enabled`; a page with candidates, a running encode and a history
// comes as a simulated API answer (the server side is covered by the Rust tests of spec
// ui-reencode).

import { expect, test, type Page } from '@playwright/test';
import type { PageCandidate } from '../src/lib/api/generated/PageCandidate';
import type { ReencodePage } from '../src/lib/api/generated/ReencodePage';
import { json } from './helpers';
import { LIBRARY } from './library-helpers';

const GB = 1024 ** 3;

const HEAT: PageCandidate = {
  kind: 'movie',
  movie_id: 4,
  series_id: null,
  episode_ids: [],
  title: 'Heat (1995)',
  path: '/filme/Heat/Heat.mkv',
  size: 60 * GB,
  mb_per_minute: 360.4,
  threshold: 60,
  resolution: '1080p',
  video_codec: 'h264',
  state: 'due',
  rests_until: null,
};

const ISLAND: PageCandidate = {
  kind: 'episode',
  movie_id: null,
  series_id: 7,
  episode_ids: [21, 22],
  title: 'Insel – S01E02-E03',
  path: '/serien/Insel/S01E02.mkv',
  size: 10 * GB,
  mb_per_minute: 113.8,
  threshold: 60,
  resolution: '1080p',
  video_codec: 'h264',
  state: 'excluded',
  rests_until: null,
};

const PAGE: ReencodePage = {
  job: { active: true, message: null, running: null },
  running: null,
  saved_files: 2,
  saved_bytes: 50 * GB,
  count: 2,
  total_size: 70 * GB,
  excluded: { no_media_info: 1, resolution_off: 0, unknown_duration: 3, dynamic_metadata: 0 },
  candidates: [HEAT, ISLAND],
  history: [
    {
      title: 'Alien (1979)',
      started_at: '2026-11-17T22:00:00Z',
      runner: 'server',
      outcome: 'replaced',
      reason: null,
      size_before: 40 * GB,
      size_after: 10 * GB,
    },
    {
      title: 'Ran (1985)',
      started_at: '2026-11-17T20:00:00Z',
      runner: 'cli',
      outcome: 'failed',
      reason: 'ffmpeg: Conversion failed!',
      size_before: 30 * GB,
      size_after: null,
    },
  ],
  shadow: false,
};

const row = (page: Page, title: string) => page.locator(`[data-candidate="${title}"]`);

async function open(page: Page, answer: ReencodePage): Promise<void> {
  await page.route('**/api/reencode', (route) => json(route, answer));
  await page.goto(`${LIBRARY}/neu-encodieren`);
  await expect(page.getByRole('heading', { level: 1, name: 'Neu encodieren' })).toBeVisible();
}

test.describe('page "Neu encodieren" (ui-reencode)', () => {
  test('is in the navigation and shows the empty states (AC-01, AC-07)', async ({ page }) => {
    await page.goto(`${LIBRARY}/`);
    await page.getByRole('link', { name: 'Neu encodieren' }).first().click();
    await expect(page).toHaveURL(/\/neu-encodieren$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Neu encodieren' })).toBeVisible();
    await expect(page.getByTestId('reencode-job')).toHaveText(
      'aus – in der Konfiguration ausgeschaltet',
    );
    await expect(page.getByTestId('no-candidates')).toHaveText('Keine Datei ist zu groß.');
    await expect(page.getByTestId('history')).toContainText('Noch nichts encodiert.');
    await expect(page.getByTestId('reencode-saved')).toHaveText('0 Dateien, 0,0 GB gespart');
  });

  test('shows job, saved space, candidates with their state, and the history (AC-02, AC-03, AC-06)', async ({
    page,
  }) => {
    await open(page, {
      ...PAGE,
      job: { active: true, message: null, running: 'Ran (1985)' },
      running: {
        title: 'Ran (1985)',
        path: '/filme/Ran/Ran.mkv',
        started_at: '2026-11-18T06:00:00Z',
        runner: 'cli',
        attempt: null,
      },
    });
    await expect(page.getByTestId('reencode-job')).toHaveText('aktiv – encodiert Ran (1985)');
    await expect(page.getByTestId('reencode-running')).toContainText('Ran (1985) – seit');
    await expect(page.getByTestId('reencode-running')).toContainText('(CLI)');
    await expect(page.getByTestId('reencode-saved')).toHaveText('2 Dateien, 50,0 GB gespart');
    await expect(page.getByTestId('candidates-summary')).toHaveText('2 Dateien, 70,0 GB');
    await expect(page.getByTestId('left-out')).toHaveText([
      '1 × keine Media-Infos',
      '3 × Laufzeit unbekannt',
    ]);
    const heat = row(page, 'Heat (1995)');
    await expect(heat.getByRole('link', { name: 'Heat (1995)' })).toHaveAttribute(
      'href',
      '/filme/4',
    );
    await expect(heat).toContainText('60,0 GB · 360,4 > 60,0 MB/min · 1080p · h264');
    await expect(heat.getByTestId('state')).toHaveText('fällig');
    const island = row(page, 'Insel – S01E02-E03');
    await expect(island.getByRole('link')).toHaveAttribute('href', '/serien/7');
    await expect(island.getByTestId('state')).toHaveText('ausgeschlossen');
    await expect(island.getByRole('button', { name: /Wieder aufnehmen/ })).toBeVisible();
    // Another encode runs: "Jetzt encodieren" is disabled and says why.
    await expect(heat.getByRole('button', { name: /Jetzt encodieren/ })).toBeDisabled();
    await expect(heat.getByTestId('blocked')).toHaveText('es läuft bereits ein Encode: Ran (1985)');
    const history = page.getByTestId('history');
    await expect(history.locator('[data-attempt="0"]')).toContainText('ersetzt');
    await expect(history.locator('[data-attempt="0"]')).toContainText(
      '40,0 GB → 10,0 GB (75 % kleiner)',
    );
    await expect(history.locator('[data-attempt="0"]')).toContainText('Server');
    await expect(history.locator('[data-attempt="1"]')).toContainText(
      'fehlgeschlagen: ffmpeg: Conversion failed!',
    );
    await expect(history.locator('[data-attempt="1"]')).toContainText('CLI');
  });

  test('starts an encode and keeps a file out of the job (AC-04, AC-05)', async ({ page }) => {
    const sent: { method: string; url: string; body: unknown }[] = [];
    await page.route('**/api/reencode/encode', (route) => {
      sent.push({
        method: route.request().method(),
        url: route.request().url(),
        body: route.request().postDataJSON(),
      });
      return json(route, { title: 'Heat (1995)', path: '/filme/Heat/Heat.mkv' }, 202);
    });
    await page.route('**/api/reencode/exclusions', (route) => {
      sent.push({
        method: route.request().method(),
        url: route.request().url(),
        body: route.request().postDataJSON(),
      });
      return route.fulfill({ status: 204 });
    });
    await open(page, PAGE);
    await row(page, 'Heat (1995)')
      .getByRole('button', { name: /Jetzt encodieren/ })
      .click();
    await expect(page.getByRole('status').or(page.getByRole('alert')).first()).toContainText(
      'Encode gestartet: Heat (1995)',
    );
    await row(page, 'Heat (1995)')
      .getByRole('button', { name: /Ausschließen/ })
      .click();
    await row(page, 'Insel – S01E02-E03')
      .getByRole('button', { name: /Wieder aufnehmen/ })
      .click();
    await expect.poll(() => sent.length).toBe(3);
    expect(sent.map((s) => [s.method, s.body])).toEqual([
      ['POST', { movie_id: 4 }],
      ['POST', { movie_id: 4 }],
      ['DELETE', { episode_id: 21 }],
    ]);
    // A refusal shows its German reason.
    await page.unroute('**/api/reencode/encode');
    await page.route('**/api/reencode/encode', (route) =>
      json(route, { error: 'zu wenig Speicherplatz: 10 MB frei, verlangt sind 50000 MB' }, 409),
    );
    await row(page, 'Heat (1995)')
      .getByRole('button', { name: /Jetzt encodieren/ })
      .click();
    await expect(page.getByRole('alert').first()).toContainText(
      'zu wenig Speicherplatz: 10 MB frei, verlangt sind 50000 MB',
    );
  });

  test('is read-only in shadow mode (AC-08)', async ({ page }) => {
    await open(page, { ...PAGE, shadow: true });
    const heat = row(page, 'Heat (1995)');
    await expect(heat.getByRole('button', { name: /Jetzt encodieren/ })).toBeDisabled();
    await expect(heat.getByRole('button', { name: /Ausschließen/ })).toBeDisabled();
    await expect(
      row(page, 'Insel – S01E02-E03').getByRole('button', { name: /Wieder aufnehmen/ }),
    ).toBeDisabled();
    await expect(heat.getByTestId('blocked')).toHaveText('im Shadow-Mode');
  });

  test('follows the state without a reload and says when sofarr is gone (AC-09)', async ({
    page,
  }) => {
    await page.clock.install();
    let answer: ReencodePage = PAGE;
    let reachable = true;
    await page.route('**/api/reencode', (route) =>
      reachable ? json(route, answer) : route.abort('connectionrefused'),
    );
    await page.goto(`${LIBRARY}/neu-encodieren`);
    await expect(row(page, 'Heat (1995)').getByTestId('state')).toHaveText('fällig');
    answer = { ...PAGE, candidates: [{ ...HEAT, state: 'done' }, ISLAND] };
    await page.clock.runFor(16_000);
    await expect(row(page, 'Heat (1995)').getByTestId('state')).toHaveText('erledigt');
    reachable = false;
    await page.clock.runFor(16_000);
    await expect(page.getByTestId('reencode-stale')).toContainText('sofarr ist nicht erreichbar');
    await expect(page.getByTestId('reencode-stale')).toContainText('Stand von');
    await expect(row(page, 'Heat (1995)').getByTestId('state')).toHaveText('erledigt');
  });
});
