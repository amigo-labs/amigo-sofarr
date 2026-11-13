// The library endpoints (spec web-ui-library) with the types generated from the server.

import { api } from '../auth/session.svelte';
import type { ApiResult } from './http';
import type { ActivityEntry } from './generated/ActivityEntry';
import type { BackupFile } from './generated/BackupFile';
import type { BackupList } from './generated/BackupList';
import type { BlocklistPage } from './generated/BlocklistPage';
import type { BulkSearchBody } from './generated/BulkSearchBody';
import type { BulkSearchReport } from './generated/BulkSearchReport';
import type { CalendarAnswer } from './generated/CalendarAnswer';
import type { ChangePreview } from './generated/ChangePreview';
import type { EpisodeDetail } from './generated/EpisodeDetail';
import type { FolderAnswer } from './generated/FolderAnswer';
import type { FolderPreview } from './generated/FolderPreview';
import type { LibraryOptions } from './generated/LibraryOptions';
import type { MediaKind } from './generated/MediaKind';
import type { MonitorAnswer } from './generated/MonitorAnswer';
import type { MonitorItem } from './generated/MonitorItem';
import type { MovieChange } from './generated/MovieChange';
import type { MovieDetail } from './generated/MovieDetail';
import type { MovieListItem } from './generated/MovieListItem';
import type { OverviewAnswer } from './generated/OverviewAnswer';
import type { Page } from './generated/Page';
import type { RemoveAnswer } from './generated/RemoveAnswer';
import type { RemoveBody } from './generated/RemoveBody';
import type { RenameAnswer } from './generated/RenameAnswer';
import type { RenamePreview } from './generated/RenamePreview';
import type { RestoreAnswer } from './generated/RestoreAnswer';
import type { SearchBody } from './generated/SearchBody';
import type { SearchReport } from './generated/SearchReport';
import type { SeasonAnswer } from './generated/SeasonAnswer';
import type { SeriesChange } from './generated/SeriesChange';
import type { SeriesDetail } from './generated/SeriesDetail';
import type { SeriesListItem } from './generated/SeriesListItem';
import type { WantedItem } from './generated/WantedItem';
import type { WantedList } from './generated/WantedList';
import type { WantedMedia } from './generated/WantedMedia';

/** The path segment of a movie or series in the library endpoints. */
export type LibraryKind = 'movies' | 'series';

export const libraryKind = (kind: MediaKind): LibraryKind =>
  kind === 'movie' ? 'movies' : 'series';

/** A query string from parameters; `null` values are left out. */
export function queryString(params: Readonly<Record<string, string | number | null>>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== null) search.set(key, String(value));
  }
  const text = search.toString();
  return text === '' ? '' : `?${text}`;
}

export const getOptions = (): Promise<ApiResult<LibraryOptions>> =>
  api.read('/api/library/options');

export const listMovies = (query: string): Promise<ApiResult<Page<MovieListItem>>> =>
  api.read(`/api/library/movies${query}`);

export const listSeries = (query: string): Promise<ApiResult<Page<SeriesListItem>>> =>
  api.read(`/api/library/series${query}`);

export const getMovie = (id: number): Promise<ApiResult<MovieDetail>> =>
  api.read(`/api/library/movies/${id}`);

export const getSeries = (id: number): Promise<ApiResult<SeriesDetail>> =>
  api.read(`/api/library/series/${id}`);

export const getSeason = (id: number, season: number): Promise<ApiResult<SeasonAnswer>> =>
  api.read(`/api/library/series/${id}/seasons/${season}`);

export const getEpisode = (id: number, episode: number): Promise<ApiResult<EpisodeDetail>> =>
  api.read(`/api/library/series/${id}/episodes/${episode}`);

export const previewMovieChange = (
  id: number,
  change: MovieChange,
): Promise<ApiResult<ChangePreview>> =>
  api.write('Änderung prüfen', 'POST', `/api/library/movies/${id}/change-preview`, change);

export const changeMovie = (id: number, change: MovieChange): Promise<ApiResult<MovieDetail>> =>
  api.write('Einstellungen speichern', 'PATCH', `/api/library/movies/${id}`, change);

export const previewSeriesChange = (
  id: number,
  change: SeriesChange,
): Promise<ApiResult<ChangePreview>> =>
  api.write('Änderung prüfen', 'POST', `/api/library/series/${id}/change-preview`, change);

