// The titles and people of the fake TMDB the discover e2e servers use (spec discover):
// a small catalog with named titles for the tests and fillers for paging. TMDB ids of
// movies are 5xx/7xx, of series 6xx, of people 9xxx.

/** IMDb ids of the catalog: `tt95` and the TMDB id. */
export const imdbOf = (id) => `tt95${String(id).padStart(5, '0')}`;

/** Movie and series details are answered only after the test released them (pending). */
export const HELD = { movie: [504, 507], series: [] };

/** A person TMDB never answers for: the page shows the German message (AC-14). */
export const UNREACHABLE_PERSON = 9999;

/** A series whose TVDB id TVDB does not answer for (AC-09). */
export const TVDB_DOWN = 88404;

function movie(id, title, year, popularity, fields = {}) {
  return {
    kind: 'movie',
    id,
    title,
    original: fields.original ?? title,
    date: `${year}-05-01`,
    popularity,
    genres: fields.genres ?? [18],
    language: fields.language ?? 'en',
    poster: fields.poster === null ? null : `/m${id}.png`,
    imdb: fields.imdb === null ? null : imdbOf(id),
    rating: fields.rating ?? 6.5,
    votes: fields.votes ?? 1000,
  };
}

function series(id, title, year, popularity, fields = {}) {
  return {
    ...movie(id, title, year, popularity, fields),
    kind: 'series',
    poster: fields.poster === null ? null : `/s${id}.png`,
    tvdb: fields.tvdb ?? null,
    seasons: fields.seasons ?? 1,
  };
}

const fillers = (from, count, make) => Array.from({ length: count }, (_, i) => make(from + i, i));

export const MOVIES = [
  movie(501, 'Das Boot', 1981, 90, {
    original: 'Das Boot',
    language: 'de',
    rating: 8.3,
    votes: 250_000,
  }),
  movie(502, 'Sternenfahrt', 2024, 95, { original: 'Star Voyage', rating: 7.8, votes: 12_345 }),
  movie(503, 'Ausgeschlossen', 2020, 40, { rating: 6.5 }),
  movie(504, 'Langsamer Film', 2023, 85, { rating: 5 }),
  movie(505, 'Ohne Bewertung', 2022, 30, { imdb: null, poster: null }),
  movie(506, 'Schwach bewertet', 2021, 29, { rating: 4 }),
  movie(507, 'Langsamer Kandidat', 2019, 99, { genres: [99], rating: 5 }),
  movie(508, 'Mit Ordner', 2018, 20, { rating: 7.1 }),
  movie(509, 'Doppelt', 2017, 19, { rating: 7.2 }),
  movie(510, 'Standardwerte', 2016, 18, { rating: 7.3 }),
  ...fillers(520, 60, (id, i) =>
    movie(id, `Beliebter Film ${String(i + 1).padStart(2, '0')}`, 2000 + (i % 25), 80 - i, {
      genres: i % 2 === 0 ? [18] : [35],
      rating: 6 + (i % 30) / 10,
    }),
  ),
  ...fillers(700, 30, (id, i) =>
    movie(id, `Doku ${String(i + 1).padStart(2, '0')}`, 2010 + (i % 10), 70 - i, {
      genres: [99],
      rating: 7 + (i % 10) / 10,
    }),
  ),
];

export const SERIES = [
  series(601, 'Großes Meer', 2020, 88, { tvdb: 88001, seasons: 2, rating: 8 }),
  series(602, 'Ohne TVDB', 2021, 60, { rating: 6.8 }),
  series(603, 'Kleine Insel', 2020, 70, { tvdb: 70001, rating: 7 }),
  series(604, 'TVDB Stumm', 2022, 50, { tvdb: TVDB_DOWN, rating: 6.1 }),
  series(605, 'Sternstunden', 2019, 45, { tvdb: 88005, rating: 7.4 }),
  ...fillers(620, 40, (id, i) =>
    series(id, `Beliebte Serie ${String(i + 1).padStart(2, '0')}`, 2005 + (i % 20), 40 - i / 2, {
      rating: 6 + (i % 25) / 10,
    }),
  ),
];

