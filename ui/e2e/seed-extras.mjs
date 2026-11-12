// Test libraries and fake services of the e2e servers for IMDb ratings, subtitles and
// trailers (specs imdb-ratings, subtitles, trailers):
// - `ratings`: shadow mode, a small library with IMDb ids; a fake IMDb dataset server serves
//   a generated `title.ratings.tsv.gz` (the first load needs at least 100,000 titles).
// - `extras`: live mode with real (tiny) video files, IMDb ratings off, and a fake
//   OpenSubtitles.com: one movie gets a German subtitle at the first run, one only at a
//   search by hand, everything else stays needed.
// Secrets of the fakes are made per run: test keys are never literals.

import { randomUUID } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { portBase } from './env.mjs';
import { SONARR_KEY, episode, fakeSonarr, file, movie, run } from './seed.mjs';

export const EXTRA_VARIANTS = ['ratings', 'extras'];

const IMDB_PORT = portBase() + 40;
const OPENSUBTITLES_PORT = portBase() + 41;
const SONARR_PORTS = { ratings: portBase() + 42, extras: portBase() + 43 };
const OPENSUBTITLES_KEY = `os-key-${randomUUID()}`;
const OPENSUBTITLES_PASSWORD = `os-pass-${randomUUID()}`;
/** The first load of ratings needs at least this many titles (spec imdb-ratings AC-02). */
const DATASET_TITLES = 100_050;

const slash = (path) => path.replaceAll('\\', '/');

/** Movies of the `ratings` library: [title, IMDb id, rating, votes]. */
const RATED_MOVIES = [
  ['Bestnote', 'tt0000001', 9.1, 250_000],
  ['Gutfilm', 'tt0000002', 7.5, 12_000],
  ['Nischenfilm', 'tt0000003', 9.8, 12],
  ['Mittelmaß', 'tt0000004', 5.2, 80_000],
  ['Ohne IMDb', null, null, null],
  ['Unbewertet', 'tt0999999', null, null],
];
const RATED_SERIES = [
  ['Hochserie', 'tt0000101', 8.8, 50_000],
  ['Tiefserie', 'tt0000102', 6, 3000],
  ['Serie ohne IMDb', null, null, null],
];

/** The ratings dataset: the titles above and filler titles nobody has. */
function dataset() {
  const lines = ['tconst\taverageRating\tnumVotes'];
  for (const [, id, rating, votes] of [...RATED_MOVIES, ...RATED_SERIES]) {
    if (id !== null && rating !== null) lines.push(`${id}\t${rating.toFixed(1)}\t${votes}`);
  }
  for (let n = lines.length; n <= DATASET_TITLES; n += 1) {
    lines.push(`tt${String(5_000_000 + n)}\t${(1 + (n % 90) / 10).toFixed(1)}\t${n % 1000}`);
  }
  return gzipSync(`${lines.join('\n')}\n`);
}

/** Serves the ratings dataset at any path. */
function fakeImdb() {
  const body = dataset();
  return createServer((_request, response) => {
    response.writeHead(200, { 'content-type': 'application/gzip' });
    response.end(body);
  });
}

const SRT = '1\n00:00:01,000 --> 00:00:03,000\nHallo aus dem Test\n';

/** One search result of the fake OpenSubtitles.com. */
function result(fileId, language, release) {
  return {
    id: String(fileId),
    type: 'subtitle',
    attributes: {
      subtitle_id: String(fileId),
      language,
      release,
      hearing_impaired: false,
      foreign_parts_only: false,
      moviehash_match: false,
      download_count: 10,
      files: [{ file_id: fileId, file_name: `${release}.srt` }],
    },
  };
}

function answer(response, status, body) {
  response.writeHead(status, { 'content-type': 'application/json' });
  response.end(JSON.stringify(body));
}

/**
 * A fake OpenSubtitles.com: "Untertitelfilm" has a German subtitle; "Spätsuche" has an
 * English one from its second search on; nothing else has any.
 */
