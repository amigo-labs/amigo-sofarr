// The fictional catalog behind the website screenshots (`screenshots.mjs`): movies, series
// and people that do not exist, with generated artwork. Nothing here may name a real title
// or person. TMDB ids: movies 1xxxx, series 2xxxx, people 3xxxx; TVDB ids of series 4xxxx.

/** Artwork styles of `artwork.mjs`: two colours, an accent and a motif. */
const look = (from, to, accent, motif) => ({ from, to, accent, motif });

const movie = (id, title, year, fields) => ({
  kind: 'movie',
  id,
  title,
  original: fields.original ?? title,
  year,
  date: fields.date ?? `${year}-${fields.month ?? '04'}-12`,
  genres: fields.genres,
  language: fields.language ?? 'de',
  rating: fields.rating,
  votes: fields.votes ?? 24_000,
  runtime: fields.runtime ?? 112,
  overview: fields.overview,
  tagline: fields.tagline ?? null,
  director: fields.director,
  cast: fields.cast,
  look: fields.look,
  popularity: fields.popularity ?? 50,
  library: fields.library ?? null,
});

const series = (id, title, year, fields) => ({
  ...movie(id, title, year, fields),
  kind: 'series',
  tvdb: 40_000 + (id - 20_000),
  seasons: fields.seasons ?? 2,
  episodes: fields.episodes ?? 8,
  status: fields.status ?? 'Returning Series',
});

export const PEOPLE = [
  { id: 30_001, name: 'Lena Marquardt', born: '1984-02-11', place: 'Bremen, Deutschland', hue: 12 },
  { id: 30_002, name: 'Jonas Velde', born: '1979-07-23', place: 'Utrecht, Niederlande', hue: 210 },
  { id: 30_003, name: 'Ingrid Solberg', born: '1990-11-02', place: 'Bergen, Norwegen', hue: 280 },
  { id: 30_004, name: 'Mateo Ferraz', born: '1987-05-30', place: 'Porto, Portugal', hue: 30 },
  {
    id: 30_005,
    name: 'Clara Weißenfels',
    born: '1972-09-14',
    place: 'Leipzig, Deutschland',
    hue: 340,
  },
  { id: 30_006, name: 'Yusuf Demiral', born: '1993-01-19', place: 'Köln, Deutschland', hue: 160 },
  { id: 30_007, name: 'Henrike Aalto', born: '1981-03-08', place: 'Turku, Finnland', hue: 190 },
  { id: 30_008, name: 'Paul Brandtner', born: '1969-12-01', place: 'Graz, Österreich', hue: 45 },
];

const BIOGRAPHY = {
  30_001:
    'Lena Marquardt wuchs in Bremen auf und stand zuerst am Theater Bremen auf der Bühne. ' +
    'Bekannt wurde sie mit „Nordlicht über Tromsø“; seitdem pendelt sie zwischen Kino und Serie.',
  30_002:
    'Jonas Velde ist Schauspieler und Regisseur. Nach „Die letzte Fähre“ drehte er mit ' +
    '„Sternenfahrt“ seinen ersten eigenen Film.',
};

export const personById = (id) => PEOPLE.find((p) => p.id === id);
export const biographyOf = (id) => BIOGRAPHY[id] ?? null;

