import { describe, expect, it } from 'vitest';
import type { CalendarEntry } from '../api/generated/CalendarEntry';
import type { MovieListItem } from '../api/generated/MovieListItem';
import type { SeriesListItem } from '../api/generated/SeriesListItem';
import { ROW_SIZE, newestTiles, rowQuery, upcomingTiles } from './home';

const TODAY = '2026-11-24';

const entry = (fields: Partial<CalendarEntry>): CalendarEntry => ({
  date: TODAY,
  kind: 'movie',
  release: 'cinema',
  movie_id: 1,
  series_id: null,
  episode_id: null,
  season: null,
  episode: null,
  title: 'Nordlicht',
  episode_title: null,
  year: 2026,
  state: 'upcoming',
  monitored: true,
  has_poster: true,
  ...fields,
});

const episode = (date: string, season: number, number: number): CalendarEntry =>
  entry({
    date,
    kind: 'episode',
    release: null,
    movie_id: null,
    series_id: 9,
    episode_id: season * 100 + number,
    season,
    episode: number,
    title: 'Kleine Insel',
    year: 2019,
    has_poster: false,
  });

const movie = (id: number, added: string, fields: Partial<MovieListItem> = {}): MovieListItem => ({
  id,
  tmdb_id: id,
  title: `Film ${id}`,
  title_de: null,
  original_title: null,
  year: 2001,
  monitored: true,
  quality: null,
  size: null,
  missing: true,
  cutoff_unmet: false,
  available: true,
  has_poster: false,
  has_backdrop: false,
  quality_profile_id: 1,
  tags: [],
  added_at: added,
  imdb_rating: null,
  imdb_votes: null,
  ...fields,
});

const series = (id: number, added: string): SeriesListItem => ({
  id,
  tvdb_id: id,
  title: `Serie ${id}`,
  title_de: null,
  year: 2019,
  monitored: false,
  series_type: 'standard',
  episodes: 3,
  monitored_episodes: 3,
  files: 1,
  missing: 2,
  cutoff_unmet: 0,
  complete: false,
  size: 0,
  has_poster: true,
  has_backdrop: false,
  quality_profile_id: 1,
  tags: [],
  added_at: added,
  imdb_rating: null,
  imdb_votes: null,
});

describe('„Demnächst“ (AC-01, AC-02)', () => {
  it('shows a series once, at the day and code of its soonest episode', () => {
    const tiles = upcomingTiles(
      [episode('2026-11-26', 3, 6), episode('2026-11-25', 3, 5), episode('2026-11-25', 3, 4)],
      TODAY,
    );
    expect(tiles).toHaveLength(1);
    expect(tiles[0]).toMatchObject({
      kind: 'series',
      id: 9,
      href: '/serien/9',
      hasPoster: false,
      when: 'Morgen · S03E04',
      name: 'Kleine Insel, 2019, Morgen, S03E04',
    });
  });

  it('shows a movie once with its earlier date, soonest title first', () => {
    const tiles = upcomingTiles(
      [
        entry({ date: '2026-11-26', release: 'cinema' }),
        episode('2026-11-25', 1, 1),
        entry({ date: '2026-11-24', release: 'digital' }),
        entry({ date: '2026-11-26', movie_id: 2, title: 'Später', has_poster: false }),
      ],
      TODAY,
    );
    expect(tiles.map((t) => [t.title, t.when])).toEqual([
      ['Nordlicht', 'Heute'],
      ['Kleine Insel', 'Morgen · S01E01'],
      ['Später', 'Donnerstag, 26.11.'],
    ]);
    expect(tiles[0]?.name).toBe('Nordlicht, 2026, Heute');
    expect(tiles[0]?.href).toBe('/filme/1');
  });

  it(`keeps at most ${ROW_SIZE} titles`, () => {
    const many = Array.from({ length: 30 }, (_, i) => entry({ movie_id: i + 1 }));
    expect(upcomingTiles(many, TODAY)).toHaveLength(ROW_SIZE);
  });
});

describe('„Zuletzt hinzugefügt“ and „Fehlt noch“ (AC-01)', () => {
  it('merges movies and series, newest added first, at most 20', () => {
    const movies = Array.from({ length: 20 }, (_, i) =>
      movie(i + 1, `2026-11-${String(i + 1).padStart(2, '0')}T10:00:00Z`),
    );
    const tiles = newestTiles(movies, [series(5, '2026-11-15T12:00:00Z')]);
    expect(tiles).toHaveLength(ROW_SIZE);
    expect(tiles[0]?.key).toBe('movie/20');
    expect(tiles.map((t) => t.key).indexOf('series/5')).toBe(5);
    expect(tiles.at(-1)?.key).toBe('movie/2');
  });

  it('carries the marks and dims unmonitored titles as the library does', () => {
    const [m, s] = newestTiles(
      [movie(1, '2026-11-02T00:00:00Z')],
      [series(2, '2026-11-01T00:00:00Z')],
    );
    expect(m).toMatchObject({ mark: { text: 'fehlt' }, dimmed: false, name: 'Film 1, 2001' });
    expect(s).toMatchObject({ mark: { text: '2 fehlen' }, dimmed: true, href: '/serien/2' });
  });

  it('asks the list API for one page of the newest', () => {
    expect(rowQuery(false)).toBe('?sort=added&order=desc&page=1&page_size=20');
    expect(rowQuery(true)).toBe('?missing=yes&sort=added&order=desc&page=1&page_size=20');
  });
});