export const changeSeries = (id: number, change: SeriesChange): Promise<ApiResult<SeriesDetail>> =>
  api.write('Einstellungen speichern', 'PATCH', `/api/library/series/${id}`, change);

export const monitor = (
  monitored: boolean,
  items: readonly MonitorItem[],
): Promise<ApiResult<MonitorAnswer>> =>
  api.write(monitored ? 'Überwachen' : 'Nicht mehr überwachen', 'POST', '/api/library/monitor', {
    monitored,
    items,
  });

export const search = (body: SearchBody): Promise<ApiResult<SearchReport>> =>
  api.write('Suchen', 'POST', '/api/command/search', body);

export const searchBulk = (body: BulkSearchBody): Promise<ApiResult<BulkSearchReport>> =>
  api.write('Suchen', 'POST', '/api/command/search-bulk', body);

export const markFailed = (kind: MediaKind, grabId: number): Promise<ApiResult<null>> =>
  api.write('Als fehlgeschlagen markieren', 'POST', `/api/grabs/${kind}/${grabId}/fail`);

export const getWanted = (
  list: WantedList,
  media: WantedMedia,
  page: number,
): Promise<ApiResult<Page<WantedItem>>> =>
  api.read(`/api/library/wanted${queryString({ list, media, page, page_size: 50 })}`);

export const getOverview = (): Promise<ApiResult<OverviewAnswer>> => api.read('/api/overview');

export const getActivity = (limit: number): Promise<ApiResult<ActivityEntry[]>> =>
  api.read(`/api/activity${queryString({ limit })}`);

export const getCalendar = (from: string, to: string): Promise<ApiResult<CalendarAnswer>> =>
  api.read(`/api/calendar${queryString({ from, to })}`);

export const renamePreview = (kind: LibraryKind, id: number): Promise<ApiResult<RenamePreview>> =>
  api.read(`/api/library/${kind}/${id}/rename`);

export const rename = (kind: LibraryKind, id: number): Promise<ApiResult<RenameAnswer>> =>
  api.write('Umbenennen', 'POST', `/api/library/${kind}/${id}/rename`);

export const folderPreview = (kind: LibraryKind, id: number): Promise<ApiResult<FolderPreview>> =>
  api.read(`/api/library/${kind}/${id}/folder`);

export const moveFolder = (kind: LibraryKind, id: number): Promise<ApiResult<FolderAnswer>> =>
  api.write('Ordner verschieben', 'POST', `/api/library/${kind}/${id}/folder`);

export const remove = (
  kind: LibraryKind,
  id: number,
  body: RemoveBody,
): Promise<ApiResult<RemoveAnswer>> =>
  api.write('Entfernen', 'POST', `/api/library/${kind}/${id}/remove`, body);

export const getBlocklist = (page: number): Promise<ApiResult<BlocklistPage>> =>
  api.read(`/api/blocklist${queryString({ page, page_size: 50 })}`);

export const unblock = (id: number): Promise<ApiResult<null>> =>
  api.write('Blocklist-Eintrag entfernen', 'DELETE', `/api/blocklist/${id}`);

export const getBackups = (): Promise<ApiResult<BackupList>> => api.read('/api/backups');

export const createBackup = (): Promise<ApiResult<BackupFile>> =>
  api.write('Sicherung erstellen', 'POST', '/api/backups');

export const backupAddress = (name: string): string => `/api/backups/${encodeURIComponent(name)}`;

export const deleteBackup = (name: string): Promise<ApiResult<null>> =>
  api.write('Sicherung löschen', 'DELETE', backupAddress(name));

export const chooseRestore = (name: string): Promise<ApiResult<RestoreAnswer>> =>
  api.write('Wiederherstellung wählen', 'POST', `${backupAddress(name)}/restore`);

export const cancelRestore = (): Promise<ApiResult<null>> =>
  api.write('Wiederherstellung abbrechen', 'DELETE', '/api/backups/restore');

/** The poster address of an item; sofarr serves it from its cache (AC-20). */
export const posterAddress = (kind: MediaKind, id: number, size: 'list' | 'detail'): string =>
  `/api/images/${kind}/${id}/${size}`;