function fakeOpenSubtitles(tmdbIds) {
  const searches = new Map();
  return createServer((request, response) => {
    const url = new URL(request.url ?? '/', 'http://localhost');
    if (url.pathname.startsWith('/file/')) {
      response.writeHead(200, { 'content-type': 'text/plain' });
      response.end(SRT);
      return;
    }
    if (request.headers['api-key'] !== OPENSUBTITLES_KEY) {
      answer(response, 401, { message: 'unbekannter Schlüssel' });
      return;
    }
    if (url.pathname === '/api/v1/login') {
      let text = '';
      request.on('data', (chunk) => (text += chunk));
      request.on('end', () => {
        const ok = JSON.parse(text).password === OPENSUBTITLES_PASSWORD;
        answer(response, ok ? 200 : 401, ok ? { token: randomUUID(), status: 200 } : {});
      });
      return;
    }
    if (url.pathname === '/api/v1/download') {
      let text = '';
      request.on('data', (chunk) => (text += chunk));
      request.on('end', () => {
        const id = JSON.parse(text).file_id;
        answer(response, 200, {
          link: `http://127.0.0.1:${OPENSUBTITLES_PORT}/file/${id}`,
          remaining: 99,
          reset_time_utc: new Date(Date.now() + 86_400_000).toISOString(),
        });
      });
      return;
    }
    if (url.pathname === '/api/v1/subtitles') {
      const tmdb = Number(url.searchParams.get('tmdb_id'));
      const language = url.searchParams.get('languages');
      const key = `${tmdb}-${language}`;
      const count = (searches.get(key) ?? 0) + 1;
      searches.set(key, count);
      const data = [];
      if (tmdb === tmdbIds.Untertitelfilm && language === 'de') {
        data.push(result(1001, 'de', 'Untertitelfilm.2020.German.1080p.BluRay.x264-GRP'));
      }
      if (tmdb === tmdbIds['Spätsuche'] && language === 'en' && count > 1) {
        data.push(result(1002, 'en', 'Spaetsuche.2022.1080p.WEB-DL.x264-GRP'));
      }
      answer(response, 200, { total_count: data.length, data });
      return;
    }
    answer(response, 404, { message: 'unbekannt' });
  });
}

function listen(server, port) {
  return new Promise((resolve) => server.listen(port, '127.0.0.1', () => resolve(server)));
}

/** The movies of a variant's Radarr export. */
function moviesOf(variant, root) {
  if (variant === 'ratings') {
    return RATED_MOVIES.map(([title, imdbId], i) => {
      const rated = movie(i + 1, root, {
        title,
        year: 2001 + i,
        file: [`${title}.mkv`, 'Bluray-1080p'],
      });
      if (imdbId !== null) rated.imdbId = imdbId;
      return rated;
    });
  }
  const named = [
    { title: 'Untertitelfilm', year: 2020, monitored: true, file: true },
    { title: 'Fremduntertitel', year: 2021, monitored: true, file: true },
    { title: 'Spätsuche', year: 2022, monitored: true, file: true },
    { title: 'Trailerfilm', year: 2019, monitored: false, file: true },
    { title: 'Ohne Datei', year: 2018, monitored: false, file: false },
  ];
  return named.map((m, i) =>
    movie(i + 1, root, {
      title: m.title,
      year: m.year,
      monitored: m.monitored,
      file: m.file ? [`${m.title} (${m.year}).mkv`, 'Bluray-1080p'] : null,
    }),
  );
}

/** The series of a variant for the fake Sonarr, with episodes and episode files. */
function seriesOf(variant, root) {
  const list =
    variant === 'ratings'
      ? RATED_SERIES.map(([title, imdbId]) => ({ title, imdbId, monitored: false }))
      : [{ title: 'Untertitelserie', imdbId: null, monitored: true }];
  const series = [];
  const episodes = new Map();
  const files = new Map();
  list.forEach((s, i) => {
    const id = i + 1;
    series.push({
      id,
      tvdbId: 80_000 + id,
      ...(s.imdbId === null ? {} : { imdbId: s.imdbId }),
      title: s.title,
      year: 2021,
      monitored: s.monitored,
      qualityProfileId: 4,
      seasonFolder: true,
      seriesType: 'standard',
      path: `${root}/${s.title}`,
      tags: [],
      alternateTitles: [],
      images: [],
    });
    episodes.set(id, [
      episode(id * 10 + 1, 1, 1, { title: 'Erste', monitored: s.monitored, fileId: id }),
      episode(id * 10 + 2, 1, 2, { title: 'Zweite', monitored: s.monitored }),
    ]);
    files.set(id, [{ id, ...file(`Season 1/${s.title} S01E01.mkv`, 'WEBDL-1080p') }]);
  });
  return { series, episodes, files };
}

