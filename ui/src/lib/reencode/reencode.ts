// Re-encoding (specs reencode-job AC-08, ui-reencode): the API calls of the page
// "Neu encodieren" and the German texts the pages show.

import { api } from '../auth/session.svelte';
import type { ApiResult } from '../api/http';
import type { HistoryEntry } from '../api/generated/HistoryEntry';
import type { PageCandidate } from '../api/generated/PageCandidate';
import type { ReencodePage } from '../api/generated/ReencodePage';
import type { ReencodeSupport } from '../api/generated/ReencodeSupport';
import type { RuleExclusions } from '../api/generated/RuleExclusions';
import { dateTimeText } from '../system/format';

/** Whether the re-encode job runs, or why not (spec reencode-job AC-08). */
export function jobText(support: ReencodeSupport): string {
  if (!support.active) return `aus – ${support.message ?? 'Grund unbekannt'}`;
  return support.running ? `aktiv – encodiert ${support.running}` : 'aktiv';
}

export const getReencodePage = (): Promise<ApiResult<ReencodePage>> => api.read('/api/reencode');

/** The body that names a candidate's file: its movie, or the first of its episodes. */
export function targetBody(c: PageCandidate): { movie_id: number } | { episode_id: number } {
  if (c.movie_id !== null) return { movie_id: c.movie_id };
  return { episode_id: c.episode_ids[0] ?? -1 };
}

export const startEncode = (c: PageCandidate): Promise<ApiResult<unknown>> =>
  api.write('Encode starten', 'POST', '/api/reencode/encode', targetBody(c));

export const setExcluded = (c: PageCandidate, excluded: boolean): Promise<ApiResult<unknown>> =>
  api.write(
    excluded ? 'Ausschließen' : 'Wieder aufnehmen',
    excluded ? 'POST' : 'DELETE',
    '/api/reencode/exclusions',
    targetBody(c),
  );

/** Bytes as GB (binary) with one decimal and a German comma. */
export function gbText(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(1).replace('.', ',')} GB`;
}

/** A size per minute in MB with one decimal. */
export const mbText = (mb: number): string => mb.toFixed(1).replace('.', ',');

export function savedText(files: number, bytes: number): string {
  return `${files} ${files === 1 ? 'Datei' : 'Dateien'}, ${gbText(bytes)} gespart`;
}

/** The job state of a candidate (spec ui-reencode, "Job state of a candidate"). */
export function stateText(c: PageCandidate, timeZone?: string): string {
  switch (c.state) {
    case 'running':
      return 'läuft';
    case 'excluded':
      return 'ausgeschlossen';
    case 'done':
      return 'erledigt';
    case 'no_gain':
      return 'kein Gewinn';
    case 'rests_until':
      return c.rests_until === null ? 'ruht' : `ruht bis ${dateTimeText(c.rests_until, timeZone)}`;
    case 'rests_until_change':
      return 'ruht, bis sich die Datei ändert';
    default:
      return 'fällig';
  }
}

export function outcomeText(outcome: string | null): string {
  switch (outcome) {
    case 'replaced':
      return 'ersetzt';
    case 'no_gain':
      return 'kein Gewinn';
    case 'failed':
      return 'fehlgeschlagen';
    case 'refused':
      return 'abgelehnt';
    default:
      return 'läuft';
  }
}

export const runnerText = (runner: string): string => (runner === 'cli' ? 'CLI' : 'Server');

/** Sizes of an attempt: before, after and the gain for "ersetzt". */
export function sizesText(entry: HistoryEntry): string {
  if (entry.size_after === null) return gbText(entry.size_before);
  const text = `${gbText(entry.size_before)} → ${gbText(entry.size_after)}`;
  if (entry.outcome !== 'replaced' || entry.size_before === 0) return text;
  const gain = Math.round(((entry.size_before - entry.size_after) * 100) / entry.size_before);
  return `${text} (${gain} % kleiner)`;
}

/** The library files of spec reencode-rule left out, per reason that has files. */
export function exclusionTexts(excluded: RuleExclusions): string[] {
  const reasons: [number, string][] = [
    [excluded.no_media_info, 'keine Media-Infos'],
    [excluded.resolution_off, 'Auflösung ausgeschaltet'],
    [excluded.unknown_duration, 'Laufzeit unbekannt'],
    [excluded.dynamic_metadata, 'Dolby Vision/HDR10+ würde verloren gehen'],
  ];
  return reasons.filter(([n]) => n > 0).map(([n, reason]) => `${n} × ${reason}`);
}

/** Why "Jetzt encodieren" is not available now, or `null`. */
export function encodeBlocked(page: ReencodePage): string | null {
  if (page.shadow) return 'im Shadow-Mode';
  if (page.running !== null) return `es läuft bereits ein Encode: ${page.running.title}`;
  return null;
}

/** The page of a candidate's movie or series. */
export function candidateHref(c: PageCandidate): string | null {
  if (c.movie_id !== null) return `/filme/${c.movie_id}`;
  return c.series_id === null ? null : `/serien/${c.series_id}`;
}
