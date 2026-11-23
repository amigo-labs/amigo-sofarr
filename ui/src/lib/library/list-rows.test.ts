import { describe, expect, it } from 'vitest';
import type { MovieListItem } from '../api/generated/MovieListItem';
import type { SeriesListItem } from '../api/generated/SeriesListItem';
import { EMPTY_FILTERS, parseFilters } from './list-query';
import {
  LIST_ROWS,
  isPlain,
  keepsRows,
  listRowHref,
  listRowQuery,
  listRowTiles,
  type ListRowId,
} from './list-rows';

const rowOf = (id: ListRowId) => {
  const row = LIST_ROWS.find((r) => r.id === id);
  if (row === undefined) throw new Error(id);
  return row;
};

const movie = (id: number, fields: Partial<MovieListItem> = {}): MovieListItem => ({
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
  added_at: '2026-11-01T10:00:00Z',
  imdb_rating: null,
  imdb_votes: null,
  ...fields,
});

const series = (id: number, fields: Partial<SeriesListItem> = {}): SeriesListItem => ({
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
  added_at: '2026-11-01T10:00:00Z',
  imdb_rating: null,
  imdb_votes: null,
  ...fields,
});

describe('the rows (AC-01, AC-02)', () => {
  it('come in this order, „Am besten bewertet“ only with ratings', () => {
    expect(LIST_ROWS.map((r) => r.title)).toEqual([
      'Zuletzt hinzugefügt',
      'Fehlt noch',
      'Unter Cutoff',
      'Am besten bewertet',
    ]);
    expect(LIST_ROWS.filter((r) => r.ratings).map((r) => r.id)).toEqual(['rated']);
  });

  it('each ask the list API for one page of 20', () => {
    expect(listRowQuery('movie', rowOf('added'))).toBe(
      '?sort=added&order=desc&page=1&page_size=20',
    );
    expect(listRowQuery('movie', rowOf('missing'))).toBe(
      '?missing=yes&sort=added&order=desc&page=1&page_size=20',
    );
    expect(listRowQuery('series', rowOf('cutoff'))).toBe(
      '?cutoff_unmet=yes&sort=added&order=desc&page=1&page_size=20',
    );
    expect(listRowQuery('series', rowOf('rated'))).toBe(
      '?sort=rating&order=asc&page=1&page_size=20',
    );
  });

  it('„Am besten bewertet“ leaves out the titles without a rating', () => {
    const tiles = listRowTiles(
      rowOf('rated'),
      [
        movie(1, { imdb_rating: 9.1, imdb_votes: 250_000 }),
        movie(2, { imdb_rating: 7.5, imdb_votes: 12_000 }),
        movie(3),
      ],
      true,
    );
    expect(tiles.map((t) => t.key)).toEqual(['movie/1', 'movie/2']);
    expect(tiles[0]?.name).toBe('Film 1, 2001, IMDb 9,1 (250.000 Stimmen)');
    expect(tiles[0]?.when).toBe('★ 9,1');
  });

  it('other rows keep titles without a rating; no rating while ratings are off', () => {
    const tiles = listRowTiles(rowOf('added'), [movie(3), series(4, { monitored: false })], false);
    expect(tiles.map((t) => [t.key, t.href, t.name, t.when, t.dimmed])).toEqual([
      ['movie/3', '/filme/3', 'Film 3, 2001', null, false],
      ['series/4', '/serien/4', 'Serie 4, 2019', null, true],
    ]);
    expect(tiles[0]?.mark).toEqual({ text: 'fehlt', tone: 'danger' });
  });

  it('hold at most 20 tiles', () => {
    const many = Array.from({ length: 25 }, (_, i) => movie(i + 1));
    expect(listRowTiles(rowOf('added'), many, false)).toHaveLength(20);
  });
});

describe('plain lists (AC-04)', () => {
  it('have no search text and no filter; sorting does not count', () => {
    expect(isPlain(EMPTY_FILTERS)).toBe(true);
    expect(isPlain({ ...EMPTY_FILTERS, sort: 'added', order: 'desc' })).toBe(true);
    expect(isPlain({ ...EMPTY_FILTERS, q: 'boot' })).toBe(false);
    expect(isPlain({ ...EMPTY_FILTERS, tris: { monitored: 'no' } })).toBe(false);
    expect(isPlain({ ...EMPTY_FILTERS, tag: 3 })).toBe(false);
  });
});

describe('„Alle anzeigen“ (AC-05)', () => {
  it('leads to the list with the row’s sort and filter', () => {
    expect(listRowHref('movie', rowOf('added'))).toBe('/filme?sortierung=hinzugefuegt&richtung=ab');
    expect(listRowHref('series', rowOf('missing'))).toBe(
      '/serien?fehlt=ja&sortierung=hinzugefuegt&richtung=ab',
    );
    expect(listRowHref('movie', rowOf('cutoff'))).toBe(
      '/filme?cutoff=ja&sortierung=hinzugefuegt&richtung=ab',
    );
    expect(listRowHref('movie', rowOf('rated'))).toBe('/filme?sortierung=bewertung');
  });

  it('keeps the rows for the sorted rows, not for the filtered ones', () => {
    expect(LIST_ROWS.filter(keepsRows).map((r) => r.id)).toEqual(['added', 'rated']);
    for (const row of LIST_ROWS) {
      const back = parseFilters(
        'movie',
        new URL(listRowHref('movie', row), 'http://x').searchParams,
      );
      expect(back).toEqual(row.filters);
    }
  });
});
