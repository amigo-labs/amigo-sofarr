// Spec ui-list-toolbar AC-03, AC-05 .. AC-07: the active filters of a list as chips, their
// count, and removing one or all of them.

import { describe, expect, it } from 'vitest';
import { filterChips, withoutFilters } from './filter-chips';
import { EMPTY_FILTERS, type ListFilters } from './list-query';

const PROFILES = [
  { id: 4, name: 'HD-1080p' },
  { id: 6, name: 'Ultra-HD' },
];
const TAGS = [{ id: 2, name: 'kinder' }];

const all: ListFilters = {
  ...EMPTY_FILTERS,
  tris: { monitored: 'yes', missing: 'no', cutoff_unmet: 'yes', has_file: 'no' },
  profile: 4,
  tag: 2,
  q: 'boot',
  minRating: 7,
  minVotes: 10_000,
  sort: 'year',
  order: 'desc',
};

const texts = (chips: ReturnType<typeof filterChips>) => chips.map((c) => c.text);

describe('filter chips', () => {
  it('has none while no filter is active; search and sort are no filters', () => {
    const filters = { ...EMPTY_FILTERS, q: 'boot', sort: 'year', order: 'desc' } as const;
    expect(filterChips('movie', filters, true, PROFILES, TAGS)).toEqual([]);
  });

  it("names every active filter and its value in the panel's order (AC-05)", () => {
    expect(texts(filterChips('movie', all, true, PROFILES, TAGS))).toEqual([
      'Überwacht: Ja',
      'Fehlt: Nein',
      'Cutoff nicht erreicht: Ja',
      'Mit Datei: Nein',
      'Qualitätsprofil: HD-1080p',
      'Tag: kinder',
      'Mindestbewertung: ab 7,0',
      'Mindeststimmen: ab 10.000',
    ]);
  });

  it('series have Komplett instead of Mit Datei', () => {
    const filters = { ...EMPTY_FILTERS, tris: { complete: 'yes', has_file: 'no' } } as const;
    expect(texts(filterChips('series', filters, true, PROFILES, TAGS))).toEqual(['Komplett: Ja']);
  });

  it('shows the id of an unknown profile or tag', () => {
    const filters = { ...EMPTY_FILTERS, profile: 99, tag: 98 };
    expect(texts(filterChips('movie', filters, true, PROFILES, TAGS))).toEqual([
      'Qualitätsprofil: 99',
      'Tag: 98',
    ]);
  });

  it('leaves out the rating filters while ratings are off', () => {
    const filters = { ...EMPTY_FILTERS, minRating: 7, minVotes: 100 };
    expect(filterChips('movie', filters, false, PROFILES, TAGS)).toEqual([]);
  });

  it('removing a chip sets only that filter back to Alle (AC-06)', () => {
    const chips = filterChips('movie', all, true, PROFILES, TAGS);
    const profile = chips.find((c) => c.text.startsWith('Qualitätsprofil'));
    expect(profile?.remove(all)).toEqual({ ...all, profile: null });
    const monitored = chips.find((c) => c.text.startsWith('Überwacht'));
    expect(monitored?.remove(all).tris).toEqual({
      monitored: 'any',
      missing: 'no',
      cutoff_unmet: 'yes',
      has_file: 'no',
    });
    const votes = chips.find((c) => c.text.startsWith('Mindeststimmen'));
    expect(votes?.remove(all)).toEqual({ ...all, minVotes: null });
  });

  it('resetting clears every filter and keeps search and sort (AC-07)', () => {
    expect(withoutFilters(all)).toEqual({
      ...EMPTY_FILTERS,
      q: 'boot',
      sort: 'year',
      order: 'desc',
    });
  });
});
