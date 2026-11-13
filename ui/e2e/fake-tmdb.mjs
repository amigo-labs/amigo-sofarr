// A fake TMDB, TVDB and IMDb ratings dataset for the discover e2e servers (spec discover):
// sofarr's `[tmdb] url`, `[tvdb] url` and `[imdb] url` point here. Answers come from the
// catalog in `discover-data.mjs`. Details of titles in `HELD` wait until a test releases
// them (`POST /control/release/{movie|tv}/{id}`), so their rating stays "pending" until then.

import { randomUUID } from 'node:crypto';
import { createServer } from 'node:http';
import { gzipSync } from 'node:zlib';
import {
  BOOT_CAST,
  GENRES,
  HELD,
  LISTS,
  MOVIES,
  PEOPLE,
  SERIES,
  TVDB_DOWN,
  UNREACHABLE_PERSON,
  movieById,
  seriesById,
} from './discover-data.mjs';

const PAGE = 20;
/** TVDB's login token, made per run. */
const TVDB_TOKEN = randomUUID();

/** IMDb needs at least 100,000 titles in a first load; fillers plus the catalog's. */
function ratingsDataset() {
  const lines = ['tconst\taverageRating\tnumVotes'];
  for (let n = 1; n <= 100_000; n += 1) lines.push(`tt${String(n).padStart(7, '0')}\t6.0\t100`);
  for (const t of [...MOVIES, ...SERIES]) {
    if (t.imdb !== null) lines.push(`${t.imdb}\t${t.rating.toFixed(1)}\t${t.votes}`);
  }
  return gzipSync(`${lines.join('\n')}\n`);
}

function entry(t, mediaType) {
  const names =
    t.kind === 'movie'
      ? { title: t.title, original_title: t.original, release_date: t.date }
      : { name: t.title, original_name: t.original, first_air_date: t.date };
  return {
    id: t.id,
    ...names,
    ...(mediaType ? { media_type: t.kind === 'movie' ? 'movie' : 'tv' } : {}),
    poster_path: t.poster,
    popularity: t.popularity,
    genre_ids: t.genres,
    original_language: t.language,
  };
}

function page(list, number, mediaType) {
  const pages = Math.max(1, Math.ceil(list.length / PAGE));
  const results = list.slice((number - 1) * PAGE, number * PAGE).map((t) => entry(t, mediaType));
  return { page: number, total_pages: pages, total_results: list.length, results };
}

function genreObjects(t) {
  const all = [...GENRES.movie, ...GENRES.tv];
  return t.genres.map((id) => all.find((g) => g.id === id)).filter((g) => g !== undefined);
}

function movieDetail(t) {
  const cast =
    t.id === 501 ? BOOT_CAST : [{ id: 9000, name: 'Anna Beispiel', character: 'Rolle 1' }];
  return {
    ...entry(t, false),
    imdb_id: t.imdb,
    runtime: 105,
    status: 'Released',
    backdrop_path: `/b${t.id}.png`,
    overview: `${t.title}: ein Film zum Testen.`,
    genres: genreObjects(t),
    external_ids: { imdb_id: t.imdb },
    credits: {
      cast: cast.map((c) => Object.assign({ profile_path: `/p${c.id}.png` }, c)),
      crew: [
        { id: 9100, name: 'Clara Kamera', job: 'Director of Photography', department: 'Camera' },
      ],
    },
    videos: { results: [{ site: 'YouTube', type: 'Trailer', key: `e2e${t.id}`, iso_639_1: 'de' }] },
    release_dates: {
      results: [
        { iso_3166_1: 'DE', release_dates: [{ type: 3, release_date: `${t.date}T00:00:00.000Z` }] },
      ],
    },
  };
}

function seriesDetail(t) {
  return {
    ...entry(t, false),
    overview: `${t.title}: eine Serie zum Testen.`,
    episode_run_time: [45],
    status: 'Returning Series',
    backdrop_path: `/b${t.id}.png`,
    genres: genreObjects(t),
    seasons: Array.from({ length: t.seasons + 1 }, (_, n) => ({
      season_number: n,
      name: n === 0 ? 'Specials' : `Staffel ${n}`,
      episode_count: 2,
      air_date: `${2020 + n}-01-01`,
    })),
    external_ids: { imdb_id: t.imdb, tvdb_id: t.tvdb },
    credits: {
      cast: [
        { id: 9000, name: 'Anna Beispiel', character: 'Kapitänin', profile_path: '/p9000.png' },
      ],
      crew: [],
    },
  };
}

const credit = (c, extra) => ({ ...entry(c.title, true), ...extra });

function personDetail(p) {
  return {
    id: p.id,
    name: p.name,
    biography: p.biography,
    birthday: p.birthday,
    deathday: null,
    place_of_birth: p.place,
    profile_path: p.photo,
    known_for_department: p.department,
    combined_credits: {
      cast: p.cast.map((c) => credit(c, { character: c.character })),
      crew: p.crew.map((c) => credit(c, { department: c.department, job: c.job })),
    },
  };
}

/** The filtered result list: `discover/{movie|tv}` with genre, language and year range. */
function discover(kind, query) {
  const list = kind === 'movie' ? MOVIES : SERIES;
  const genre = Number(query.get('with_genres') ?? NaN);
  const language = query.get('with_original_language');
  const from = query.get('primary_release_date.gte') ?? query.get('first_air_date.gte');
  const to = query.get('primary_release_date.lte') ?? query.get('first_air_date.lte');
  return list
    .filter((t) => Number.isNaN(genre) || t.genres.includes(genre))
    .filter((t) => language === null || t.language === language)
    .filter((t) => (from === null || t.date >= from) && (to === null || t.date <= to))
    .toSorted((a, b) => b.popularity - a.popularity);
}

