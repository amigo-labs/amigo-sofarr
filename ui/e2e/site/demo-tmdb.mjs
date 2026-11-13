// A fake TMDB, TVDB and IMDb ratings dataset for the website screenshots, answering from the
// fictional catalog in `demo-data.mjs`. The answer shapes follow the e2e fake
// (`../fake-tmdb.mjs`); this one has no test hooks but real-looking content.

import { randomUUID } from 'node:crypto';
import { createServer } from 'node:http';
import { gzipSync } from 'node:zlib';
import {
  GENRES,
  MOVIES,
  PEOPLE,
  SERIES,
  backdropPath,
  biographyOf,
  byPopularity,
  episodesOf,
  imdbOf,
  movieById,
  personById,
  photoPath,
  posterPath,
  seriesById,
} from './demo-data.mjs';

const PAGE = 20;
const TVDB_TOKEN = randomUUID();
const ALL = [...MOVIES, ...SERIES];

/** IMDb needs at least 100,000 titles in a first load: fillers plus the catalog's. */
function ratingsDataset() {
  const lines = ['tconst\taverageRating\tnumVotes'];
  for (let n = 1; n <= 100_000; n += 1) lines.push(`tt${String(n).padStart(7, '0')}\t6.0\t100`);
  for (const t of ALL) lines.push(`${imdbOf(t.id)}\t${t.rating.toFixed(1)}\t${t.votes}`);
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
    poster_path: posterPath(t),
    backdrop_path: backdropPath(t),
    popularity: t.popularity,
    genre_ids: t.genres,
    original_language: t.language,
    overview: t.overview,
  };
}

function page(list, number, mediaType) {
  const pages = Math.max(1, Math.ceil(list.length / PAGE));
  const results = list.slice((number - 1) * PAGE, number * PAGE).map((t) => entry(t, mediaType));
  return { page: number, total_pages: pages, total_results: list.length, results };
}

const genreObjects = (t) =>
  t.genres
    .map((id) => [...GENRES.movie, ...GENRES.tv].find((g) => g.id === id))
    .filter((g) => g !== undefined);

const ROLES = ['Hauptrolle', 'Kapitänin', 'Kommissar', 'Ärztin', 'Fremder', 'Archivarin'];

function credits(t) {
  const director = personById(t.director);
  return {
    cast: t.cast.map((id, i) => ({
      id,
      name: personById(id).name,
      character: ROLES[(t.id + i) % ROLES.length],
      profile_path: photoPath(personById(id)),
      order: i,
    })),
    crew: director
      ? [
          {
            id: director.id,
            name: director.name,
            job: 'Director',
            department: 'Directing',
            profile_path: photoPath(director),
          },
        ]
      : [],
  };
}

function movieDetail(t) {
  return {
    ...entry(t, false),
    imdb_id: imdbOf(t.id),
    runtime: t.runtime,
    tagline: t.tagline,
    status: t.date > new Date().toISOString().slice(0, 10) ? 'Post Production' : 'Released',
    genres: genreObjects(t),
    external_ids: { imdb_id: imdbOf(t.id) },
    credits: credits(t),
    videos: {
      results: [{ site: 'YouTube', type: 'Trailer', key: `demo${t.id}`, iso_639_1: 'de' }],
    },
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
    episode_run_time: [48],
    status: t.status,
    genres: genreObjects(t),
    number_of_seasons: t.seasons,
    seasons: Array.from({ length: t.seasons }, (_, i) => ({
      season_number: i + 1,
      name: `Staffel ${i + 1}`,
      episode_count: t.episodes,
      air_date: `${t.year + i}-09-01`,
    })),
    external_ids: { imdb_id: imdbOf(t.id), tvdb_id: t.tvdb },
    credits: credits(t),
  };
}

function personDetail(p) {
  const acted = ALL.filter((t) => t.cast.includes(p.id));
  const directed = ALL.filter((t) => t.director === p.id);
  return {
    id: p.id,
    name: p.name,
    biography: biographyOf(p.id) ?? `${p.name} stammt aus ${p.place.split(',')[0]}.`,
    birthday: p.born,
    deathday: null,
    place_of_birth: p.place,
    profile_path: photoPath(p),
    known_for_department: acted.length >= directed.length ? 'Acting' : 'Directing',
    combined_credits: {
      cast: acted.map((t, i) =>
        Object.assign(entry(t, true), { character: ROLES[i % ROLES.length] }),
      ),
      crew: directed.map((t) =>
        Object.assign(entry(t, true), { department: 'Directing', job: 'Director' }),
      ),
    },
  };
}

function discover(kind, query) {
  const list = kind === 'movie' ? MOVIES : SERIES;
  const genre = Number(query.get('with_genres') ?? NaN);
  const language = query.get('with_original_language');
  const from = query.get('primary_release_date.gte') ?? query.get('first_air_date.gte');
  const to = query.get('primary_release_date.lte') ?? query.get('first_air_date.lte');
  return byPopularity(
    list
      .filter((t) => Number.isNaN(genre) || t.genres.includes(genre))
      .filter((t) => language === null || t.language === language)
      .filter((t) => (from === null || t.date >= from) && (to === null || t.date <= to)),
  );
}

