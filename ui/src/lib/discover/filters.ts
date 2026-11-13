// The filters of the discover page, kept in the page address with German names (spec
// discover AC-02, like the library lists), and the matching API query.

import type { Rating } from '../api/generated/Rating';
import type { TitleKind } from '../api/generated/TitleKind';

export type DiscoverFilters = {
  readonly kind: TitleKind | null;
  readonly genre: number | null;
  readonly yearFrom: number | null;
  readonly yearTo: number | null;
  /** Original language, ISO 639-1. */
  readonly language: string | null;
  /** Minimum IMDb rating; only offered while ratings are on. */
  readonly minRating: number | null;
};

export const NO_FILTERS: DiscoverFilters = {
  kind: null,
  genre: null,
  yearFrom: null,
  yearTo: null,
  language: null,
  minRating: null,
};

/** Years the server accepts. */
export const FIRST_YEAR = 1870;
export const LAST_YEAR = 2100;

/** The original languages offered, by German name. */
export const LANGUAGES: readonly { readonly code: string; readonly name: string }[] = [
  { code: 'da', name: 'Dänisch' },
  { code: 'de', name: 'Deutsch' },
  { code: 'en', name: 'Englisch' },
  { code: 'fr', name: 'Französisch' },
  { code: 'hi', name: 'Hindi' },
  { code: 'it', name: 'Italienisch' },
  { code: 'ja', name: 'Japanisch' },
  { code: 'ko', name: 'Koreanisch' },
  { code: 'nl', name: 'Niederländisch' },
  { code: 'no', name: 'Norwegisch' },
  { code: 'pl', name: 'Polnisch' },
  { code: 'es', name: 'Spanisch' },
  { code: 'sv', name: 'Schwedisch' },
  { code: 'tr', name: 'Türkisch' },
  { code: 'zh', name: 'Chinesisch' },
];

/** The minimum ratings offered. */
export const MIN_RATINGS: readonly number[] = [5, 6, 6.5, 7, 7.5, 8, 8.5, 9];

const KIND_PARAM: Readonly<Record<TitleKind, string>> = { movie: 'filme', series: 'serien' };

function kindOf(text: string | null): TitleKind | null {
  if (text === 'filme') return 'movie';
  if (text === 'serien') return 'series';
  return null;
}

function idOf(text: string | null): number | null {
  if (text === null || !/^\d{1,9}$/.test(text)) return null;
  return Number(text);
}

function yearOf(text: string | null): number | null {
  if (text === null || !/^\d{4}$/.test(text)) return null;
  const year = Number(text);
  return year >= FIRST_YEAR && year <= LAST_YEAR ? year : null;
}

function ratingOf(text: string | null): number | null {
  if (text === null || !/^\d{1,2}([.,]\d)?$/.test(text)) return null;
  const rating = Number(text.replace(',', '.'));
  return rating >= 0 && rating <= 10 ? rating : null;
}

/** The filters of a page address; unknown or invalid values are left out. */
export function parseFilters(params: Pick<URLSearchParams, 'get'>): DiscoverFilters {
  const language = params.get('sprache');
  const yearFrom = yearOf(params.get('von'));
  const yearTo = yearOf(params.get('bis'));
  const swapped = yearFrom !== null && yearTo !== null && yearFrom > yearTo;
  return {
    kind: kindOf(params.get('art')),
    genre: idOf(params.get('genre')),
    yearFrom: swapped ? yearTo : yearFrom,
    yearTo: swapped ? yearFrom : yearTo,
    language: language !== null && /^[a-z]{2}$/.test(language) ? language : null,
    minRating: ratingOf(params.get('bewertung')),
  };
}

/** The page address parameters of filters; unset ones are left out. */
export function filterParams(filters: DiscoverFilters): URLSearchParams {
  const params = new URLSearchParams();
  if (filters.kind !== null) params.set('art', KIND_PARAM[filters.kind]);
  if (filters.genre !== null) params.set('genre', String(filters.genre));
  if (filters.yearFrom !== null) params.set('von', String(filters.yearFrom));
  if (filters.yearTo !== null) params.set('bis', String(filters.yearTo));
  if (filters.language !== null) params.set('sprache', filters.language);
  if (filters.minRating !== null) params.set('bewertung', String(filters.minRating));
  return params;
}

/** The filters that apply: the rating filter only while ratings are on. */
export function effective(filters: DiscoverFilters, ratings: boolean): DiscoverFilters {
  return ratings ? filters : { ...filters, minRating: null };
}

/** Whether any filter is set: then the result list replaces the shelves. */
export function isFiltered(filters: DiscoverFilters): boolean {
  return Object.values(filters).some((value) => value !== null);
}

/** The API query of one page of the result list (`/api/discover/list?…`). */
export function listQuery(filters: DiscoverFilters, page: number): string {
  const params = new URLSearchParams();
  if (filters.kind !== null) params.set('kind', filters.kind);
  if (filters.genre !== null) params.set('genre', String(filters.genre));
  if (filters.yearFrom !== null) params.set('year_from', String(filters.yearFrom));
  if (filters.yearTo !== null) params.set('year_to', String(filters.yearTo));
  if (filters.language !== null) params.set('language', filters.language);
  if (filters.minRating !== null) params.set('min_rating', String(filters.minRating));
  params.set('page', String(page));
  return `?${params.toString()}`;
}

/**
 * Whether a title stays in a list filtered by a minimum rating: a known rating that
 * reaches it, or one still loading (shown with its placeholder until it is known).
 */
export function keeps(rating: Rating, minRating: number | null): boolean {
  if (minRating === null) return true;
  if (rating.state === 'pending') return true;
  return rating.state === 'rated' && rating.rating !== null && rating.rating >= minRating;
}
