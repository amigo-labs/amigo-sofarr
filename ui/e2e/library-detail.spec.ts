// spec web-ui-library AC-05, AC-06 (series page), AC-09, AC-10, AC-11, AC-12 (without files),
// AC-17, AC-21, AC-24 against the seeded shadow-mode library server.

import { expect, test } from '@playwright/test';
import { dateText } from '../src/lib/library/format';
import { json } from './helpers';
import { LIBRARY, apiTotal, movieId, seriesId } from './library-helpers';

test.describe('movie page (AC-05, AC-21)', () => {
  test('shows titles, dates, status, availability, profile, folder and the file', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIBRARY, 'Das Boot');
    await page.goto(`${LIBRARY}/filme/${id}`);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Das Boot (1981)');
    await expect(page.getByText('Auch: The Boat')).toBeVisible();
    await expect(page.getByText('Ein U-Boot im Zweiten Weltkrieg.')).toBeVisible();
    await expect(page.getByTestId('stage')).toHaveText('im Kino');
    await expect(page.getByText('verfügbar', { exact: true })).toBeVisible();
    await expect(page.getByText('01.03.1981')).toBeVisible();
    await expect(page.locator('dd').filter({ hasText: '1080p' }).first()).toBeVisible();
    await expect(page.getByTestId('folder')).toContainText('Das Boot (1981)');
    await expect(page.getByTestId('file-path')).toHaveText('Das Boot (1981) Bluray-1080p.mkv');
    await expect(page.getByText('Release-Gruppe')).toBeVisible();
    await expect(page.getByText('Noch keine Grabs.')).toBeVisible();
    await expect(page.getByText('Keine gesperrten Releases.')).toBeVisible();
    // The cached poster comes from sofarr (AC-20/21).
    const poster = page.locator('.poster img').first();
    await expect(poster).toHaveAttribute('src', `/api/images/movie/${id}/detail`);
    await expect.poll(() => poster.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBe(1);
  });

  test('shows the media info read from the file', async ({ page, request }) => {
    const id = await movieId(request, LIBRARY, 'Das Boot');
    // The seeded files are no real videos: the answer gets the media info a read stores.
    await page.route(`**/api/library/movies/${id}`, async (route) => {
      const detail = await (await route.fetch()).json();
      detail.file.media_info = {
        status: 'complete',
        container: 'matroska',
        video: { codec: 'h265', encoder: 'x265', width: 1920, height: 1080 },
        audio: [{ codec: 'eac3', channels: 6, object_audio: false, language: 'ger' }],
        subtitles: [
          { language: 'eng', format: 'srt', forced: true, default: false, hearing_impaired: false },
        ],
        labels: { video: 'x265', audio: 'EAC3', channels: '5.1', dynamic_range: 'HDR10' },
      };
      await json(route, detail);
    });
    await page.goto(`${LIBRARY}/filme/${id}`);
    const fact = (name: string) =>
      page
        .locator('dl > div')
        .filter({ has: page.locator('dt', { hasText: new RegExp(`^${name}$`) }) })
        .locator('dd');
    await expect(page.getByRole('heading', { name: 'Media-Infos' })).toBeVisible();
    await expect(fact('Gelesen')).toHaveText('vollständig');
    await expect(fact('Container')).toHaveText('Matroska');
    await expect(fact('Video')).toHaveText('x265 · 1920 × 1080 · HDR10');
    await expect(fact('Audio')).toHaveText('EAC3 5.1');
    await expect(fact('Audiosprachen')).toHaveText('Deutsch');
    await expect(fact('Untertitel')).toHaveText('Englisch (erzwungen)');
    await expect(page.getByText('Media-Infos wurden noch nicht gelesen.')).toHaveCount(0);
  });

  test('shows cinema, digital and disc dates and the tags', async ({ page, request }) => {
    const fact = (name: string) =>
      page
        .locator('dl > div')
        .filter({ has: page.locator('dt', { hasText: new RegExp(`^${name}$`) }) })
        .locator('dd');
    const id = await movieId(request, LIBRARY, 'Kalenderfilm');
    const detail = await (await request.get(`${LIBRARY}/api/library/movies/${id}`)).json();
    const dates = detail.release_dates as { in_cinemas: string; digital: string; physical: string };
    await page.goto(`${LIBRARY}/filme/${id}`);
    await expect(fact('Im Kino')).toHaveText(dateText(dates.in_cinemas));
    await expect(fact('Digital')).toHaveText(dateText(dates.digital));
    await expect(fact('Disc')).toHaveText(dateText(dates.physical));
    await expect(fact('Tags')).toHaveText('keine');
    await page.goto(`${LIBRARY}/filme/${await movieId(request, LIBRARY, 'Sammel A')}`);
    await expect(fact('Tags')).toHaveText('sammlung');
  });

  test('an unavailable poster shows the initial; no request leaves for anyone but sofarr', async ({
    page,
    request,
  }) => {
    const hosts = new Set<string>();
    page.on('request', (r) => hosts.add(new URL(r.url()).host));
    const id = await movieId(request, LIBRARY, 'Ohne Bild');
    await page.goto(`${LIBRARY}/filme/${id}`);
    await expect(page.getByTestId('poster-placeholder')).toHaveText('O');
    await page.goto(`${LIBRARY}/filme?suche=Boot`);
    await expect(page.locator('[data-testid="library-items"] img')).toHaveCount(1);
    await page.goto(`${LIBRARY}/filme?suche=Ohne`);
    await expect(page.getByTestId('poster-placeholder')).toHaveText('O');
    expect([...hosts]).toEqual([new URL(LIBRARY).host]);
  });

  test('grabs with state and message and blocklist entries; marking a grab as failed', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIBRARY, 'Fehlt Sehr');
    const real = (await (
      await request.get(`${LIBRARY}/api/library/movies/${id}`)
    ).json()) as Record<string, unknown>;
    let marked = 0;
    await page.route(`**/api/library/movies/${id}`, (route) =>
      json(route, {
        ...real,
        grabs: [
          {
            id: 7,
            title: 'Fehlt.Sehr.2015.1080p.WEB-DL-GRP',
            indexer: 'Testindexer',
            quality: 'WEBDL-1080p',
            size: 4_000_000_000,
            status: 'downloading',
            message: 'Lädt bei nzbget',
            grabbed_at: '2026-11-12T10:00:00Z',
            active: true,
            episode_ids: [],
          },
        ],
        blocklist: [
          {
            id: 3,
            title: 'Fehlt.Sehr.2015.720p-ALT',
            indexer: 'Testindexer',
            reason: 'Download fehlgeschlagen',
            blocklisted_at: '2026-11-10T08:00:00Z',
          },
        ],
      }),
    );
    await page.route('**/api/grabs/movie/7/fail', (route) => {
      marked += 1;
      return route.fulfill({ status: 204 });
    });
    await page.goto(`${LIBRARY}/filme/${id}`);
    const grab = page.getByTestId('grab');
    await expect(grab).toContainText('lädt herunter');
    await expect(grab).toContainText('Lädt bei nzbget');
    await expect(page.getByText('Fehlt.Sehr.2015.720p-ALT')).toBeVisible();
    await expect(page.getByText('Download fehlgeschlagen')).toBeVisible();
    await grab.getByRole('button', { name: 'Als fehlgeschlagen markieren' }).click();
    await expect(page.getByText('wurde als fehlgeschlagen markiert')).toBeVisible();
    expect(marked).toBe(1);
    expect(new URL(page.url()).pathname).toBe(`/filme/${id}`);
  });
});

