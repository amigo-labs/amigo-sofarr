import type { ReencodeSupport } from '../api/generated/ReencodeSupport';

/** Whether the re-encode job runs, or why not (spec reencode-job AC-08). */
export function jobText(support: ReencodeSupport): string {
  if (!support.active) return `aus – ${support.message ?? 'Grund unbekannt'}`;
  return support.running ? `aktiv – encodiert ${support.running}` : 'aktiv';
}
