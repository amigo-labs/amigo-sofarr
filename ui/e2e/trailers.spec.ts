// spec trailers AC-09, AC-10, AC-12 in the browser. The `extras` server has no TMDB, so it
// refuses downloads; states that need TMDB and yt-dlp come as simulated API answers (the
// server side is covered by the Rust tests of spec trailers).

import { expect, test, type Page } from '@playwright/test';
import type { TrailerInfo } from '../src/lib/api/generated/TrailerInfo';
import { SAMPLE_SYSTEM, expectSystemPage, json } from './helpers';
import { EXTRAS } from './extras-helpers';
import { LIBRARY, movieId, seriesId } from './library-helpers';

const NONE: TrailerInfo = {
  status: 'waiting',
  files: [],
  language: null,
  link: null,
  message: 'Wartet auf den täglichen Trailer-Lauf und sein Tagesbudget',
  attempts: 0,
  next_attempt: null,
  blocked: null,
};

const DOWNLOADED: TrailerInfo = {
  ...NONE,
  status: 'downloaded',
  files: ['trailers/Trailerfilm (2019) - Trailer (Deutsch).mkv'],
  language: 'Deutsch',
  link: 'https://www.youtube.com/watch?v=abcdefghijk',
  message: null,
  attempts: 1,
};

const box = (page: Page) => page.getByTestId('trailer');

test.describe('trailer state on the pages (AC-10)', () => {
  test('a trailer someone else put there, and why sofarr downloads none (AC-09)', async ({
    page,
    request,
  }) => {
    await page.goto(`${EXTRAS}/filme/${await movieId(request, EXTRAS, 'Trailerfilm')}`);
    await expect(box(page)).toContainText('vorhanden, nicht von sofarr');
    await expect(box(page)).toContainText('trailers/Mein Trailer.mkv');
    const button = box(page).getByRole('button', { name: 'Trailer laden' });
    await expect(button).toBeDisabled();
    await expect(button).toHaveAccessibleDescription(
      'Keine Trailer-Downloads: [tmdb] fehlt in der Konfiguration',
    );
  });

  test('a movie without a file and a series without TMDB id say why there is none', async ({
    page,
    request,
  }) => {
    await page.goto(`${EXTRAS}/filme/${await movieId(request, EXTRAS, 'Ohne Datei')}`);
    await expect(box(page)).toContainText('kein automatischer Download');
    await expect(page.getByTestId('trailer-message')).toHaveText(
      'Keine Videodatei in der Bibliothek – ein Trailer nur von Hand',
    );
    await page.goto(`${EXTRAS}/serien/${await seriesId(request, EXTRAS, 'Untertitelserie')}`);
    await expect(page.getByTestId('trailer-message')).toHaveText(
      'Die Serie hat noch keine TMDB-ID',
    );
  });

  test('every other reason reads in German, with attempts and the next one', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, EXTRAS, 'Trailerfilm');
    let answer: TrailerInfo = NONE;
    await page.route(`**/api/movies/${id}/trailer`, (route) => json(route, answer));
    const cases: readonly [TrailerInfo, string][] = [
      [NONE, 'wartet'],
      [
        {
          ...NONE,
          status: 'no_candidate',
          message: 'TMDB listet keinen passenden Trailer auf YouTube',
          attempts: 2,
          next_attempt: '2026-11-19T08:00:00Z',
        },
        'kein Trailer gefunden',
      ],
      [
        {
          ...NONE,
          status: 'failed',
          message: 'yt-dlp: Video nicht verfügbar – nach 4 Versuchen nur noch von Hand',
          attempts: 4,
        },
        'fehlgeschlagen',
      ],
      [
        {
          ...NONE,
          status: 'low_space',
          message: 'zu wenig Speicherplatz: 5 MB frei, verlangt sind 100 MB',
        },
        'zu wenig Speicherplatz',
      ],
    ];
    for (const [info, label] of cases) {
      answer = info;
      await page.goto(`${EXTRAS}/filme/${id}`);
      await expect(box(page).locator('.label')).toHaveText(label);
      await expect(page.getByTestId('trailer-message')).toHaveText(info.message ?? '');
      await expect(box(page).getByRole('button', { name: 'Trailer laden' })).toBeEnabled();
      if (info.attempts > 0) {
        await expect(box(page)).toContainText(`Versuche ${info.attempts} von 4`);
      }
    }
    answer = cases[1]?.[0] ?? NONE;
    await page.goto(`${EXTRAS}/filme/${id}`);
    await expect(box(page)).toContainText(/Versuche\s*2 von 4/);
    await expect(box(page)).toContainText(/Nächster Versuch\s*19\.11\.2026, 09:00:00/);
  });
});

