// The demo library of the website screenshots, imported the way a real one is (as the e2e
// seeds do): a Radarr export for `import-radarr` and a fake Sonarr for `import-sonarr`.
// Generated artwork goes straight into sofarr's image cache, so nothing is loaded from the
// internet.

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { SONARR_KEY, fakeSonarr, run } from '../seed.mjs';
import { backdrop, portrait, poster } from './artwork.mjs';
import {
  MOVIES,
  PEOPLE,
  SERIES,
  backdropPath,
  episodesOf,
  imdbOf,
  personById,
  photoPath,
  posterPath,
  tvdbPosterPath,
} from './demo-data.mjs';

const TMDB_SIZES = ['w185', 'w342', 'w500', 'w780', 'h632', 'w1280'];
const GROUPS = ['NORDLICHT', 'KUTTER', 'FJORD', 'HAFEN', 'SALZ'];
const DAY = 86_400_000;

const slash = (path) => path.replaceAll('\\', '/');
const key = (path) => createHash('sha256').update(path).digest('hex');
const isoAt = (days) => new Date(Date.now() + days * DAY).toISOString();

/** Writes `png` into the image cache under `kind` in every size. */
function cache(dir, kind, sizes, path, png) {
  for (const size of sizes) {
    const folder = join(dir, 'cache', 'images', kind, size);
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(folder, `${key(path)}.png`), png);
  }
}

/** Renders every poster, backdrop and portrait into the cache, all at once. */
export async function renderArtwork(browser, dir) {
  const titles = [...MOVIES, ...SERIES].map(async (t) => {
    const director = personById(t.director);
    const credit =
      t.kind === 'series'
        ? `Mit ${personById(t.cast[0]).name}`
        : director
          ? `Ein Film von ${director.name}`
          : 'Ein Dokumentarfilm';
    const [png, wide] = await Promise.all([poster(browser, t, credit), backdrop(browser, t)]);
    cache(dir, 'tmdb', TMDB_SIZES, posterPath(t), png);
    if (t.kind === 'movie') cache(dir, 'movie', ['list', 'detail'], posterPath(t), png);
    else cache(dir, 'series', ['list', 'detail'], tvdbPosterPath(t), png);
    cache(dir, 'tmdb', TMDB_SIZES, backdropPath(t), wide);
  });
  const people = PEOPLE.map(async (p) =>
    cache(dir, 'tmdb', TMDB_SIZES, photoPath(p), await portrait(browser, p)),
  );
  await Promise.all([...titles, ...people]);
}

const SOURCE = {
  'Bluray-1080p': '1080p.BluRay.x264',
  'WEBDL-1080p': '1080p.WEB.h264',
  'WEBDL-2160p': '2160p.WEB.DV.HDR.h265',
  'Remux-2160p': '2160p.BluRay.REMUX.HEVC.DTS-HD.MA',
};

/** A scene-style release name: German dual language, so the custom formats score it. */
function sceneName(name, quality, group) {
  const dotted = name
    .replaceAll('ä', 'ae')
    .replaceAll('ö', 'oe')
    .replaceAll('ü', 'ue')
    .replaceAll('ß', 'ss')
    .replaceAll('ø', 'oe')
    .replaceAll(/[^A-Za-z0-9]+/g, '.');
  return `${dotted}.German.DL.${SOURCE[quality]}-${group}`;
}

function movieFile(t, quality, n) {
  const group = GROUPS[n % GROUPS.length];
  const uhd = quality.includes('2160p');
  return {
    relativePath: `${t.title} (${t.year}) ${quality}-${group}.mkv`,
    sceneName: sceneName(`${t.title} ${t.year}`, quality, group),
    size: Math.round((uhd ? 38 : 9) * 1e9 + n * 7.3e8),
    releaseGroup: group,
    quality: { quality: { name: quality }, revision: { version: 1, real: 0 } },
  };
}

/** A Radarr export entry of a library movie; `upcoming` and `cinema` are not out yet. */
function radarrMovie(t, n, root) {
  const [quality, monitored] = t.library;
  const dates =
    quality === 'upcoming'
      ? { status: 'announced', inCinemas: isoAt(3), digitalRelease: isoAt(40) }
      : quality === 'cinema'
        ? { status: 'inCinemas', inCinemas: isoAt(-9), digitalRelease: isoAt(5) }
        : {
            status: 'released',
            inCinemas: `${t.date}T00:00:00Z`,
            digitalRelease: `${t.year}-08-20T00:00:00Z`,
            physicalRelease: `${t.year}-10-02T00:00:00Z`,
          };
  const hasFile = quality !== null && quality !== 'upcoming' && quality !== 'cinema';
  return {
    id: n,
    tmdbId: t.id,
    imdbId: imdbOf(t.id),
    title: t.title,
    originalTitle: t.original,
    year: t.year,
    monitored,
    qualityProfileId: quality?.includes('2160p') ? 5 : 4,
    path: `${root}/${t.title} (${t.year})`,
    minimumAvailability: 'released',
    physicalRelease: null,
    ...dates,
    overview: t.overview,
    runtime: t.runtime,
    tags: [],
    alternateTitles: t.original === t.title ? [] : [{ title: t.original }],
    images: [
      { coverType: 'poster', remoteUrl: `https://image.tmdb.org/t/p/original${posterPath(t)}` },
    ],
    ...(hasFile ? { movieFile: movieFile(t, quality, n) } : {}),
  };
}

