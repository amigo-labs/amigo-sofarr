// The active filters of a movie or series list as chips (spec ui-list-toolbar AC-03,
// AC-05 .. AC-07): one per filter that is not "Alle", in the filter panel's order, each
// knowing how to remove itself. Search and sort are no filters.

import type { Choice } from '../api/generated/Choice';
import { ratingText } from '../imdb/imdb';
import { EMPTY_FILTERS, TRI_FILTERS, type ListFilters, type ListKind } from './list-query';

export type FilterChip = {
  /** A stable key for lists and focus, e.g. `monitored` or `profile`. */
  readonly key: string;
  /** The filter and its value, e.g. "Überwacht: Ja". */
  readonly text: string;
  /** The filters with this one back at "Alle". */
  readonly remove: (filters: ListFilters) => ListFilters;
};

const VOTES_TEXT = new Intl.NumberFormat('de-DE');

const nameOf = (choices: readonly Choice[], id: number): string =>
  choices.find((c) => c.id === id)?.name ?? String(id);

/** The chips of the active filters; rating minimums only while ratings are shown. */
export function filterChips(
  kind: ListKind,
  filters: ListFilters,
  ratings: boolean,
  profiles: readonly Choice[],
  tags: readonly Choice[],
): readonly FilterChip[] {
  const chips: FilterChip[] = [];
  for (const filter of TRI_FILTERS[kind]) {
    const value = filters.tris[filter.key] ?? 'any';
    if (value === 'any') continue;
    chips.push({
      key: filter.key,
      text: `${filter.label}: ${value === 'yes' ? 'Ja' : 'Nein'}`,
      remove: (f) => ({ ...f, tris: { ...f.tris, [filter.key]: 'any' } }),
    });
  }
  if (filters.profile !== null) {
    chips.push({
      key: 'profile',
      text: `Qualitätsprofil: ${nameOf(profiles, filters.profile)}`,
      remove: (f) => ({ ...f, profile: null }),
    });
  }
  if (filters.tag !== null) {
    chips.push({
      key: 'tag',
      text: `Tag: ${nameOf(tags, filters.tag)}`,
      remove: (f) => ({ ...f, tag: null }),
    });
  }
  if (ratings && filters.minRating !== null) {
    chips.push({
      key: 'minRating',
      text: `Mindestbewertung: ab ${ratingText(filters.minRating)}`,
      remove: (f) => ({ ...f, minRating: null }),
    });
  }
  if (ratings && filters.minVotes !== null) {
    chips.push({
      key: 'minVotes',
      text: `Mindeststimmen: ab ${VOTES_TEXT.format(filters.minVotes)}`,
      remove: (f) => ({ ...f, minVotes: null }),
    });
  }
  return chips;
}

/** Every filter back at "Alle"; search and sort stay. */
export function withoutFilters(filters: ListFilters): ListFilters {
  return { ...EMPTY_FILTERS, q: filters.q, sort: filters.sort, order: filters.order };
}