const byId = (list, id) => list.find((t) => t.id === id);
export const movieById = (id) => byId(MOVIES, id);
export const seriesById = (id) => byId(SERIES, id);
const pick = (kind, ids) => ids.map((id) => (kind === 'movie' ? movieById(id) : seriesById(id)));

/** The shelves' TMDB lists by path. */
export const LISTS = {
  'trending/all/day': [
    ...pick('movie', [501, 504]),
    ...pick('series', [603]),
    ...pick('movie', [502]),
    ...pick('series', [601]),
    ...pick(
      'movie',
      fillers(520, 20, (id) => id),
    ),
  ],
  'trending/all/week': [...pick('movie', [502, 505, 506]), ...pick('series', [601, 605])],
  'movie/popular': [
    ...pick('movie', [502, 503]),
    ...pick(
      'movie',
      fillers(520, 60, (id) => id),
    ),
  ],
  'movie/upcoming': pick('movie', [510, 509]),
  'movie/now_playing': pick('movie', [508, 503]),
  'discover/movie?new': pick('movie', [505, 506]),
  'tv/popular': [
    ...pick('series', [601, 602, 603, 604, 605]),
    ...pick(
      'series',
      fillers(620, 40, (id) => id),
    ),
  ],
};

const person = (id, name, department, fields = {}) => ({
  id,
  name,
  department,
  biography: fields.biography ?? null,
  birthday: fields.birthday ?? null,
  place: fields.place ?? null,
  photo: fields.photo === null ? null : `/p${id}.png`,
  cast: fields.cast ?? [],
  crew: fields.crew ?? [],
});

/** Cast of "Das Boot": 12 people, the first billed first. */
export const BOOT_CAST = fillers(9000, 12, (id, i) => ({
  id,
  name: i === 0 ? 'Anna Beispiel' : `Darsteller ${String(i + 1).padStart(2, '0')}`,
  character: `Rolle ${i + 1}`,
}));

export const PEOPLE = [
  person(9000, 'Anna Beispiel', 'Acting', {
    biography: 'Anna Beispiel ist eine Schauspielerin aus Hamburg.',
    birthday: '1970-03-01',
    place: 'Hamburg, Deutschland',
    cast: [
      ...pick('movie', [501, 502, 503, 505]).map((t, i) => ({
        title: t,
        character: `Rolle ${i + 1}`,
      })),
      { title: seriesById(601), character: 'Kapitänin' },
      ...pick(
        'movie',
        fillers(520, 60, (id) => id),
      ).map((t) => ({ title: t, character: 'Gast' })),
    ],
    crew: [
      { title: movieById(502), department: 'Directing', job: 'Director' },
      { title: movieById(502), department: 'Writing', job: 'Screenplay' },
    ],
  }),
  ...BOOT_CAST.slice(1).map((c) =>
    person(c.id, c.name, 'Acting', { cast: [{ title: movieById(501), character: c.character }] }),
  ),
  person(9100, 'Clara Kamera', 'Camera', {
    crew: [
      { title: movieById(501), department: 'Camera', job: 'Director of Photography' },
      { title: movieById(502), department: 'Camera', job: 'Director of Photography' },
    ],
  }),
  person(9200, 'Stella Stern', 'Acting', {
    cast: [{ title: movieById(502), character: 'Stella' }],
  }),
];

export const GENRES = {
  movie: [
    { id: 18, name: 'Drama' },
    { id: 35, name: 'Komödie' },
    { id: 99, name: 'Dokumentarfilm' },
  ],
  tv: [
    { id: 18, name: 'Drama' },
    { id: 10765, name: 'Sci-Fi & Fantasy' },
  ],
};

/** Every TMDB image path of the catalog: the e2e servers have them in their image cache. */
export function imagePaths() {
  const paths = [];
  for (const t of [...MOVIES, ...SERIES]) {
    if (t.poster !== null) paths.push(t.poster);
    paths.push(`/b${t.id}.png`);
  }
  for (const p of PEOPLE) if (p.photo !== null) paths.push(p.photo);
  return paths;
}
