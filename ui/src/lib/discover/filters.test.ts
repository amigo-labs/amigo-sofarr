import { describe, expect, it } from 'vitest';
import {
  NO_FILTERS,
  effective,
  filterParams,
  isFiltered,
  keeps,
  listQuery,
  parseFilters,
} from './filters';

const params = (text: string) => new URLSearchParams(text);
const rated = (rating: number) => ({ state: 'rated' as const, rating, votes: 10 });

describe('discover filters in the page address (discover AC-02)', () => {
  it('reads German names and values', () => {
    expect(
      parseFilters(params('art=serien&genre=18&von=1990&bis=2000&sprache=de&bewertung=7.5')),
    ).toEqual({
      kind: 'series',
      genre: 18,
      yearFrom: 1990,
      yearTo: 2000,
      language: 'de',
      minRating: 7.5,
    });
    expect(parseFilters(params('bewertung=7,5')).minRating).toBe(7.5);
  });

  it('drops invalid values and swaps a reversed year range', () => {
    expect(parseFilters(params('art=buecher&genre=x&von=12&sprache=deu&bewertung=11'))).toEqual(
      NO_FILTERS,
    );
    const swapped = parseFilters(params('von=2010&bis=2000'));
    expect([swapped.yearFrom, swapped.yearTo]).toEqual([2000, 2010]);
  });

  it('writes only the filters that are set, and round-trips', () => {
    expect(filterParams(NO_FILTERS).toString()).toBe('');
    const filters = { ...NO_FILTERS, kind: 'movie' as const, genre: 35, minRating: 6 };
    expect(filterParams(filters).toString()).toBe('art=filme&genre=35&bewertung=6');
    expect(parseFilters(filterParams(filters))).toEqual(filters);
  });

  it('turns filters into the API query', () => {
    expect(listQuery(NO_FILTERS, 1)).toBe('?page=1');
    expect(
      listQuery({ ...NO_FILTERS, kind: 'series', yearFrom: 2001, language: 'en', minRating: 7 }, 3),
    ).toBe('?kind=series&year_from=2001&language=en&min_rating=7&page=3');
  });

  it('ignores the rating filter while ratings are off', () => {
    const filters = { ...NO_FILTERS, minRating: 7 };
    expect(isFiltered(filters)).toBe(true);
    expect(isFiltered(effective(filters, false))).toBe(false);
    expect(effective(filters, true)).toEqual(filters);
  });

  it('keeps titles whose rating reaches the minimum or is still loading', () => {
    expect(keeps(rated(7), 7)).toBe(true);
    expect(keeps(rated(6.9), 7)).toBe(false);
    expect(keeps({ state: 'pending', rating: null, votes: null }, 7)).toBe(true);
    expect(keeps({ state: 'none', rating: null, votes: null }, 7)).toBe(false);
    expect(keeps({ state: 'none', rating: null, votes: null }, null)).toBe(true);
  });
});
