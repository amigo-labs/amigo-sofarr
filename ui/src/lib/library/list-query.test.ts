import { describe, expect, it } from 'vitest';
import {
  EMPTY_FILTERS,
  apiQuery,
  filterParams,
  isFiltered,
  parseFilters,
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