/** `library`: in the demo library with [quality, monitored] or null (only discoverable). */
export const MOVIES = [
  movie(10_001, 'Nordlicht über Tromsø', 2023, {
    genres: [18],
    rating: 7.6,
    votes: 48_210,
    overview:
      'Eine Meteorologin überwintert allein auf einer Messstation am Polarkreis – bis ein ' +
      'Fremder mit einem Funkgerät vor ihrer Tür steht.',
    tagline: 'Manche Nächte dauern Monate.',
    director: 30_002,
    cast: [30_001, 30_007, 30_008],
    look: look('#0b1d3a', '#1d5e6e', '#7cf2c2', 'aurora'),
    popularity: 92,
    library: ['Bluray-1080p', true],
  }),
  movie(10_002, 'Die letzte Fähre', 2021, {
    genres: [53],
    rating: 7.1,
    overview:
      'Auf der letzten Fähre des Winters verschwindet ein Passagier. Zwölf Menschen, eine ' +
      'Überfahrt, kein Netz.',
    director: 30_005,
    cast: [30_002, 30_006, 30_004],
    look: look('#1b1f24', '#3d4b57', '#f2b134', 'waves'),
    popularity: 74,
    library: ['WEBDL-2160p', true],
  }),
  movie(10_003, 'Kupferstadt', 2019, {
    genres: [80],
    rating: 6.9,
    overview:
      'Ein Ermittlerpaar folgt einer Spur aus gestohlenem Kupfer quer durch das Ruhrgebiet.',
    director: 30_008,
    cast: [30_006, 30_005],
    look: look('#2a1408', '#8a3b12', '#f6a15b', 'grid'),
    popularity: 41,
    library: ['Bluray-1080p', false],
  }),
  movie(10_004, 'Sternenfahrt', 2024, {
    original: 'Star Voyage',
    language: 'en',
    genres: [878],
    rating: 7.8,
    votes: 112_400,
    overview:
      'Die Besatzung eines Frachters findet zwischen Jupiter und Saturn ein Signal, das ' +
      'älter ist als die Menschheit.',
    tagline: 'Das Signal war immer schon da.',
    director: 30_002,
    cast: [30_003, 30_004, 30_001],
    look: look('#07061a', '#2b1a5c', '#ffd36e', 'planet'),
    popularity: 97,
    library: ['Remux-2160p', true],
  }),
  movie(10_005, 'Der Uhrmacher von Lyon', 2018, {
    genres: [18, 36],
    rating: 7.4,
    overview: 'Lyon, 1894: Ein Uhrmacher baut eine Uhr, die nie stehen bleiben soll.',
    director: 30_005,
    cast: [30_008, 30_003],
    look: look('#1e1a14', '#5a4630', '#e8c27a', 'gear'),
    popularity: 38,
    library: ['Bluray-1080p', false],
  }),
  movie(10_006, 'Salz und Asche', 2022, {
    genres: [12],
    rating: 6.7,
    overview: 'Zwei Schwestern durchqueren die Salzwüste, um das Erbe ihres Vaters zu finden.',
    director: 30_007,
    cast: [30_001, 30_003],
    look: look('#d9c7a7', '#9c6b45', '#2e1c12', 'dunes'),
    popularity: 55,
    library: ['WEBDL-1080p', true],
  }),
  movie(10_007, 'Nachtzug nach Triest', 2020, {
    genres: [53, 18],
    rating: 7.3,
    overview: 'Ein Schlafwagen, eine Grenze und ein Koffer, der nicht geöffnet werden darf.',
    director: 30_008,
    cast: [30_004, 30_005, 30_002],
    look: look('#120d1f', '#4a1f3d', '#ff7a6b', 'rails'),
    popularity: 63,
    library: ['Bluray-1080p', true],
  }),
  movie(10_008, 'Die Gärten von Kyoto', 2017, {
    genres: [99],
    rating: 8.1,
    votes: 6_420,
    overview: 'Ein Jahr mit den Gärtnern der alten Tempelgärten, von der Moosblüte bis zum Schnee.',
    director: 30_007,
    cast: [],
    look: look('#1d3020', '#5d8a4f', '#f4e8c1', 'sun'),
    popularity: 22,
    library: ['WEBDL-1080p', false],
  }),
  movie(10_009, 'Wellenbrecher', 2016, {
    genres: [18],
    rating: 6.8,
    overview: 'Ein Fischer und seine Tochter kämpfen um den letzten Kutter des Dorfes.',
    director: 30_005,
    cast: [30_008, 30_001],
    look: look('#0e2a3b', '#2f6f8f', '#e6f1f5', 'waves'),
    popularity: 30,
    library: [null, true],
  }),
  movie(10_010, 'Sommer in Aarhus', 2025, {
    genres: [35, 10749],
    rating: 7.0,
    overview: 'Ein Sommer, ein Fahrradladen und zwei Menschen, die sich nicht ausstehen können.',
    director: 30_007,
    cast: [30_006, 30_003],
    look: look('#ffb88c', '#de6262', '#fff3d6', 'sun'),
    popularity: 81,
    library: ['WEBDL-1080p', true],
  }),
  movie(10_011, 'Spiegelglas', 2024, {
    genres: [53],
    rating: 6.5,
    overview: 'Eine Architektin entdeckt in ihrem eigenen Neubau ein Stockwerk, das es nicht gibt.',
    director: 30_002,
    cast: [30_005, 30_006],
    look: look('#10141c', '#3a4a6b', '#9fd8ff', 'grid'),
    popularity: 69,
    library: [null, true],
  }),
  movie(10_012, 'Der zwölfte Mond', 2026, {
    genres: [878, 12],
    rating: 7.2,
    votes: 3_100,
    overview:
      'Auf dem zwölften Mond eines Gasriesen landet eine Expedition, die niemand gerufen hat.',
    director: 30_002,
    cast: [30_004, 30_003, 30_007],
    look: look('#050b18', '#1b3b6f', '#c9d6ff', 'planet'),
    popularity: 88,
    library: ['upcoming', true],
  }),
  movie(10_013, 'Rote Dünen', 2023, {
    genres: [12, 18],
    rating: 6.9,
    overview: 'Eine Karawane, ein Sandsturm und eine Karte, die sich über Nacht verändert.',
    director: 30_008,
    cast: [30_004, 30_001],
    look: look('#3b0d0c', '#c2410c', '#ffd8a8', 'dunes'),
    popularity: 47,
  }),
  movie(10_014, 'Polarkreis', 2019, {
    genres: [99],
    rating: 7.7,
    overview: 'Sechs Monate Dunkelheit, eine Forschungsstation und ein Kamerateam.',
    director: 30_007,
    cast: [],
    look: look('#0a1a2a', '#4a7a9a', '#ffffff', 'mountains'),
    popularity: 26,
  }),
  movie(10_015, 'Das stille Archiv', 2026, {
    genres: [9648],
    rating: 7.0,
    votes: 1_840,
    overview: 'Eine Archivarin findet einen Brief, der erst in zwanzig Jahren geschrieben wird.',
    director: 30_005,
    cast: [30_003, 30_008],
    look: look('#1a1612', '#4b3f33', '#d6c3a5', 'gear'),
    popularity: 72,
    library: ['cinema', true],
  }),
  movie(10_016, 'Hochland', 2022, {
    genres: [18],
    rating: 7.2,
    overview: 'Ein Bergbauernhof, drei Generationen und ein Winter, der alles entscheidet.',
    director: 30_008,
    cast: [30_005, 30_006],
    look: look('#1e2b22', '#6b8f71', '#f1efe2', 'mountains'),
    popularity: 35,
  }),
];

