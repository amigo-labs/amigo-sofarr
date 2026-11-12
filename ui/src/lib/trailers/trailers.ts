// Trailers (spec trailers AC-09, AC-10, AC-12): the API calls of a movie's or series'
// trailer and the German texts of its state.

import { api } from '../auth/session.svelte';
import type { ApiResult } from '../api/http';
import type { MediaKind } from '../api/generated/MediaKind';
import type { TrailerInfo } from '../api/generated/TrailerInfo';
import type { TrailerStatus } from '../api/generated/TrailerStatus';
import type { TrailerSupport } from '../api/generated/TrailerSupport';
import type { StatusText } from '../system/format';

const path = (kind: MediaKind, id: number): string =>
  `/api/${kind === 'movie' ? 'movies' : 'series'}/${id}/trailer`;

export const getTrailer = (kind: MediaKind, id: number): Promise<ApiResult<TrailerInfo>> =>
  api.read(path(kind, id));

/** Downloads the trailer at once; the answer comes when it is finished (AC-09). */
export const downloadTrailer = (kind: MediaKind, id: number): Promise<ApiResult<TrailerInfo>> =>
  api.write('Trailer laden', 'POST', path(kind, id));

/** The state label of a trailer (AC-10). */
export const STATUS_TEXT: Readonly<Record<TrailerStatus, StatusText>> = {
  downloaded: { text: 'vorhanden', tone: 'success' },
  foreign: { text: 'vorhanden, nicht von sofarr', tone: 'success' },
  waiting: { text: 'wartet', tone: 'neutral' },
  no_candidate: { text: 'kein Trailer gefunden', tone: 'neutral' },
  failed: { text: 'fehlgeschlagen', tone: 'danger' },
  low_space: { text: 'zu wenig Speicherplatz', tone: 'danger' },
  not_eligible: { text: 'kein automatischer Download', tone: 'neutral' },
};

/** Whether the item has a trailer, sofarr's or someone else's. */
export const hasTrailer = (info: TrailerInfo): boolean =>
  info.status === 'downloaded' || info.status === 'foreign';

/** The German result of a manual download (AC-09). */
export function downloadResultText(title: string, info: TrailerInfo): string {
  if (info.status === 'downloaded') {
    const language = info.language === null ? '' : ` (${info.language})`;
    return `Trailer für „${title}“ geladen${language}.`;
  }
  const reason = info.message ?? STATUS_TEXT[info.status].text;
  return `Kein Trailer für „${title}“ geladen: ${reason}`;
}

/** Whether the system downloads trailers, or why not (AC-12). */
export function supportText(support: TrailerSupport): string {
  if (support.active) return 'werden heruntergeladen';
  return `keine Downloads – ${support.message ?? 'Grund unbekannt'}`;
}