function search(text) {
  const q = text.toLowerCase();
  const hits = [...MOVIES, ...SERIES].filter((t) => t.title.toLowerCase().includes(q));
  const people = PEOPLE.filter((p) => p.name.toLowerCase().includes(q));
  return {
    page: 1,
    total_pages: 1,
    results: [
      ...hits.map((t) => entry(t, true)),
      ...people.map((p) => ({
        id: p.id,
        media_type: 'person',
        name: p.name,
        profile_path: p.photo,
        known_for_department: p.department,
        known_for: p.cast.slice(0, 2).map((c) => ({ title: c.title.title })),
      })),
    ],
  };
}

/** The TMDB answer of a path, or a status number. */
function tmdb(path, query) {
  const number = Number(query.get('page') ?? '1');
  const parts = path.split('/');
  if (parts[0] === 'movie' && /^\d+$/.test(parts[1] ?? '')) {
    const t = movieById(Number(parts[1]));
    return t === undefined ? 404 : movieDetail(t);
  }
  if (parts[0] === 'tv' && /^\d+$/.test(parts[1] ?? '')) {
    const t = seriesById(Number(parts[1]));
    if (t === undefined) return 404;
    return parts[2] === 'external_ids' ? { imdb_id: t.imdb, tvdb_id: t.tvdb } : seriesDetail(t);
  }
  if (parts[0] === 'person') {
    const id = Number(parts[1]);
    if (id === UNREACHABLE_PERSON) return 503;
    const p = PEOPLE.find((x) => x.id === id);
    if (p === undefined) return 404;
    return parts[2] === 'movie_credits' ? { cast: [], crew: [] } : personDetail(p);
  }
  if (parts[0] === 'genre') return { genres: GENRES[parts[1] === 'movie' ? 'movie' : 'tv'] };
  if (parts[0] === 'find') return { movie_results: [], tv_results: [] };
  if (path === 'search/multi') return search(query.get('query') ?? '');
  if (path === 'discover/movie' && query.has('with_release_type')) {
    return page(LISTS['discover/movie?new'], number, false);
  }
  if (path === 'discover/movie') return page(discover('movie', query), number, false);
  if (path === 'discover/tv') return page(discover('tv', query), number, false);
  const list = LISTS[path];
  return list === undefined ? 404 : page(list, number, path.startsWith('trending'));
}

const TVDB_SERIES = new Map(SERIES.filter((t) => t.tvdb !== null).map((t) => [t.tvdb, t]));

function tvdb(method, path) {
  const parts = path.split('/');
  if (method === 'POST' && path === 'login') return { data: { token: TVDB_TOKEN } };
  const id = Number(parts[1]);
  if (id === TVDB_DOWN) return 503;
  const t = TVDB_SERIES.get(id);
  if (parts[0] !== 'series' || t === undefined) return 404;
  if (parts[2] === 'extended') {
    return {
      data: {
        id,
        name: t.title,
        year: t.date.slice(0, 4),
        originalLanguage: 'eng',
        status: { name: 'Continuing' },
        overview: `${t.title}: eine Serie zum Testen.`,
        translations: { nameTranslations: [{ language: 'deu', name: t.title }] },
      },
    };
  }
  if (parts[2] === 'episodes' && parts[4] !== undefined) return { data: { episodes: [] } };
  if (parts[2] === 'episodes') {
    const episodes = [];
    for (let season = 0; season <= t.seasons; season += 1) {
      for (let number = 1; number <= 2; number += 1) {
        episodes.push({
          id: id * 100 + season * 10 + number,
          seasonNumber: season,
          number,
          name: `Folge ${number}`,
          aired: `${2020 + season}-01-0${number}`,
        });
      }
    }
    return { data: { episodes }, links: { next: null } };
  }
  return 404;
}

/** Answers JSON, or an empty error answer for a status number. */
function send(response, body) {
  if (typeof body === 'number') {
    response.writeHead(body, { 'content-type': 'application/json' });
    response.end('{}');
  } else {
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end(JSON.stringify(body));
  }
}

/** Starts the fake on `port`; it runs as long as the process. */
export function startFakeTmdb(port) {
  const ratings = ratingsDataset();
  const held = new Set([
    ...HELD.movie.map((id) => `movie/${id}`),
    ...HELD.series.map((id) => `tv/${id}`),
  ]);
  const waiting = new Map();
  const server = createServer((request, response) => {
    const url = new URL(request.url ?? '/', 'http://localhost');
    const path = url.pathname;
    if (path === '/imdb/title.ratings.tsv.gz') {
      response.writeHead(200, { 'content-type': 'application/gzip' });
      response.end(ratings);
    } else if (request.method === 'POST' && path.startsWith('/control/release/')) {
      const key = path.slice('/control/release/'.length);
      held.delete(key);
      for (const answer of waiting.get(key) ?? []) answer();
      waiting.delete(key);
      send(response, { ok: true });
    } else if (path.startsWith('/tmdb/3/')) {
      const rest = path.slice('/tmdb/3/'.length);
      const key = rest.split('/').slice(0, 2).join('/');
      const answer = () => send(response, tmdb(rest, url.searchParams));
      if (held.has(key)) waiting.set(key, [...(waiting.get(key) ?? []), answer]);
      else answer();
    } else if (path.startsWith('/tvdb/')) {
      send(response, tvdb(request.method ?? 'GET', path.slice('/tvdb/'.length)));
    } else {
      send(response, 404);
    }
  });
  return new Promise((resolve) => server.listen(port, '127.0.0.1', () => resolve(server)));
}
