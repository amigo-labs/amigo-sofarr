import { describe, expect, it } from 'vitest';
import {
  EMPTY_FILTERS,
  apiQuery,
  filterParams,
  isFiltered,
  parseFilters,
  sortsFor,
  usesRatings,
  withoutRatings,
  type ListFilters,
} from './list-query';

const params = (text: string) => new URLSearchParams(text);

describe('list filters in the page address (AC-03, AC-06)', () => {
  it('reads German parameters', () => {
    const filters = parseFilters(
      'movie',
      params(
        'ueberwacht=ja&fehlt=nein&cutoff=ja&datei=nein&profil=4&tag=2&suche=Boot&sortierung=jahr&richtung=ab',
      ),
    );
    expect(filters).toEqual({
      tris: { monitored: 'yes', missing: 'no', cutoff_unmet: 'yes', has_file: 'no' },
      profile: 4,
      tag: 2,
      q: 'Boot',
      minRating: null,
      minVotes: null,
      sort: 'year',
      order: 'desc',
    });
  });

  it('falls back to defaults for unknown or invalid values', () => {
    expect(
      parseFilters(
        'movie',
        params('ueberwacht=vielleicht&profil=abc&tag=-1&sortierung=farbe&richtung=x'),
      ),
    ).toEqual(EMPTY_FILTERS);
  });

  it('has "komplett" for series and no "datei"', () => {
    const filters = parseFilters('series', params('komplett=nein&datei=ja'));
    expect(filters.tris).toEqual({ complete: 'no' });
  });

  it('writes only what differs from the default, and round-trips', () => {
    expect(filterParams('movie', EMPTY_FILTERS).toString()).toBe('');
    const filters: ListFilters = {
      tris: { monitored: 'yes', missing: 'any' },
      profile: 5,
      tag: null,
      q: '  Das Boot ',
      minRating: null,
      minVotes: null,
      sort: 'added',
      order: 'desc',
    };
    const text = filterParams('movie', filters).toString();
    expect(text).toBe('ueberwacht=ja&profil=5&suche=Das+Boot&sortierung=hinzugefuegt&richtung=ab');
    expect(parseFilters('movie', params(text))).toEqual({
      ...filters,
      tris: { monitored: 'yes' },
      q: 'Das Boot',
    });
  });

  it('builds the API query of a page', () => {
    const filters = parseFilters('series', params('ueberwacht=nein&komplett=ja&suche=Insel'));
    expect(apiQuery('series', filters, 3, 50)).toBe(
      '?monitored=no&complete=yes&q=Insel&sort=title&order=asc&page=3&page_size=50',
    );
    expect(apiQuery('movie', EMPTY_FILTERS, 1, 50)).toBe(
      '?sort=title&order=asc&page=1&page_size=50',
    );
  });

  it('knows whether anything narrows the list', () => {
    expect(isFiltered(EMPTY_FILTERS)).toBe(false);
    expect(isFiltered({ ...EMPTY_FILTERS, sort: 'year' })).toBe(false);
    expect(isFiltered({ ...EMPTY_FILTERS, q: 'x' })).toBe(true);
    expect(isFiltered({ ...EMPTY_FILTERS, tris: { missing: 'no' } })).toBe(true);
  });
});

describe('IMDb rating sort and minimums in the page address (imdb-ratings AC-06, AC-07, AC-09)', () => {
  it('reads the minimums and the rating sort', () => {
    const filters = parseFilters(
      'movie',
      params('mindestbewertung=7,5&mindeststimmen=1000&sortierung=bewertung'),
    );
    expect(filters).toEqual({ ...EMPTY_FILTERS, minRating: 7.5, minVotes: 1000, sort: 'rating' });
    expect(parseFilters('series', params('mindestbewertung=8')).minRating).toBe(8);
  });

  it('ignores minimums outside 0 to 10, with more decimals or not numbers', () => {
    for (const text of ['11', '7.55', '-1', 'gut', '', '1e3']) {
      expect(parseFilters('movie', params(`mindestbewertung=${text}`)).minRating).toBeNull();
    }
    for (const text of ['-5', '1.5', 'viele', '']) {
      expect(parseFilters('movie', params(`mindeststimmen=${text}`)).minVotes).toBeNull();
    }
    expect(parseFilters('movie', params('mindestbewertung=10')).minRating).toBe(10);
    expect(parseFilters('movie', params('mindeststimmen=0')).minVotes).toBe(0);
  });

  it('round-trips through the address and builds the API query', () => {
    const filters: ListFilters = { ...EMPTY_FILTERS, minRating: 6.5, minVotes: 50, sort: 'rating' };
    const text = filterParams('series', filters).toString();
    expect(text).toBe('mindestbewertung=6.5&mindeststimmen=50&sortierung=bewertung');
    expect(parseFilters('series', params(text))).toEqual(filters);
    expect(apiQuery('series', filters, 1, 50)).toBe(
      '?min_rating=6.5&min_votes=50&sort=rating&order=asc&page=1&page_size=50',
    );
  });

  it('counts the minimums as filters, the rating sort not', () => {
    expect(isFiltered({ ...EMPTY_FILTERS, minRating: 7 })).toBe(true);
    expect(isFiltered({ ...EMPTY_FILTERS, minVotes: 0 })).toBe(true);
    expect(isFiltered({ ...EMPTY_FILTERS, sort: 'rating' })).toBe(false);
  });

  it('offers and sends nothing by rating while ratings are off', () => {
    expect(sortsFor(true).map((s) => s.value)).toContain('rating');
    expect(sortsFor(false).map((s) => s.value)).toEqual(['title', 'year', 'added', 'size']);
    const rated: ListFilters = {
      ...EMPTY_FILTERS,
      q: 'Boot',
      minRating: 7,
      minVotes: 10,
      sort: 'rating',
      order: 'desc',
    };
    expect(usesRatings(rated)).toBe(true);
    expect(withoutRatings(rated)).toEqual({ ...EMPTY_FILTERS, q: 'Boot', order: 'desc' });
    expect(usesRatings(withoutRatings(rated))).toBe(false);
    const plain = { ...EMPTY_FILTERS, sort: 'year' as const };
    expect(withoutRatings(plain)).toBe(plain);
  });
});
