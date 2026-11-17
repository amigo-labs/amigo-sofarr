// Spec ui-discover-polish AC-01, AC-02: the active discover filters as chips and their count.

import { describe, expect, it } from 'vitest';
import { NO_FILTERS, discoverChips } from './filters';

const GENRES = [{ id: 99, name: 'Dokumentarfilm', movie: true, series: true }];

describe('discover chips', () => {
  it('has none without filters', () => {
    expect(discoverChips(NO_FILTERS, GENRES, true)).toEqual([]);
  });

  it('names every active filter and its value in the panel order', () => {
    const chips = discoverChips(
      { kind: 'movie', genre: 99, yearFrom: 1990, yearTo: 2000, language: 'da', minRating: 7 },
      GENRES,
      true,
    );
    expect(chips.map((c) => c.text)).toEqual([
      'Art: Filme',
      'Genre: Dokumentarfilm',
      'Jahr von: 1990',
      'Jahr bis: 2000',
      'Originalsprache: Dänisch',
      'IMDb-Bewertung: ab 7,0',
    ]);
  });

  it('shows the id of an unknown genre; leaves out the rating while ratings are off', () => {
    const filters = { ...NO_FILTERS, genre: 12, minRating: 7 };
    expect(discoverChips(filters, GENRES, false).map((c) => c.text)).toEqual(['Genre: 12']);
  });

  it('removing a chip clears only its filter; the other year bound stays', () => {
    const filters = { ...NO_FILTERS, kind: 'series' as const, yearFrom: 1990, yearTo: 2000 };
    const from = discoverChips(filters, GENRES, true).find((c) => c.key === 'yearFrom');
    expect(from?.remove(filters)).toEqual({ ...filters, yearFrom: null });
    const kind = discoverChips(filters, GENRES, true).find((c) => c.key === 'kind');
    expect(kind?.remove(filters)).toEqual({ ...filters, kind: null });
  });
});