test.describe('actions with German results (AC-09)', () => {
  test('a search shows its result without leaving the page', async ({ page, request }) => {
    const id = await movieId(request, LIBRARY, 'Fehlt Sehr');
    await page.goto(`${LIBRARY}/filme/${id}`);
    await page.getByRole('button', { name: 'Suchen' }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Fehlt Sehr: Suche' })).toBeVisible();
    expect(new URL(page.url()).pathname).toBe(`/filme/${id}`);
  });

  test('a search report with grabs and errors reads in German', async ({ page, request }) => {
    const id = await movieId(request, LIBRARY, 'Fehlt Sehr');
    await page.route('**/api/command/search', (route) =>
      json(route, { releases: 4, grabs: 1, errors: ['Testindexer: Zeitüberschreitung'] }),
    );
    await page.goto(`${LIBRARY}/filme/${id}`);
    await page.getByRole('button', { name: 'Suchen' }).click();
    await expect(page.getByRole('alert')).toContainText(
      'Fehlt Sehr: Suche beendet: 4 Releases gefunden, 1 Grab. Fehler: Testindexer: Zeitüberschreitung',
    );
  });

  test('marking without a reachable download client reports the German message', async ({
    page,
  }) => {
    await page.route('**/api/overview', (route) =>
      json(route, {
        queue: {
          items: [
            {
              download_state: 'paused',
              progress: 0.42,
              remaining_bytes: 1_000_000,
              remaining_seconds: null,
              kind: 'movie',
              grab_id: 9,
              media_id: 1,
              media_title: 'Film 0001',
              release_title: 'Film.0001.1951.1080p-GRP',
              quality: 'Bluray-1080p',
              size: 8_000_000_000,
              status: 'downloading',
              message: null,
              episode_ids: [],
              grabbed_at: '2026-11-12T09:00:00Z',
              updated_at: '2026-11-12T09:30:00Z',
            },
          ],
          notice: null,
          as_of: '2026-11-12T09:30:00Z',
        },
        activity: [],
        problems: [],
        upcoming: [],
      }),
    );
    await page.route('**/api/grabs/movie/9/fail', (route) =>
      json(route, { error: 'Download-Client nicht erreichbar' }, 503),
    );
    await page.goto(`${LIBRARY}/`);
    await page.getByRole('button', { name: 'Als fehlgeschlagen markieren' }).click();
    await expect(page.getByRole('alert')).toContainText(
      '„Film 0001“ nicht markiert: Download-Client nicht erreichbar',
    );
  });
});

test.describe('series page (AC-06, AC-10)', () => {
  test('shows seasons with counts and episodes with number, title, air date, file, monitoring', async ({
    page,
    request,
  }) => {
    const id = await seriesId(request, LIBRARY, 'Kleine Insel');
    await page.goto(`${LIBRARY}/serien/${id}`);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Kleine Insel (2020)');
    await expect(page.getByTestId('episode-counts')).toHaveText(
      '1 mit Datei / 3 überwacht / 5 gesamt',
    );
    const counts = page.getByTestId('season-counts');
    await expect(counts.first()).toContainText('1 mit Datei / 3 überwacht / 3 gesamt');
    const e1 = page.locator('[data-episode="S01E01"]');
    await expect(e1).toContainText('Ankunft');
    await expect(e1).toContainText('01.01.2024');
    await expect(e1).toContainText('WEBDL-1080p');
    await expect(e1).toContainText('Cutoff nicht erreicht');
    const e2 = page.locator('[data-episode="S01E02"]');
    await expect(e2).toContainText('Sturm');
    await expect(e2).toContainText('fehlt');
    await expect(
      e2.getByRole('button', { name: 'Überwachen „Kleine Insel S01E02“' }),
    ).toHaveAttribute('aria-pressed', 'true');
  });

  test('shows what the movie page shows: overview, profile, tags, folder, grabs, blocklist and the file of an episode', async ({
    page,
    request,
  }) => {
    const id = await seriesId(request, LIBRARY, 'Kleine Insel');
    const real = (await (await request.get(`${LIBRARY}/api/library/series/${id}`)).json()) as {
      quality_profile: { name: string };
      seasons: { episodes: { id: number; season: number; number: number }[] }[];
    };
    const first = real.seasons
      .flatMap((s) => s.episodes)
      .find((e) => e.season === 1 && e.number === 1);
    expect(first).toBeDefined();
    await page.route(`**/api/library/series/${id}`, (route) =>
      json(route, {
        ...real,
        overview: 'Eine kleine Insel im Sturm.',
        tags: [{ id: 3, name: 'familie' }],
        grabs: [
          {
            id: 9,
            title: 'Kleine.Insel.S01E02.1080p.WEB-DL-GRP',
            indexer: 'Testindexer',
            quality: 'WEBDL-1080p',
            size: 2_000_000_000,
            status: 'failed',
            message: 'Entpacken fehlgeschlagen',
            grabbed_at: '2026-11-12T10:00:00Z',
            active: false,
            episode_ids: [],
          },
        ],
        blocklist: [
          {
            id: 4,
            title: 'Kleine.Insel.S01E02.720p-ALT',
            indexer: 'Testindexer',
            reason: 'Download fehlgeschlagen',
            blocklisted_at: '2026-11-10T08:00:00Z',
          },
        ],
      }),
    );
    // The seeded files are no real videos: the episode answer gets the media info a read stores.
    await page.route(`**/api/library/series/${id}/episodes/${first?.id}`, async (route) => {
      const episode = await (await route.fetch()).json();
      episode.file.media_info = {
        status: 'complete',
        container: 'matroska',
        video: { codec: 'h265', encoder: 'x265', width: 1920, height: 1080 },
        audio: [{ codec: 'aac', channels: 2, object_audio: false, language: 'ger' }],
        subtitles: [],
        labels: { video: 'x265', audio: 'AAC', channels: '2.0', dynamic_range: null },
      };
      await json(route, episode);
    });
    await page.goto(`${LIBRARY}/serien/${id}`);
    const fact = (name: string) =>
      page
        .locator('dl > div')
        .filter({ has: page.locator('dt', { hasText: new RegExp(`^${name}$`) }) })
        .locator('dd');
    await expect(page.getByText('Eine kleine Insel im Sturm.')).toBeVisible();
    await expect(fact('Qualitätsprofil')).toHaveText(real.quality_profile.name);
    await expect(fact('Tags')).toHaveText('familie');
    await expect(page.getByTestId('folder')).toContainText('Kleine Insel');
    const grab = page.getByTestId('grab');
    await expect(grab).toContainText('Kleine.Insel.S01E02.1080p.WEB-DL-GRP');
    await expect(grab).toContainText('Entpacken fehlgeschlagen');
    await expect(page.getByText('Kleine.Insel.S01E02.720p-ALT')).toBeVisible();
    await expect(page.getByText('Download fehlgeschlagen')).toBeVisible();
    const e1 = page.locator('[data-episode="S01E01"]');
    await e1
      .locator('summary')
      .filter({ hasText: /^Datei/ })
      .click();
    await expect(e1.getByTestId('file-path')).toHaveText(/S01E01/);
    await expect(e1.getByText('Release-Gruppe')).toBeVisible();
    await expect(e1.getByRole('heading', { name: 'Media-Infos' })).toBeVisible();
    await expect(e1.getByText('x265 · 1920 × 1080 · SDR')).toBeVisible();
    await expect(e1.getByText('AAC 2.0')).toBeVisible();
  });

  test('switching a season switches its episodes', async ({ page, request }) => {
    const id = await seriesId(request, LIBRARY, 'Staffelschalter');
    await page.goto(`${LIBRARY}/serien/${id}`);
    await page.getByRole('button', { name: 'Überwachen „Staffelschalter – Staffel 1“' }).click();
    for (const code of ['S01E01', 'S01E02']) {
      await expect(
        page.getByRole('button', { name: `Überwachen „Staffelschalter ${code}“` }),
      ).toHaveAttribute('aria-pressed', 'true');
    }
    await page.locator('summary').filter({ hasText: 'Staffel 2' }).click();
    await expect(
      page.getByRole('button', { name: 'Überwachen „Staffelschalter S02E01“' }),
    ).toHaveAttribute('aria-pressed', 'false');
  });
});

test.describe('profile and availability (AC-11)', () => {
  test('the page shows what becomes missing before saving the availability', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIBRARY, 'Bald verfügbar');
    await page.goto(`${LIBRARY}/filme/${id}`);
    const save = page.getByRole('button', { name: 'Speichern' });
    await expect(save).toBeDisabled();
    await page.getByLabel('Mindestverfügbarkeit').selectOption({ label: 'im Kino' });
    const preview = page.getByTestId('change-preview');
    await expect(preview).toContainText('Werden fehlend');
    await expect(preview).toContainText('Bald verfügbar (2026)');
    await save.click();
    await expect(page.getByText('Einstellungen gespeichert')).toBeVisible();
    await expect(page.locator('dd').filter({ hasText: 'im Kino' }).first()).toBeVisible();
    expect(await apiTotal(request, '/api/library/movies?q=Bald%20verf%C3%BCgbar&missing=yes')).toBe(
      1,
    );
  });

  test('a profile change is previewed, saved, or dropped', async ({ page, request }) => {
    const id = await movieId(request, LIBRARY, 'Profilwechsel');
    await page.goto(`${LIBRARY}/filme/${id}`);
    await page.getByLabel('Qualitätsprofil').selectOption({ label: 'Ultra-HD' });
    await expect(page.getByTestId('change-preview')).toBeVisible();
    await page.getByRole('button', { name: 'Speichern' }).click();
    await expect(page.getByText('Einstellungen gespeichert')).toBeVisible();
    expect(await apiTotal(request, '/api/library/movies?q=Profilwechsel&profile=5')).toBe(1);
    await page.getByLabel('Mindestverfügbarkeit').selectOption({ label: 'angekündigt' });
    await expect(page.getByTestId('change-preview')).toBeVisible();
    await page.getByRole('button', { name: 'Verwerfen' }).click();
    await expect(page.getByLabel('Mindestverfügbarkeit')).toHaveValue('released');
  });
});

