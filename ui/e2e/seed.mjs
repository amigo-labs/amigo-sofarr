// Test libraries for the e2e servers of spec web-ui-library. sofarr has no seed command, so
// the library goes in the way a real one does: `import-radarr --exports <folder>` reads a
// Radarr export written here, and `import-sonarr` reads a fake Sonarr API served here for
// the duration of the import. Quality profiles come from fixtures/public/config/radarr.

import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { join } from 'node:path';
import { portBase } from './env.mjs';

/** Movies of the large library (AC-04); the named movies below come on top. */
export const FILLER_MOVIES = 2490;
/** The fake Sonarr of each seeded variant (they start at the same time). */
const SONARR_PORTS = { library: portBase() + 20, bench: portBase() + 21 };
/** The fake Sonarr's API key, made per run: test keys are never literals. */
export const SONARR_KEY = `fake-sonarr-${randomUUID()}`;

/** A 1x1 PNG: the cached poster of "Das Boot" (served from sofarr's image cache). */
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
);
export const CACHED_POSTER = '/e2e-poster.png';

const slash = (path) => path.replaceAll('\\', '/');

/** `YYYY-MM-DD` of today plus `days`, in UTC. */
function day(days) {
  const date = new Date();
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

function file(relativePath, quality) {
  return {
    relativePath,
    size: 4_500_000_000,
    releaseGroup: 'GRP',
    quality: { quality: { name: quality }, revision: { version: 1, real: 0 } },
  };
}

function movie(n, root, fields) {
  const year = fields.year ?? 2000;
  return {
    id: n,
    tmdbId: 100_000 + n,
    title: fields.title,
    originalTitle: fields.originalTitle ?? fields.title,
    year,
    monitored: fields.monitored ?? false,
    qualityProfileId: fields.profile ?? 4,
    path: `${root}/${fields.folder ?? `${fields.title} (${year})`}`,
    minimumAvailability: fields.minimumAvailability ?? 'released',
    status: fields.status ?? 'released',
    inCinemas: fields.inCinemas ?? `${year}-03-01T00:00:00Z`,
    digitalRelease: fields.digitalRelease ?? null,
    physicalRelease: fields.physicalRelease ?? null,
    overview: fields.overview ?? null,
    tags: fields.tags ?? [],
    alternateTitles: (fields.alternativeTitles ?? []).map((title) => ({ title })),
    images: fields.poster ? [{ coverType: 'poster', remoteUrl: fields.poster }] : [],
    ...(fields.file ? { movieFile: file(fields.file[0], fields.file[1]) } : {}),
  };
}

/** The large shadow-mode library: 2,490 filler movies plus named ones the tests use. */
function libraryMovies(root) {
  const movies = [];
  for (let n = 1; n <= FILLER_MOVIES; n += 1) {
    const title = `Film ${String(n).padStart(4, '0')}`;
    const year = 1950 + (n % 70);
    const quality = n % 2 === 0 ? 'Bluray-1080p' : 'WEBDL-1080p';
    movies.push(
      movie(n, root, {
        title,
        year,
        monitored: n % 50 === 0,
        tags: n % 10 === 0 ? [1] : [],
        file: n % 3 === 0 ? null : [`${title}.mkv`, quality],
      }),
    );
  }
  let n = FILLER_MOVIES;
  const named = (fields) => movies.push(movie((n += 1), root, fields));
  named({
    title: 'Das Boot',
    year: 1981,
    monitored: true,
    alternativeTitles: ['The Boat'],
    overview: 'Ein U-Boot im Zweiten Weltkrieg.',
    poster: `https://image.tmdb.org/t/p/original${CACHED_POSTER}`,
    file: ['Das Boot (1981) Bluray-1080p.mkv', 'Bluray-1080p'],
  });
  named({
    title: 'Ohne Bild',
    year: 1990,
    poster: 'https://image.tmdb.org/t/p/original/e2e-nicht-im-cache.jpg',
  });
  named({
    title: 'Kalenderfilm',
    year: Number(day(0).slice(0, 4)),
    monitored: true,
    status: 'inCinemas',
    inCinemas: `${day(0)}T00:00:00Z`,
    digitalRelease: `${day(2)}T00:00:00Z`,
    physicalRelease: `${day(4)}T00:00:00Z`,
  });
  named({ title: 'Einzeln Überwachen', year: 2010 });
  for (const letter of ['A', 'B', 'C']) {
    named({ title: `Sammel ${letter}`, year: 2011, tags: [2] });
  }
  named({ title: 'Profilwechsel', year: 2012, monitored: true, file: ['p.mkv', 'WEBDL-1080p'] });
  named({ title: 'Entfernen im Shadow', year: 2013 });
  named({ title: 'Schattenaktion', year: 2014, file: ['s.mkv', 'Bluray-1080p'] });
  named({ title: 'Fehlt Sehr', year: 2015, monitored: true });
  named({
    title: 'Bald verfügbar',
    year: 2026,
    monitored: true,
    status: 'inCinemas',
    inCinemas: `${day(-10)}T00:00:00Z`,
  });
  return movies;
}

/** The live-mode library: movies with real files for the file actions. */
function liveMovies(root) {
  let n = 0;
  const list = [];
  const named = (fields) => list.push(movie((n += 1), root, fields));
  named({ title: 'Neuer Name', year: 2001, file: ['alter.name.mkv', 'Bluray-1080p'] });
  named({
    title: 'Neuer Titel',
    year: 2002,
    folder: 'Alter Titel (2002)',
    file: ['Neuer Titel (2002) Bluray-1080p.mkv', 'Bluray-1080p'],
  });
  named({
    title: 'Blockiert',
    year: 2003,
    folder: 'Blockiert alt (2003)',
    file: ['Blockiert (2003) Bluray-1080p.mkv', 'Bluray-1080p'],
  });
  named({ title: 'Ziel Belegt', year: 2005, file: ['x.mkv', 'Bluray-1080p'] });
  named({ title: 'Weg Damit', year: 2004, file: ['weg.mkv', 'Bluray-1080p'] });
  named({ title: 'Ohne Dateien Weg', year: 2006, file: ['bleibt.mkv', 'Bluray-1080p'] });
  return list;
}

/** Folders and stray files the live tests expect next to the library files. */
function writeLiveFiles(root, movies) {
  for (const m of movies) {
    mkdirSync(m.path, { recursive: true });
    if (m.movieFile) writeFileSync(join(m.path, m.movieFile.relativePath), 'e2e');
  }
  // A folder that is in the way of "Blockiert" (AC-16) and a file in the way of a rename.
  mkdirSync(join(root, 'Blockiert (2003)'), { recursive: true });
  writeFileSync(join(root, 'Ziel Belegt (2005)', 'Ziel Belegt (2005) Bluray-1080p.mkv'), 'da');
}

function episode(id, season, number, fields) {
  return {
    id,
    tvdbId: 900_000 + id,
    seasonNumber: season,
    episodeNumber: number,
    title: fields.title ?? `Folge ${number}`,
    airDate: fields.airDate ?? '2024-01-01',
    monitored: fields.monitored ?? true,
    episodeFileId: fields.fileId ?? 0,
  };
}

/** Series, episodes and episode files of the fake Sonarr. */
function librarySeries(root) {
  const series = [];
  const episodes = new Map();
  const files = new Map();
  let episodeId = 0;
  let fileId = 0;
  const add = (title, monitored, list) => {
    const id = series.length + 1;
    series.push({
      id,
      tvdbId: 70_000 + id,
      title,
      year: 2020,
      monitored,
      qualityProfileId: 4,
      seasonFolder: true,
      seriesType: 'standard',
      path: `${root}/${title}`,
      tags: [],
      alternateTitles: [],
      images: [],
    });
    const eps = [];
    const fs = [];
    for (const [season, number, fields] of list) {
      const quality = fields.file;
      const fid = quality ? (fileId += 1) : 0;
      eps.push(episode((episodeId += 1), season, number, { ...fields, fileId: fid }));
      if (quality) {
        fs.push({
          id: fid,
          ...file(`Season ${season}/${title} S0${season}E0${number}.mkv`, quality),
        });
      }
    }
    episodes.set(id, eps);
    files.set(id, fs);
  };
  add('Kleine Insel', true, [
    [1, 1, { title: 'Ankunft', file: 'WEBDL-1080p' }],
    [1, 2, { title: 'Sturm', airDate: '2024-01-08' }],
    [1, 3, { title: 'Abschied', airDate: day(1) }],
    [2, 1, { title: 'Rückkehr', monitored: false }],
    [2, 2, { title: 'Ende', monitored: false }],
  ]);
  add('Serie Zwei', true, [
    [1, 1, { file: 'Bluray-1080p' }],
    [1, 2, { file: 'Bluray-1080p' }],
  ]);
  add('Staffelschalter', false, [
    [1, 1, { monitored: false }],
    [1, 2, { monitored: false }],
    [2, 1, { monitored: false }],
  ]);
  for (let i = 1; i <= 20; i += 1) {
    add(`Serie ${String(i).padStart(2, '0')}`, false, [[1, 1, { file: 'Bluray-1080p' }]]);
  }
  return { series, episodes, files };
}

/** Serves Sonarr's v3 API for the import: settings, series, episodes, episode files. */
function fakeSonarr(fixtures, root, data) {
  const lists = {
    qualityprofile: fixtures.qualityprofile,
    qualitydefinition: fixtures.qualitydefinition,
    customformat: fixtures.customformat,
    tag: [{ id: 1, label: 'kinder' }],
    rootfolder: [{ path: root }],
    series: data.series,
  };
  return createServer((request, response) => {
    const url = new URL(request.url ?? '/', 'http://localhost');
    const key = url.pathname.replace(/^\/api\/v3\//, '');
    const seriesId = Number(url.searchParams.get('seriesId'));
    let body;
    if (request.headers['x-api-key'] !== SONARR_KEY) body = null;
    else if (key === 'episode') body = data.episodes.get(seriesId) ?? [];
    else if (key === 'episodefile') body = data.files.get(seriesId) ?? [];
    else if (key in lists) body = lists[key];
    else if (key.startsWith('config/')) body = {};
    response.writeHead(body === undefined || body === null ? 404 : 200, {
      'content-type': 'application/json',
    });
    response.end(JSON.stringify(body ?? { error: 'unbekannt' }));
  });
}

function run(binary, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ['ignore', 'ignore', 'inherit'] });
    child.on('error', reject);
    child.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`${args.join(' ')} exited with ${code}`)),
    );
  });
}

