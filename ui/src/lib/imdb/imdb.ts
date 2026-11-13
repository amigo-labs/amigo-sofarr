// IMDb ratings next to titles (spec imdb-ratings): the API call for the ratings data and
// the German texts every page that shows ratings uses.

import { api } from '../auth/session.svelte';
import type { ApiResult } from '../api/http';
import type { ImdbInfo } from '../api/generated/ImdbInfo';
import { dateTimeText } from '../system/format';

/** Whether ratings are on, the date of the data and IMDb's attribution (AC-08, AC-09). */
export const getImdbInfo = (): Promise<ApiResult<ImdbInfo>> => api.read('/api/imdb');

const RATING = new Intl.NumberFormat('de-DE', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});
const VOTES = new Intl.NumberFormat('de-DE');

/** A rating as German text, e.g. "7,7"; "–" without one (AC-04). */
export function ratingText(rating: number | null): string {
  return rating === null || !Number.isFinite(rating) ? '–' : RATING.format(rating);
}

/** Votes as German text, e.g. "4.242 Stimmen"; "–" without them (AC-04). */
export function votesText(votes: number | null): string {
  if (votes === null || !Number.isFinite(votes)) return '–';
  return `${VOTES.format(votes)} ${votes === 1 ? 'Stimme' : 'Stimmen'}`;
}

/** Rating and votes in one, e.g. "7,7 (4.242 Stimmen)"; "–" without a rating (AC-04). */
export function ratingWithVotesText(rating: number | null, votes: number | null): string {
  if (rating === null || !Number.isFinite(rating)) return '–';
  return votes === null ? ratingText(rating) : `${ratingText(rating)} (${votesText(votes)})`;
}

/** The date line under the attribution; `null` while no data was loaded yet (AC-08). */
export function dataDateText(info: ImdbInfo, timeZone?: string): string | null {
  return info.loaded_at === null
    ? null
    : `Stand der Bewertungen: ${dateTimeText(info.loaded_at, timeZone)}`;
}
