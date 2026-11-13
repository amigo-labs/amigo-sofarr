// Actions of several pages that report their German result as a notice (AC-09).

import type { MediaKind } from '../api/generated/MediaKind';
import type { SearchBody } from '../api/generated/SearchBody';
import { markFailed, search } from '../api/library';
import { searchText, searchTone } from './format';
import { notify } from './notices.svelte';

export const searchBody = (fields: Partial<SearchBody>): SearchBody => ({
  movie_id: fields.movie_id ?? null,
  series_id: fields.series_id ?? null,
  season: fields.season ?? null,
  episode: fields.episode ?? null,
});

/** Searches one item and shows the result; `label` names it in the notice. */
export async function runSearch(label: string, body: SearchBody): Promise<void> {
  const result = await search(body);
  if (result.ok) {
    notify(searchTone(result.data.errors), `${label}: ${searchText(result.data)}`);
  } else if (result.failure.kind !== 'unauthorized') {
    notify('danger', `${label}: Suche fehlgeschlagen – ${result.failure.message}`);
  }
}

/** Marks a grab as failed; returns whether it worked. */
export async function runMarkFailed(
  kind: MediaKind,
  grabId: number,
  title: string,
): Promise<boolean> {
  const result = await markFailed(kind, grabId);
  if (result.ok) {
    notify('info', `„${title}“ wurde als fehlgeschlagen markiert; sofarr sucht einen Ersatz.`);
    return true;
  }
  if (result.failure.kind !== 'unauthorized') {
    notify('danger', `„${title}“ nicht markiert: ${result.failure.message}`);
  }
  return false;
}

/** The page of a movie or series. */
export const mediaHref = (kind: MediaKind, id: number): string =>
  `/${kind === 'movie' ? 'filme' : 'serien'}/${id}`;
