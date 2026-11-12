// spec subtitles AC-11, AC-12 (with AC-13 and AC-15 as the pages show them) in the browser.
// The `extras` server searches a fake OpenSubtitles.com at start (e2e/seed-extras.mjs):
// "Untertitelfilm" gets a German subtitle, "Spätsuche" an English one only by hand.

import { expect, test } from '@playwright/test';
import type { FileSubtitles } from '../src/lib/api/generated/FileSubtitles';
import type { SubtitleOverview } from '../src/lib/api/generated/SubtitleOverview';
import { json } from './helpers';
import { EXTRAS, RATINGS, waitForSubtitleRun } from './extras-helpers';
import { movieId, seriesId } from './library-helpers';

const DATE = /\d{2}\.\d{2}\.\d{4}, \d{2}:\d{2}:\d{2}/;

test.describe('subtitle overview (AC-11)', () => {
  test.beforeEach(async ({ request }) => {
    await waitForSubtitleRun(request);
  });

  test('lists needed subtitles with item, language, kind, last and next search', async ({
    page,
    request,
  }) => {
    await page.goto(`${EXTRAS}/untertitel`);
    await expect(page.getByRole('heading', { level: 1, name: 'Untertitel' })).toBeVisible();
    const nav = page.getByRole('navigation', { name: 'Hauptnavigation' });
    await expect(nav.getByRole('link', { name: 'Untertitel' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    const needed = page.getByTestId('needed');
    const film = needed.locator('li[data-file]').filter({ hasText: 'Untertitelfilm (2020)' });
    const id = await movieId(request, EXTRAS, 'Untertitelfilm');
    await expect(film.getByRole('link', { name: 'Untertitelfilm (2020)' })).toHaveAttribute(
      'href',
      `/filme/${id}`,
    );
    const english = film.locator('[data-need="en-normal"]');
    await expect(english).toContainText('Englisch · normal');
    await expect(english).toContainText(new RegExp(`zuletzt gesucht: ${DATE.source}`));
    await expect(english).toContainText(new RegExp(`nächste Suche: ${DATE.source}`));
    await expect(film.locator('[data-need="de-normal"]')).toHaveCount(0);

    const foreign = needed.locator('li[data-file]').filter({ hasText: 'Fremduntertitel (2021)' });
    await expect(foreign.locator('[data-need]')).toHaveCount(1);
    await expect(foreign).toContainText('Englisch · normal');

    const episode = needed.locator('li[data-file]').filter({ hasText: 'Untertitelserie – S01E01' });
    const series = await seriesId(request, EXTRAS, 'Untertitelserie');
    await expect(episode.getByRole('link')).toHaveAttribute('href', `/serien/${series}`);
    await expect(episode.locator('[data-need]')).toHaveText([
      /Deutsch · normal/,
      /Englisch · normal/,
    ]);
    await expect(
      episode.getByRole('button', { name: 'Jetzt suchen für „Untertitelserie – S01E01“' }),
    ).toBeEnabled();
  });

  test('shows the subtitles found in the last 7 days', async ({ page }) => {
    await page.goto(`${EXTRAS}/untertitel`);
    const found = page
      .getByTestId('recent')
      .locator('li')
      .filter({ hasText: 'Untertitelfilm (2020).de.srt' });
    await expect(found.getByRole('link', { name: 'Untertitelfilm (2020)' })).toBeVisible();
    await expect(found).toContainText('Deutsch · normal');
    await expect(found).toContainText('Release: Untertitelfilm.2020.German.1080p.BluRay.x264-GRP');
    await expect(found).toContainText('Treffer: nur Titel');
    await expect(found).toContainText(DATE);
  });

  test('a search for one file at once loads what the provider has now', async ({ page }) => {
    await page.goto(`${EXTRAS}/untertitel`);
    const late = page
      .getByTestId('needed')
      .locator('li[data-file]')
      .filter({ hasText: 'Spätsuche' });
    await expect(late.locator('[data-need]')).toHaveCount(2);
    await late.getByRole('button', { name: 'Jetzt suchen für „Spätsuche (2022)“' }).click();
    await expect(page.getByRole('status').filter({ hasText: '„Spätsuche (2022)“' })).toHaveText(
      /„Spätsuche \(2022\)“: 2 Untertitel gesucht, 1 geladen \(Spätsuche \(2022\)\.en\.srt\), 1 nicht gefunden\./,
    );
    await expect(late.locator('[data-need]')).toHaveText([/Deutsch · normal/]);
    await expect(page.getByTestId('recent')).toContainText('Spätsuche (2022).en.srt');
  });

  test('shadow mode, a used-up quota and a failing provider read in German (AC-13)', async ({
    page,
  }) => {
    const overview: SubtitleOverview = {
      enabled: true,
      reason: null,
      shadow: true,
      quota_until: '2026-11-13T00:00:00Z',
      needed: [
        {
          item: {
            kind: 'movie',
            movie_id: 9,
            series_id: null,
            episode_id: null,
            title: 'X (2019)',
          },
          language: 'de',
          kind: 'forced',
          last_search: null,
          next_search: null,
          would_download: 'X.2019.German.1080p.BluRay-GRP',
        },
      ],
      recent: [],
    };
    await page.route('**/api/subtitles', (route) => json(route, overview));
    await page.route('**/api/movies/9/subtitles/search', (route) =>
      json(route, { error: 'OpenSubtitles antwortet mit HTTP 503' }, 502),
    );
    await page.goto(`${EXTRAS}/untertitel`);
    await expect(page.getByText(/^Schattenmodus: sofarr sucht Untertitel/)).toBeVisible();
    await expect(page.getByTestId('subtitles-quota')).toContainText(
      'aufgebraucht; sofarr lädt ab 13.11.2026, 01:00:00 weiter.',
    );
    const need = page.locator('[data-need="de-forced"]');
    await expect(need).toContainText('Deutsch · erzwungen');
    await expect(need).toContainText('zuletzt gesucht: noch nie');
    await expect(need).toContainText('nächste Suche: beim nächsten Lauf');
    await expect(need).toContainText('würde laden: X.2019.German.1080p.BluRay-GRP');
    await expect(page.getByTestId('recent')).toContainText(
      'In den letzten 7 Tagen wurden keine Untertitel gefunden.',
    );
    await page.getByRole('button', { name: 'Jetzt suchen für „X (2019)“' }).click();
    await expect(page.getByRole('alert')).toContainText(
      'Untertitelsuche für „X (2019)“: OpenSubtitles antwortet mit HTTP 503',
    );
  });

  test('switched off, the overview says why and offers no search (AC-15)', async ({ page }) => {
    await page.goto(`${RATINGS}/untertitel`);
    await expect(page.getByTestId('subtitles-off')).toHaveText(
      'Keine Untertitelsuche: Kein OpenSubtitles-Zugang eingetragen ([opensubtitles] fehlt in der Konfiguration)',
    );
    await expect(page.getByRole('button', { name: /Jetzt suchen/ })).toHaveCount(0);
  });
});

test.describe('subtitles of a file (AC-12)', () => {
  test.beforeEach(async ({ request }) => {
    await waitForSubtitleRun(request);
  });

  test('a movie file shows its own and foreign subtitle files and what is still needed', async ({
    page,
    request,
  }) => {
    await page.goto(`${EXTRAS}/filme/${await movieId(request, EXTRAS, 'Untertitelfilm')}`);
    await expect(page.getByTestId('subtitles-embedded')).toHaveText(
      'unbekannt – die Media-Infos der Datei sind noch nicht gelesen',
    );
    const external = page.getByTestId('subtitles-external');
    await expect(external).toContainText('Deutsch · normal');
    await expect(external).toContainText('von sofarr');
    await expect(external).toContainText('Treffer: nur Titel');
    await expect(external).toContainText('Untertitelfilm (2020).de.srt');
    await expect(page.getByTestId('subtitles-needed')).toContainText('Englisch · normal');
    await expect(page.getByRole('button', { name: 'Untertitelsuche starten' })).toBeEnabled();

    await page.goto(`${EXTRAS}/filme/${await movieId(request, EXTRAS, 'Fremduntertitel')}`);
    await expect(page.getByTestId('subtitles-external')).toContainText('Deutsch · normal');
    await expect(page.getByTestId('subtitles-external')).toContainText('fremd');
    await expect(page.getByTestId('subtitles-external')).toContainText(
      'Fremduntertitel (2021).de.srt',
    );
    await expect(page.getByTestId('subtitles-needed')).not.toContainText('Deutsch');

    await page.goto(`${EXTRAS}/filme/${await movieId(request, EXTRAS, 'Ohne Datei')}`);
    await expect(page.getByText('Ohne Datei gibt es keine Untertitel.')).toBeVisible();
  });

  test('embedded tracks show language, kind and format', async ({ page, request }) => {
    const subtitles: FileSubtitles = {
      enabled: true,
      embedded: [
        { language: 'de', forced: true, hearing_impaired: false, format: 'SubRip' },
        { language: 'en', forced: false, hearing_impaired: true, format: 'PGS' },
        { language: 'und', forced: false, hearing_impaired: false, format: 'ASS' },
      ],
      external: [],
      needed: [],
    };
    const id = await movieId(request, EXTRAS, 'Trailerfilm');
    await page.route(`**/api/movies/${id}/subtitles`, (route) => json(route, subtitles));
    await page.goto(`${EXTRAS}/filme/${id}`);
    await expect(page.getByTestId('subtitles-embedded').locator('li')).toHaveText([
      /Deutsch · erzwungen · SubRip/,
      /Englisch · normal, für Hörgeschädigte · PGS/,
      /Sprache unbekannt · normal · ASS/,
    ]);
    await expect(page.getByTestId('subtitles-external')).toHaveText('keine');
    await expect(page.getByTestId('subtitles-needed')).toHaveText(
      'keine – jede gewünschte Sprache ist vorhanden',
    );
  });

  test('an episode file shows its subtitles when opened', async ({ page, request }) => {
    await page.goto(`${EXTRAS}/serien/${await seriesId(request, EXTRAS, 'Untertitelserie')}`);
    const first = page.locator('[data-episode="S01E01"]');
    await expect(page.locator('[data-episode="S01E02"] summary')).toHaveCount(0);
    await first.locator('summary', { hasText: 'Untertitel' }).click();
    await expect(first.getByTestId('subtitles-external')).toHaveText('keine');
    await expect(first.getByTestId('subtitles-needed').locator('li')).toHaveText([
      /Deutsch · normal/,
      /Englisch · normal/,
    ]);
    await expect(first.getByRole('button', { name: 'Untertitelsuche starten' })).toBeEnabled();
  });

  test('switched off, a file says so and offers no search (AC-15)', async ({ page, request }) => {
    await page.goto(`${RATINGS}/filme/${await movieId(request, RATINGS, 'Bestnote')}`);
    await expect(page.getByText('Die Untertitelsuche ist ausgeschaltet')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Untertitelsuche starten' })).toHaveCount(0);
  });
});