function search(text) {
  const q = text.toLowerCase();
  const people = PEOPLE.filter((p) => p.name.toLowerCase().includes(q));
  return {
    page: 1,
    total_pages: 1,
    results: [
      ...ALL.filter((t) => t.title.toLowerCase().includes(q)).map((t) => entry(t, true)),
      ...people.map((p) => ({
        id: p.id,
        media_type: 'person',
        name: p.name,
        profile_path: photoPath(p),
        known_for_department: 'Acting',
        known_for: ALL.filter((t) => t.cast.includes(p.id))
          .slice(0, 2)
          .map((t) => ({ title: t.title })),
      })),
    ],
  };
}

const today = () => new Date().toISOString().slice(0, 10);

/** The shelves' TMDB lists by path. */
function lists() {
  const released = MOVIES.filter((t) => t.date <= today());
  return {
    'trending/all/day': byPopularity(ALL),
    'trending/all/week': byPopularity(ALL).toReversed(),
    'movie/popular': byPopularity(MOVIES),
    'movie/upcoming': MOVIES.filter((t) => t.date > today()),
    'movie/now_playing': byPopularity(released).slice(0, 6),
    'tv/popular': byPopularity(SERIES),
  };
}

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
    return parts[2] === 'external_ids'
      ? { imdb_id: imdbOf(t.id), tvdb_id: t.tvdb }
      : seriesDetail(t);
  }
  if (parts[0] === 'person') {
    const p = personById(Number(parts[1]));
    if (p === undefined) return 404;
    return parts[2] === 'movie_credits' ? { cast: [], crew: [] } : personDetail(p);
  }
  if (parts[0] === 'genre') return { genres: GENRES[parts[1] === 'movie' ? 'movie' : 'tv'] };
  if (parts[0] === 'find') {
    const t = SERIES.find((s) => String(s.tvdb) === parts[1]);
    return { movie_results: [], tv_results: t ? [entry(t, false)] : [] };
  }
  if (path === 'search/multi') return search(query.get('query') ?? '');
  if (path === 'discover/movie' && query.has('with_release_type')) {
    return page(byPopularity(MOVIES.filter((t) => t.year >= 2025)), number, false);
  }
  if (path === 'discover/movie') return page(discover('movie', query), number, false);
  if (path === 'discover/tv') return page(discover('tv', query), number, false);
  const list = lists()[path];
  return list === undefined ? 404 : page(list, number, path.startsWith('trending'));
}

function tvdbEpisodes(t) {
  return episodesOf(t).map((e) => ({
    id: e.id,
    seasonNumber: e.season,
    number: e.number,
    name: e.title,
    aired: e.airDate,
  }));
}

function tvdb(method, path) {
  if (method === 'POST' && path === 'login') return { data: { token: TVDB_TOKEN } };
  const parts = path.split('/');
  const t = SERIES.find((s) => s.tvdb === Number(parts[1]));
  if (parts[0] !== 'series' || t === undefined) return 404;
  if (parts[2] === 'extended') {
    return {
      data: {
        id: t.tvdb,
        name: t.title,
        year: String(t.year),
        originalLanguage: 'deu',
        status: { name: t.status === 'Ended' ? 'Ended' : 'Continuing' },
        overview: t.overview,
        translations: { nameTranslations: [{ language: 'deu', name: t.title }] },
      },
    };
  }
  if (parts[2] === 'episodes')
    return { data: { episodes: tvdbEpisodes(t) }, links: { next: null } };
  return 404;
}

function send(response, body) {
  const status = typeof body === 'number' ? body : 200;
  response.writeHead(status, { 'content-type': 'application/json' });
  response.end(typeof body === 'number' ? '{}' : JSON.stringify(body));
}

/** Starts the fake on `port`; it runs until the returned server is closed. */
export function startDemoTmdb(port) {
  const ratings = ratingsDataset();
  const server = createServer((request, response) => {
    const url = new URL(request.url ?? '/', 'http://localhost');
    const path = url.pathname;
    if (path === '/imdb/title.ratings.tsv.gz') {
      response.writeHead(200, { 'content-type': 'application/gzip' });
      response.end(ratings);
    } else if (path.startsWith('/tmdb/3/')) {
      send(response, tmdb(path.slice('/tmdb/3/'.length), url.searchParams));
    } else if (path.startsWith('/tvdb/')) {
      send(response, tvdb(request.method ?? 'GET', path.slice('/tvdb/'.length)));
    } else {
      send(response, 404);
    }
  });
  return new Promise((resolve) => server.listen(port, '127.0.0.1', () => resolve(server)));
}
