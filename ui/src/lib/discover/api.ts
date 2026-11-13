// The discover endpoints (spec discover) with the types generated from the server.

import { api } from '../auth/session.svelte';
import type { ApiResult } from '../api/http';
import type { AddAnswer } from '../api/generated/AddAnswer';
import type { AddOptions } from '../api/generated/AddOptions';
import type { CastAnswer } from '../api/generated/CastAnswer';
import type { DiscoverStatus } from '../api/generated/DiscoverStatus';
import type { GenresAnswer } from '../api/generated/GenresAnswer';
import type { ImportListAnswer } from '../api/generated/ImportListAnswer';
import type { ImportListBody } from '../api/generated/ImportListBody';
import type { ListAnswer } from '../api/generated/ListAnswer';
import type { MovieAddBody } from '../api/generated/MovieAddBody';
import type { PersonPage } from '../api/generated/PersonPage';
import type { SearchAnswer } from '../api/generated/SearchAnswer';
import type { SeriesAddBody } from '../api/generated/SeriesAddBody';
import type { ShelfId } from '../api/generated/ShelfId';
import type { ShelvesAnswer } from '../api/generated/ShelvesAnswer';
import type { StatesAnswer } from '../api/generated/StatesAnswer';
import type { TitleKind } from '../api/generated/TitleKind';
import type { TitlePreview } from '../api/generated/TitlePreview';
import type { TitleRef } from '../api/generated/TitleRef';
import { queryString } from '../api/library';

export const getStatus = (): Promise<ApiResult<DiscoverStatus>> => api.read('/api/discover/status');

export const getShelves = (): Promise<ApiResult<ShelvesAnswer>> =>
  api.read('/api/discover/shelves');

export const getShelf = (id: ShelfId, page: number): Promise<ApiResult<ListAnswer>> =>
  api.read(`/api/discover/shelves/${id}${queryString({ page })}`);

/** The filtered result list; `query` comes from `listQuery` (filters.ts). */
export const getList = (query: string): Promise<ApiResult<ListAnswer>> =>
  api.read(`/api/discover/list${query}`);

export const getGenres = (): Promise<ApiResult<GenresAnswer>> => api.read('/api/discover/genres');

export const searchTitles = (q: string, page: number): Promise<ApiResult<SearchAnswer>> =>
  api.read(`/api/discover/search${queryString({ q, page })}`);

/**
 * Library state and rating of titles already shown (at most 500 per request). A read sent
 * as POST: it is repeated freely and no write action is reported when it gets 401.
 */
export const getStates = (titles: readonly TitleRef[]): Promise<ApiResult<StatesAnswer>> =>
  api.open('POST', '/api/discover/states', { titles });

export const getTitle = (kind: TitleKind, id: number): Promise<ApiResult<TitlePreview>> =>
  api.read(`/api/discover/titles/${kind}/${id}`);

export const getPerson = (id: number): Promise<ApiResult<PersonPage>> =>
  api.read(`/api/discover/people/${id}`);

export const getCast = (kind: TitleKind, id: number): Promise<ApiResult<CastAnswer>> =>
  api.read(`/api/library/${kind === 'movie' ? 'movies' : 'series'}/${id}/cast`);

export const getAddOptions = (kind: TitleKind): Promise<ApiResult<AddOptions>> =>
  api.read(`/api/discover/add/${kind}/options`);

export const addMovie = (body: MovieAddBody): Promise<ApiResult<AddAnswer>> =>
  api.write('Film hinzufügen', 'POST', '/api/discover/add/movie', body);

export const addSeries = (body: SeriesAddBody): Promise<ApiResult<AddAnswer>> =>
  api.write('Serie hinzufügen', 'POST', '/api/discover/add/series', body);

export const createImportList = (
  personId: number,
  body: ImportListBody,
): Promise<ApiResult<ImportListAnswer>> =>
  api.write('Import-Liste anlegen', 'POST', `/api/discover/people/${personId}/import-list`, body);