/** Series, episodes and files of the fake Sonarr: aired episodes have files. */
function sonarrData(root) {
  const series = [];
  const episodes = new Map();
  const files = new Map();
  const today = new Date().toISOString().slice(0, 10);
  let fileId = 0;
  SERIES.filter((t) => t.library !== null).forEach((t, i) => {
    const id = i + 1;
    const [quality, monitored] = t.library;
    series.push({
      id,
      tvdbId: t.tvdb,
      tmdbId: t.id,
      imdbId: imdbOf(t.id),
      title: t.title,
      year: t.year,
      monitored,
      qualityProfileId: quality.includes('2160p') ? 5 : 4,
      seasonFolder: true,
      seriesType: 'standard',
      path: `${root}/${t.title}`,
      tags: [],
      alternateTitles: [],
      images: [
        { coverType: 'poster', remoteUrl: `https://artworks.thetvdb.com${tvdbPosterPath(t)}` },
      ],
    });
    const list = [];
    const own = [];
    for (const e of episodesOf(t)) {
      // The newest aired episode of a running series is still missing.
      const aired = e.airDate < today;
      const latest = aired && e.season === t.seasons && t.status !== 'Ended' && e.number === 4;
      const fid = aired && !latest ? (fileId += 1) : 0;
      list.push({
        id: e.id,
        tvdbId: e.id,
        seasonNumber: e.season,
        episodeNumber: e.number,
        title: e.title,
        airDate: e.airDate,
        monitored,
        episodeFileId: fid,
      });
      if (fid !== 0) {
        const code = `S${String(e.season).padStart(2, '0')}E${String(e.number).padStart(2, '0')}`;
        own.push({
          id: fid,
          relativePath: `Season ${e.season}/${t.title} - ${code} - ${e.title} ${quality}.mkv`,
          sceneName: sceneName(`${t.title} ${code}`, quality, 'HAFEN'),
          size: 2_400_000_000 + (fid % 7) * 1.1e8,
          releaseGroup: 'HAFEN',
          quality: { quality: { name: quality }, revision: { version: 1, real: 0 } },
        });
      }
    }
    episodes.set(id, list);
    files.set(id, own);
  });
  return { series, episodes, files };
}

/** Imports the demo library into the database `config` points at. */
export async function seedLibrary({ binary, config, dir, fixtures, sonarrPort }) {
  const movies = slash(join(dir, 'media', 'filme'));
  const shows = slash(join(dir, 'media', 'serien'));
  const exports = join(dir, 'radarr-export');
  mkdirSync(exports, { recursive: true });
  const read = (name) => JSON.parse(readFileSync(join(fixtures, `${name}.json`), 'utf8'));
  // The fixture profiles want a custom format score of 50,000 for the cutoff, which no demo
  // file name reaches; the demo keeps only the quality cutoff.
  const docs = {
    qualityprofile: read('qualityprofile').map((p) => Object.assign(p, { cutoffFormatScore: 0 })),
    qualitydefinition: read('qualitydefinition'),
    customformat: read('customformat'),
  };
  const write = (name, value) =>
    writeFileSync(join(exports, `${name}.json`), JSON.stringify(value));
  for (const [name, value] of Object.entries(docs)) write(name, value);
  write(
    'movie',
    MOVIES.filter((t) => t.library !== null).map((t, i) => radarrMovie(t, i + 1, movies)),
  );
  write('tag', []);
  write('rootfolder', [{ path: movies }]);
  write('exclusions', []);
  await run(binary, ['--config', config, 'import-radarr', '--exports', exports]);

  const sonarr = fakeSonarr(docs, shows, sonarrData(shows));
  await new Promise((resolve) => sonarr.listen(sonarrPort, '127.0.0.1', resolve));
  try {
    await run(binary, ['--config', config, 'import-sonarr']);
  } finally {
    sonarr.close();
  }
}

export { SONARR_KEY };