test.describe('"Trailer laden" (AC-09)', () => {
  test('downloads at once and shows the trailer with language and source', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, EXTRAS, 'Trailerfilm');
    let answer: TrailerInfo = { ...NONE, status: 'failed', message: 'Download fehlgeschlagen' };
    await page.route(`**/api/movies/${id}/trailer`, (route) => {
      if (route.request().method() === 'POST') answer = DOWNLOADED;
      return json(route, answer);
    });
    await page.goto(`${EXTRAS}/filme/${id}`);
    await box(page).getByRole('button', { name: 'Trailer laden' }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Trailer für' })).toHaveText(
      /Trailer für „Trailerfilm“ geladen \(Deutsch\)\./,
    );
    await expect(box(page).locator('.label')).toHaveText('vorhanden');
    await expect(page.getByTestId('trailer-language')).toHaveText('Deutsch');
    await expect(box(page).getByRole('link', { name: 'Trailer auf YouTube' })).toHaveAttribute(
      'href',
      'https://www.youtube.com/watch?v=abcdefghijk',
    );
    await expect(box(page)).toContainText('trailers/Trailerfilm (2019) - Trailer (Deutsch).mkv');
    await expect(box(page).getByRole('button', { name: 'Trailer neu laden' })).toBeEnabled();
  });

  test('a series trailer that fails reports the reason', async ({ page, request }) => {
    const id = await seriesId(request, EXTRAS, 'Untertitelserie');
    const failed: TrailerInfo = {
      ...NONE,
      status: 'failed',
      message: 'yt-dlp: Video ist privat',
      attempts: 1,
      next_attempt: '2026-11-19T08:00:00Z',
    };
    let posted = false;
    await page.route(`**/api/series/${id}/trailer`, (route) => {
      if (route.request().method() === 'POST') posted = true;
      return json(route, posted ? failed : NONE);
    });
    await page.goto(`${EXTRAS}/serien/${id}`);
    await box(page).getByRole('button', { name: 'Trailer laden' }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Kein Trailer' })).toHaveText(
      /Kein Trailer für „Untertitelserie“ geladen: yt-dlp: Video ist privat/,
    );
    await expect(box(page).locator('.label')).toHaveText('fehlgeschlagen');
  });

  test('a refusal shows the German message', async ({ page, request }) => {
    const id = await movieId(request, EXTRAS, 'Trailerfilm');
    await page.route(`**/api/movies/${id}/trailer`, (route) =>
      route.request().method() === 'POST'
        ? json(route, { error: 'Für diesen Eintrag läuft schon ein Trailer-Download' }, 409)
        : json(route, NONE),
    );
    await page.goto(`${EXTRAS}/filme/${id}`);
    await box(page).getByRole('button', { name: 'Trailer laden' }).click();
    await expect(page.getByRole('alert')).toContainText(
      'Trailer nicht geladen: Für diesen Eintrag läuft schon ein Trailer-Download',
    );
  });
});

test.describe('system page (AC-12)', () => {
  test('says why trailers are not downloaded, or that they are', async ({ page }) => {
    await page.goto(`${EXTRAS}/system`);
    await expectSystemPage(page);
    await expect(page.getByTestId('trailers')).toHaveText(
      'keine Downloads – [tmdb] fehlt in der Konfiguration',
    );
    await page.goto(`${LIBRARY}/system`);
    await expectSystemPage(page);
    await expect(page.getByTestId('trailers')).toHaveText(
      'keine Downloads – Schattenmodus: sofarr lädt keine Trailer',
    );
    await page.route('**/api/system', (route) =>
      json(route, { ...SAMPLE_SYSTEM, trailers: { active: true, message: null } }),
    );
    await page.reload();
    await expectSystemPage(page);
    await expect(page.getByTestId('trailers')).toHaveText('werden heruntergeladen');
  });
});
