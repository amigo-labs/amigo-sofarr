// The discover e2e servers (spec discover): `discover` with a fake TMDB, TVDB and IMDb
// dataset (`fake-tmdb.mjs`) and a small library imported the real way (a Radarr export and
// a fake Sonarr, as in `seed.mjs`); `discoverPlain` with the fake TMDB and ratings off.
// TMDB images are put into sofarr's image cache, so no image is loaded from the internet.

import { createHash, randomUUID } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { imagePaths } from './discover-data.mjs';
import { portBase } from './env.mjs';
import { startFakeTmdb } from './fake-tmdb.mjs';
import { SONARR_KEY, fakeSonarr, run } from './seed.mjs';

/** The fake TMDB of each discover variant, and the fake Sonarr of the import. */
export const FAKE_PORTS = { discover: portBase() + 30, discoverPlain: portBase() + 31 };
const SONARR_PORT = portBase() + 32;

/** A 1x1 PNG for every cached image. */
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
);

const slash = (path) => path.replaceAll('\\', '/');

/** The config lines of a discover variant; API keys are made per run. */
export function discoverConfig(variant) {
  const fake = `http://127.0.0.1:${FAKE_PORTS[variant]}`;
  const lines = [
    '[tmdb]',
    `api_key = "e2e-${randomUUID()}"`,
    `url = "${fake}/tmdb/3"`,
    '[imdb]',
    `url = "${fake}/imdb/title.ratings.tsv.gz"`,
  ];
  if (variant === 'discoverPlain') return [...lines, 'enabled = false'];
  return [
    ...lines,
    '[tvdb]',
    `api_key = "e2e-${randomUUID()}"`,
    `url = "${fake}/tvdb"`,
    '[sonarr]',
    `url = "http://127.0.0.1:${SONARR_PORT}"`,
    `api_key = "${SONARR_KEY}"`,
  ];
}

/** The cache file name of a provider path, as sofarr's image cache names it. */
const key = (path) => createHash('sha256').update(path).digest('hex');

/** Writes `bytes` for every catalog image into the image cache, in every size. */
function cacheImages(dir) {
  const write = (kind, size, path) => {
    const folder = join(dir, 'cache', 'images', kind, size);
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(folder, `${key(path)}.png`), PNG);
  };
  for (const path of imagePaths()) {
    for (const size of ['w185', 'w342', 'w500', 'w780', 'h632', 'w1280']) write('tmdb', size, path);
    if (path.startsWith('/m')) for (const size of ['list', 'detail']) write('movie', size, path);
  }
}

function radarrMovie(id, tmdbId, title, year, root) {
  return {
    id,
    tmdbId,
    title,
    originalTitle: title,
    year,
    monitored: true,
    qualityProfileId: 4,
    path: `${root}/${title} (${year})`,
    minimumAvailability: 'released',
    status: 'released',
    inCinemas: `${year}-03-01T00:00:00Z`,
    tags: [],
    alternateTitles: [],
    images: [
      { coverType: 'poster', remoteUrl: `https://image.tmdb.org/t/p/original/m${tmdbId}.png` },
    ],
  };
}

/** The library of `discover`: "Das Boot" (TMDB 501), "Ausgeschlossen" (503) excluded,
 * the series "Kleine Insel" (TVDB 70001) without TMDB id; a folder "Mit Ordner (2018)"
 * with a file that adding the movie takes up (AC-07). */
async function seedLibrary({ binary, config, dir, fixtures }) {
  const movies = slash(join(dir, 'media', 'filme'));
  const shows = slash(join(dir, 'media', 'serien'));
  mkdirSync(join(movies, 'Mit Ordner (2018)'), { recursive: true });
  writeFileSync(
    join(movies, 'Mit Ordner (2018)', 'Mit.Ordner.2018.1080p.BluRay.x264-GRP.mkv'),
    'e2e',
  );
  mkdirSync(shows, { recursive: true });
  const exports = join(dir, 'radarr-export');
  mkdirSync(exports, { recursive: true });
  for (const name of ['qualityprofile', 'qualitydefinition', 'customformat']) {
    copyFileSync(join(fixtures, `${name}.json`), join(exports, `${name}.json`));
  }
  const write = (name, value) =>
    writeFileSync(join(exports, `${name}.json`), JSON.stringify(value));
  write('movie', [radarrMovie(1, 501, 'Das Boot', 1981, movies)]);
  write('tag', [{ id: 1, label: 'kinder' }]);
  write('rootfolder', [{ path: movies }]);
  write('exclusions', [{ tmdbId: 503, movieTitle: 'Ausgeschlossen', movieYear: 2020 }]);
  await run(binary, ['--config', config, 'import-radarr', '--exports', exports]);

  const read = (name) => JSON.parse(readFileSync(join(fixtures, `${name}.json`), 'utf8'));
  const docs = {
    qualityprofile: read('qualityprofile'),
    qualitydefinition: read('qualitydefinition'),
    customformat: read('customformat'),
  };
  const series = [
    {
      id: 1,
      tvdbId: 70001,
      title: 'Kleine Insel',
      year: 2020,
      monitored: true,
      qualityProfileId: 4,
      seasonFolder: true,
      seriesType: 'standard',
      path: `${shows}/Kleine Insel`,
      tags: [],
      alternateTitles: [],
      images: [],
    },
  ];
  const episodes = new Map([
    [
      1,
      [1, 2].map((n) => ({
        id: n,
        tvdbId: 700_010 + n,
        seasonNumber: 1,
        episodeNumber: n,
        title: `Folge ${n}`,
        airDate: '2020-01-0' + n,
        monitored: true,
        episodeFileId: 0,
      })),
    ],
  ]);
  const sonarr = fakeSonarr(docs, shows, { series, episodes, files: new Map() });
  await new Promise((resolve) => sonarr.listen(SONARR_PORT, '127.0.0.1', resolve));
  try {
    await run(binary, ['--config', config, 'import-sonarr']);
  } finally {
    sonarr.close();
  }
}

/** Seeds a discover variant and starts its fake TMDB, which runs as long as the server. */
export async function seedDiscover(variant, context) {
  cacheImages(context.dir);
  if (variant === 'discover') await seedLibrary(context);
  await startFakeTmdb(FAKE_PORTS[variant]);
}
