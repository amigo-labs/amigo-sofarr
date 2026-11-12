// Subtitles (spec subtitles AC-11, AC-12): the API calls of the overview and of one file,
// and the German texts the pages show.

import { api } from '../auth/session.svelte';
import type { ApiResult } from '../api/http';
import type { FileSubtitles } from '../api/generated/FileSubtitles';
import type { SubtitleItem } from '../api/generated/SubtitleItem';
import type { SubtitleOverview } from '../api/generated/SubtitleOverview';
import type { SubtitleSearchReport } from '../api/generated/SubtitleSearchReport';

/** The video file whose subtitles are shown or searched: a movie's or an episode's. */
export type SubtitleTarget =
  | { readonly kind: 'movie'; readonly movieId: number }
  | { readonly kind: 'episode'; readonly seriesId: number; readonly episodeId: number };

const base = (target: SubtitleTarget): string =>
  target.kind === 'movie'
    ? `/api/movies/${target.movieId}/subtitles`
    : `/api/series/${target.seriesId}/episodes/${target.episodeId}/subtitles`;

export const getSubtitleOverview = (): Promise<ApiResult<SubtitleOverview>> =>
  api.read('/api/subtitles');

export const getFileSubtitles = (target: SubtitleTarget): Promise<ApiResult<FileSubtitles>> =>
  api.read(base(target));

export const searchSubtitles = (target: SubtitleTarget): Promise<ApiResult<SubtitleSearchReport>> =>
  api.write('Untertitel suchen', 'POST', `${base(target)}/search`);

/** The file of an overview entry; `null` when the answer lacks its ids. */
export function targetOf(item: SubtitleItem): SubtitleTarget | null {
  if (item.kind === 'movie') {
    return item.movie_id === null ? null : { kind: 'movie', movieId: item.movie_id };
  }
  return item.series_id === null || item.episode_id === null
    ? null
    : { kind: 'episode', seriesId: item.series_id, episodeId: item.episode_id };
}

/** A stable key of an entry's file, to group the needs per file. */
export const targetKey = (item: SubtitleItem): string =>
  item.kind === 'movie' ? `movie-${item.movie_id}` : `episode-${item.episode_id}`;

/** The page of an entry's movie or series. */
export function itemHref(item: SubtitleItem): string | null {
  if (item.kind === 'movie') return item.movie_id === null ? null : `/filme/${item.movie_id}`;
  return item.series_id === null ? null : `/serien/${item.series_id}`;
}

const NAMES = new Intl.DisplayNames(['de'], { type: 'language', fallback: 'none' });

/** The German name of an ISO 639 code, e.g. "Deutsch"; the code itself when unknown. */
export function languageName(code: string | null): string {
  if (code === null || code.trim() === '' || code === 'und') return 'Sprache unbekannt';
  try {
    return NAMES.of(code) ?? code.toUpperCase();
  } catch {
    return code.toUpperCase();
  }
}

/** The kind of a subtitle in German: normal or forced, and whether it is for the deaf. */
export function kindText(forced: boolean, hearingImpaired: boolean): string {
  const kind = forced ? 'erzwungen' : 'normal';
  return hearingImpaired ? `${kind}, für Hörgeschädigte` : kind;
}

/** The kind of a need or found subtitle (`normal` or `forced`) in German. */
export const needKindText = (kind: string, hearingImpaired = false): string =>
  kindText(kind === 'forced', hearingImpaired);

/** How an own subtitle matched the video, e.g. "Release (3 Merkmale)". */
export function matchText(quality: string | null): string {
  if (quality === null) return '–';
  if (quality === 'hash') return 'Hash der Videodatei';
  if (quality === 'title') return 'nur Titel';
  const release = /^release:(\d+)$/.exec(quality);
  if (release !== null) {
    const n = Number(release[1]);
    return `Release (${n} ${n === 1 ? 'Merkmal' : 'Merkmale'})`;
  }
  return quality;
}

const count = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;

/** The German result of a search for one file (AC-11). */
export function searchReportText(title: string, report: SubtitleSearchReport): string {
  if (report.searched === 0) {
    return `„${title}“: Nichts zu suchen – jede gewünschte Sprache ist vorhanden.`;
  }
  const parts = [`${count(report.searched, 'Untertitel', 'Untertitel')} gesucht`];
  if (report.downloaded.length > 0) {
    parts.push(`${report.downloaded.length} geladen (${report.downloaded.join(', ')})`);
  }
  if (report.would_download.length > 0) {
    parts.push(`Schattenmodus, würde laden: ${report.would_download.join(', ')}`);
  }
  if (report.not_found > 0) parts.push(`${report.not_found} nicht gefunden`);
  if (report.waiting_for_quota > 0) {
    parts.push(`${report.waiting_for_quota} warten auf das Download-Kontingent`);
  }
  if (report.failed > 0) {
    parts.push(`${count(report.failed, 'Kandidat', 'Kandidaten')} nicht ladbar`);
  }
  return `„${title}“: ${parts.join(', ')}.`;
}