/** Video files, a foreign subtitle and a foreign trailer for the `extras` library. */
function writeExtrasFiles(movies, seriesRoot, data) {
  for (const m of movies) {
    mkdirSync(m.path, { recursive: true });
    if (m.movieFile) writeFileSync(join(m.path, m.movieFile.relativePath), 'e2e');
  }
  const byTitle = (title) => movies.find((m) => m.title === title);
  const foreign = byTitle('Fremduntertitel');
  writeFileSync(join(foreign.path, 'Fremduntertitel (2021).de.srt'), SRT);
  const trailer = byTitle('Trailerfilm');
  mkdirSync(join(trailer.path, 'trailers'), { recursive: true });
  writeFileSync(join(trailer.path, 'trailers', 'Mein Trailer.mkv'), 'e2e');
  for (const s of data.series) {
    for (const f of data.files.get(s.id) ?? []) {
      const path = join(seriesRoot, s.title, f.relativePath);
      mkdirSync(join(path, '..'), { recursive: true });
      writeFileSync(path, 'e2e');
    }
  }
}

/** The config lines of an extra variant in `sofarr.toml`. */
export function extrasConfig(variant) {
  const sonarr = [
    '[sonarr]',
    `url = "http://127.0.0.1:${SONARR_PORTS[variant]}"`,
    `api_key = "${SONARR_KEY}"`,
  ];
  if (variant === 'ratings') {
    return ['[imdb]', `url = "http://127.0.0.1:${IMDB_PORT}/title.ratings.tsv.gz"`, ...sonarr];
  }
  return [
    '[automation]',
    'mode = "live"',
    '[imdb]',
    'enabled = false',
    '[opensubtitles]',
    `api_key = "${OPENSUBTITLES_KEY}"`,
    'username = "e2e"',
    `password = "${OPENSUBTITLES_PASSWORD}"`,
    `url = "http://127.0.0.1:${OPENSUBTITLES_PORT}/api/v1"`,
    'pause_ms = 0',
    ...sonarr,
  ];
}

/**
 * Imports the library of an extra variant and starts the fakes the running server needs
 * (they stop with this process).
 */
export async function seedExtras(variant, { binary, config, dir, fixtures }) {
  const root = slash(join(dir, 'media', 'filme'));
  const seriesRoot = slash(join(dir, 'media', 'serien'));
  const exports = join(dir, 'radarr-export');
  mkdirSync(exports, { recursive: true });
  for (const name of ['qualityprofile', 'qualitydefinition', 'customformat']) {
    copyFileSync(join(fixtures, `${name}.json`), join(exports, `${name}.json`));
  }
  const movies = moviesOf(variant, root);
  const write = (name, value) =>
    writeFileSync(join(exports, `${name}.json`), JSON.stringify(value));
  write('movie', movies);
  write('tag', []);
  write('rootfolder', [{ path: root }]);
  write('exclusions', []);
  const data = seriesOf(variant, seriesRoot);
  if (variant === 'extras') writeExtrasFiles(movies, seriesRoot, data);
  await run(binary, ['--config', config, 'import-radarr', '--exports', exports]);

  const read = (name) => JSON.parse(readFileSync(join(fixtures, `${name}.json`), 'utf8'));
  const docs = {
    qualityprofile: read('qualityprofile'),
    qualitydefinition: read('qualitydefinition'),
    customformat: read('customformat'),
  };
  const sonarr = await listen(fakeSonarr(docs, seriesRoot, data), SONARR_PORTS[variant]);
  try {
    await run(binary, ['--config', config, 'import-sonarr']);
  } finally {
    sonarr.close();
  }

  if (variant === 'ratings') return [await listen(fakeImdb(), IMDB_PORT)];
  const tmdbIds = Object.fromEntries(movies.map((m) => [m.title, m.tmdbId]));
  return [await listen(fakeOpenSubtitles(tmdbIds), OPENSUBTITLES_PORT)];
}