export const SERIES = [
  series(20_001, 'Hafenlichter', 2021, {
    genres: [80, 18],
    rating: 8.2,
    votes: 31_500,
    overview: 'Eine Kommissarin kehrt in ihre Hafenstadt zurück und ermittelt gegen alte Freunde.',
    director: 30_005,
    cast: [30_001, 30_006, 30_008],
    look: look('#08131f', '#1f4e6b', '#ffcf6b', 'lighthouse'),
    popularity: 90,
    seasons: 3,
    library: ['Bluray-1080p', true],
  }),
  series(20_002, 'Funkstille', 2023, {
    genres: [10765],
    rating: 7.9,
    overview:
      'Eine Kleinstadt verliert über Nacht jeden Empfang – und hört plötzlich etwas anderes.',
    director: 30_002,
    cast: [30_003, 30_004],
    look: look('#0d0d12', '#2e2e48', '#7af0ff', 'signal'),
    popularity: 85,
    seasons: 2,
    library: ['WEBDL-2160p', true],
  }),
  series(20_003, 'Das Haus am Fjord', 2020, {
    genres: [18],
    rating: 7.5,
    overview: 'Vier Geschwister erben ein Haus am Fjord und die Geheimnisse ihrer Mutter.',
    director: 30_007,
    cast: [30_007, 30_001, 30_002],
    look: look('#132a2a', '#3f7270', '#f0e6d2', 'mountains'),
    popularity: 64,
    seasons: 2,
    status: 'Ended',
    library: ['Bluray-1080p', false],
  }),
  series(20_004, 'Stadt der Uhren', 2024, {
    genres: [9648, 10765],
    rating: 7.3,
    overview: 'In einer Stadt, in der jede Uhr anders geht, verschwinden Stunden.',
    director: 30_005,
    cast: [30_008, 30_003],
    look: look('#1c140c', '#6b4a22', '#f5d48a', 'gear'),
    popularity: 58,
    seasons: 1,
    library: ['WEBDL-1080p', true],
  }),
  series(20_005, 'Weite Felder', 2022, {
    genres: [18],
    rating: 7.0,
    overview: 'Eine Landtierärztin, ein Dorf und zu viele offene Rechnungen.',
    director: 30_008,
    cast: [30_005, 30_006],
    look: look('#3a4a1c', '#a3a85a', '#fff7d6', 'sun'),
    popularity: 44,
    seasons: 2,
  }),
  series(20_006, 'Kaltes Wasser', 2025, {
    genres: [80],
    rating: 7.6,
    overview: 'Taucher der Wasserschutzpolizei finden im Bodensee ein Auto, das 1987 verschwand.',
    director: 30_002,
    cast: [30_006, 30_002],
    look: look('#041824', '#0f5a73', '#a6ecff', 'waves'),
    popularity: 79,
    seasons: 1,
  }),
];