test.describe('shadow mode (AC-17, AC-12)', () => {
  test('file actions are not offered; removing without files works', async ({ page, request }) => {
    const id = await movieId(request, LIBRARY, 'Entfernen im Shadow');
    await page.goto(`${LIBRARY}/filme/${id}`);
    await expect(page.getByText('gibt es nur im Live-Modus')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Umbenennen …' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Ordner verschieben …' })).toHaveCount(0);
    await page.getByRole('button', { name: 'Entfernen …' }).click();
    const dialog = page.getByRole('dialog', { name: '„Entfernen im Shadow“ entfernen' });
    await expect(dialog.getByLabel(/Dateien löschen/)).toBeDisabled();
    await dialog.getByLabel(/auf die Ausschlussliste/).check();
    await dialog.getByRole('button', { name: 'Entfernen' }).click();
    await expect(page).toHaveURL(`${LIBRARY}/filme`);
    await expect(
      page.getByText('„Entfernen im Shadow“ wurde aus der Bibliothek entfernt.'),
    ).toBeVisible();
    expect(await apiTotal(request, '/api/library/movies?q=Entfernen%20im%20Shadow')).toBe(0);
  });

  test('monitoring and profile still work in shadow mode', async ({ page, request }) => {
    const id = await movieId(request, LIBRARY, 'Schattenaktion');
    await page.goto(`${LIBRARY}/filme/${id}`);
    const monitor = page.getByRole('button', { name: 'Überwachen „Schattenaktion“' });
    await monitor.click();
    await expect(monitor).toHaveAttribute('aria-pressed', 'true');
  });
});

test.describe('refused file actions (AC-24)', () => {
  test('a file action refused for an active download shows the German message', async ({
    page,
    request,
  }) => {
    const id = await movieId(request, LIBRARY, 'Schattenaktion');
    await page.route(`**/api/library/movies/${id}/remove`, (route) =>
      json(
        route,
        {
          error: 'Für „Schattenaktion“ läuft ein Download – erst danach sind Dateiaktionen möglich',
        },
        409,
      ),
    );
    await page.goto(`${LIBRARY}/filme/${id}`);
    await page.getByRole('button', { name: 'Entfernen …' }).click();
    await page.getByRole('dialog').getByRole('button', { name: 'Entfernen' }).click();
    await expect(page.getByRole('alert')).toContainText(
      'Entfernen nicht ausgeführt: Für „Schattenaktion“ läuft ein Download',
    );
    expect(new URL(page.url()).pathname).toBe(`/filme/${id}`);
  });
});
