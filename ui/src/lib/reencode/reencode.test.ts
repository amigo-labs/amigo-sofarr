import { describe, expect, it } from 'vitest';
import type { PageCandidate } from '../api/generated/PageCandidate';
import type { ReencodePage } from '../api/generated/ReencodePage';
import {
  candidateHref,
  encodeBlocked,
  exclusionTexts,
  gbText,
  jobText,
  outcomeText,
  runnerText,
  savedText,
  sizesText,
  stateText,
  targetBody,
} from './reencode';

const GB = 1024 ** 3;

const candidate: PageCandidate = {
  kind: 'movie',
  movie_id: 4,
  series_id: null,
  episode_ids: [],
  title: 'Heat (1995)',
  path: '/filme/Heat/Heat.mkv',
  size: 40.5 * GB,
  mb_per_minute: 414.72,
  threshold: 60,
  resolution: '1080p',
  video_codec: 'h264',
  state: 'due',
  rests_until: null,
};

describe('re-encode job text (reencode-job AC-08)', () => {
  it('says whether the job runs, what it encodes, or why not', () => {
    expect(jobText({ active: true, message: null, running: null })).toBe('aktiv');
    expect(jobText({ active: true, message: null, running: 'Heat (1995)' })).toBe(
      'aktiv – encodiert Heat (1995)',
    );
    expect(
      jobText({ active: false, message: 'in der Konfiguration ausgeschaltet', running: null }),
    ).toBe('aus – in der Konfiguration ausgeschaltet');
    expect(jobText({ active: false, message: null, running: null })).toBe('aus – Grund unbekannt');
  });
});

const at = (state: string, restsUntil: string | null = null): string =>
  stateText({ ...candidate, state, rests_until: restsUntil }, 'UTC');

describe('re-encode page texts (ui-reencode AC-02, AC-03, AC-06)', () => {
  it('name sizes and the saved space in German', () => {
    expect(gbText(40.5 * GB)).toBe('40,5 GB');
    expect(savedText(1, 2 * GB)).toBe('1 Datei, 2,0 GB gespart');
    expect(savedText(12, 300 * GB)).toBe('12 Dateien, 300,0 GB gespart');
  });

  it('name every job state', () => {
    expect(at('running')).toBe('läuft');
    expect(at('excluded')).toBe('ausgeschlossen');
    expect(at('done')).toBe('erledigt');
    expect(at('no_gain')).toBe('kein Gewinn');
    expect(at('rests_until', '2026-11-20T08:30:00Z')).toMatch(/^ruht bis .*20\.11\.2026.*08:30/);
    expect(at('rests_until_change')).toBe('ruht, bis sich die Datei ändert');
    expect(at('due')).toBe('fällig');
  });

  it('name outcomes, runners and sizes of the history', () => {
    expect(outcomeText('replaced')).toBe('ersetzt');
    expect(outcomeText('no_gain')).toBe('kein Gewinn');
    expect(outcomeText('failed')).toBe('fehlgeschlagen');
    expect(outcomeText('refused')).toBe('abgelehnt');
    expect(outcomeText(null)).toBe('läuft');
    expect(runnerText('server')).toBe('Server');
    expect(runnerText('cli')).toBe('CLI');
    const entry = {
      title: 'Heat (1995)',
      started_at: '2026-11-18T08:00:00Z',
      runner: 'server',
      outcome: 'replaced',
      reason: null,
      size_before: 40 * GB,
      size_after: 10 * GB,
    };
    expect(sizesText(entry)).toBe('40,0 GB → 10,0 GB (75 % kleiner)');
    expect(sizesText({ ...entry, outcome: 'no_gain', size_after: 35 * GB })).toBe(
      '40,0 GB → 35,0 GB',
    );
    expect(sizesText({ ...entry, outcome: 'failed', size_after: null })).toBe('40,0 GB');
  });

  it('list the reasons of left-out files that have any', () => {
    expect(
      exclusionTexts({
        no_media_info: 2,
        resolution_off: 0,
        unknown_duration: 1,
        dynamic_metadata: 0,
      }),
    ).toEqual(['2 × keine Media-Infos', '1 × Laufzeit unbekannt']);
  });

  it('name a candidate by its movie or first episode, and link its page', () => {
    expect(targetBody(candidate)).toEqual({ movie_id: 4 });
    const episode = {
      ...candidate,
      kind: 'episode',
      movie_id: null,
      series_id: 7,
      episode_ids: [21, 22],
    };
    expect(targetBody(episode)).toEqual({ episode_id: 21 });
    expect(candidateHref(candidate)).toBe('/filme/4');
    expect(candidateHref(episode)).toBe('/serien/7');
  });

  it('say why "Jetzt encodieren" is not available', () => {
    const page = {
      shadow: false,
      running: null,
    } as unknown as ReencodePage;
    expect(encodeBlocked(page)).toBeNull();
    expect(encodeBlocked({ ...page, shadow: true })).toBe('im Shadow-Mode');
    expect(
      encodeBlocked({
        ...page,
        running: { title: 'Heat (1995)', path: '', started_at: '', runner: 'cli', attempt: null },
      }),
    ).toBe('es läuft bereits ein Encode: Heat (1995)');
  });
});