export const GENRES = {
  movie: [
    { id: 12, name: 'Abenteuer' },
    { id: 18, name: 'Drama' },
    { id: 35, name: 'Komödie' },
    { id: 36, name: 'Historie' },
    { id: 53, name: 'Thriller' },
    { id: 80, name: 'Krimi' },
    { id: 99, name: 'Dokumentarfilm' },
    { id: 878, name: 'Science Fiction' },
    { id: 9648, name: 'Mystery' },
    { id: 10749, name: 'Liebesfilm' },
  ],
  tv: [
    { id: 18, name: 'Drama' },
    { id: 80, name: 'Krimi' },
    { id: 9648, name: 'Mystery' },
    { id: 10765, name: 'Sci-Fi & Fantasy' },
  ],
};

export const movieById = (id) => MOVIES.find((t) => t.id === id);
export const seriesById = (id) => SERIES.find((t) => t.id === id);
export const imdbOf = (id) => `tt8${String(id).padStart(6, '0')}`;
export const byPopularity = (list) => list.toSorted((a, b) => b.popularity - a.popularity);

/** The TMDB image paths of a title or person (all PNG, rendered by `artwork.mjs`). */
export const posterPath = (t) => `/${t.kind === 'movie' ? 'm' : 's'}${t.id}.png`;
export const backdropPath = (t) => `/b${t.id}.png`;
export const photoPath = (p) => `/p${p.id}.png`;
/** The TVDB artwork path of a series poster in the library. */
export const tvdbPosterPath = (t) => `/banners/demo/s${t.id}.png`;

const EPISODE_TITLES = [
  'Ankunft',
  'Ebbe',
  'Nebel',
  'Funken',
  'Spuren',
  'Treibgut',
  'Gegenwind',
  'Heimkehr',
];

const isoDay = (date) => date.toISOString().slice(0, 10);

/**
 * The episodes of a series, one a week. A running series' last season started 25 days before
 * `now`, so some episodes have aired and the rest fill the calendar.
 */
export function episodesOf(t, now = new Date()) {
  const running = t.status !== 'Ended';
  const out = [];
  for (let season = 1; season <= t.seasons; season += 1) {
    const last = season === t.seasons;
    // Each series airs on its own weekday.
    const start =
      running && last
        ? new Date(now.getTime() - (22 + (t.id % 5)) * 86_400_000)
        : new Date(Date.UTC(t.year + season - 1, 8, 1 + (t.id % 7)));
    for (let number = 1; number <= t.episodes; number += 1) {
      out.push({
        id: t.tvdb * 100 + season * 10 + number,
        season,
        number,
        title: EPISODE_TITLES[(number - 1 + t.id + season * 3) % EPISODE_TITLES.length],
        airDate: isoDay(new Date(start.getTime() + (number - 1) * 7 * 86_400_000)),
      });
    }
  }
  return out;
}