/** The config lines a seeded variant needs in `sofarr.toml`. */
export function seedConfig(variant) {
  if (variant === 'live') return ['[automation]', 'mode = "live"'];
  const port = SONARR_PORTS[variant];
  return ['[sonarr]', `url = "http://127.0.0.1:${port}"`, `api_key = "${SONARR_KEY}"`];
}

/**
 * Imports the test library of `variant` into the configured database: `library` and
 * `bench` (the same library, only read by the measurement of AC-04) or `live`.
 */
export async function seed(variant, { binary, config, dir, fixtures }) {
  const root = slash(join(dir, 'media', variant === 'live' ? 'filme' : 'bibliothek'));
  const exports = join(dir, 'radarr-export');
  mkdirSync(exports, { recursive: true });
  for (const name of ['qualityprofile', 'qualitydefinition', 'customformat']) {
    copyFileSync(join(fixtures, `${name}.json`), join(exports, `${name}.json`));
  }
  const movies = variant === 'live' ? liveMovies(root) : libraryMovies(root);
  const write = (name, value) =>
    writeFileSync(join(exports, `${name}.json`), JSON.stringify(value));
  write('movie', movies);
  write('tag', [
    { id: 1, label: 'kinder' },
    { id: 2, label: 'sammlung' },
  ]);
  write('rootfolder', [{ path: root }]);
  write('exclusions', []);
  if (variant === 'live') writeLiveFiles(root, movies);
  await run(binary, ['--config', config, 'import-radarr', '--exports', exports]);

  if (variant !== 'live') {
    const cache = join(dir, 'cache', 'images', 'movie');
    const key = createHash('sha256').update(CACHED_POSTER).digest('hex');
    for (const size of ['list', 'detail']) {
      mkdirSync(join(cache, size), { recursive: true });
      writeFileSync(join(cache, size, `${key}.png`), PNG);
    }
    const read = (name) => JSON.parse(readFileSync(join(fixtures, `${name}.json`), 'utf8'));
    const fixtureDocs = {
      qualityprofile: read('qualityprofile'),
      qualitydefinition: read('qualitydefinition'),
      customformat: read('customformat'),
    };
    const seriesRoot = slash(join(dir, 'media', 'serien'));
    const sonarr = fakeSonarr(fixtureDocs, seriesRoot, librarySeries(seriesRoot));
    await new Promise((resolve) => sonarr.listen(SONARR_PORTS[variant], '127.0.0.1', resolve));
    try {
      await run(binary, ['--config', config, 'import-sonarr']);
    } finally {
      sonarr.close();
    }
  }
}
