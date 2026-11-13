import { describe, expect, it } from 'vitest';
import type { SubtitleItem } from '../api/generated/SubtitleItem';
import type { SubtitleSearchReport } from '../api/generated/SubtitleSearchReport';
import {
  itemHref,
  kindText,
  languageName,
  matchText,
  needKindText,
  searchReportText,
  targetKey,
  targetOf,
} from './subtitles';

const movie: SubtitleItem = {
  kind: 'movie',
  movie_id: 4,
  series_id: null,
  episode_id: null,
  title: 'Der Film (2019)',
};
const episode: SubtitleItem = {
  kind: 'episode',
  movie_id: null,
  series_id: 2,
  episode_id: 31,
  title: 'Kleine Insel – S01E02',
};

const report: SubtitleSearchReport = {
  searched: 0,
  downloaded: [],
  would_download: [],
  not_found: 0,
  waiting_for_quota: 0,
  failed: 0,
};

describe('subtitle entries (subtitles AC-11, AC-12)', () => {
  it('find the file, its group and the page of the item', () => {
    expect(targetOf(movie)).toEqual({ kind: 'movie', movieId: 4 });
    expect(targetOf(episode)).toEqual({ kind: 'episode', seriesId: 2, episodeId: 31 });
    expect(targetOf({ ...episode, episode_id: null })).toBeNull();
    expect(targetKey(movie)).toBe('movie-4');
    expect(targetKey(episode)).toBe('episode-31');
    expect(itemHref(movie)).toBe('/filme/4');
    expect(itemHref(episode)).toBe('/serien/2');
    expect(itemHref({ ...movie, movie_id: null })).toBeNull();
  });

  it('name languages in German', () => {
    expect(languageName('de')).toBe('Deutsch');
    expect(languageName('en')).toBe('Englisch');
    expect(languageName('fr')).toBe('Französisch');
    expect(languageName('und')).toBe('Sprache unbekannt');
    expect(languageName(null)).toBe('Sprache unbekannt');
    expect(languageName('qqq')).toBe('QQQ');
    expect(languageName('!!')).toBe('!!');
  });

  it('name kinds and match qualities in German', () => {
    expect(kindText(false, false)).toBe('normal');
    expect(kindText(true, false)).toBe('erzwungen');
    expect(kindText(false, true)).toBe('normal, für Hörgeschädigte');
    expect(needKindText('forced')).toBe('erzwungen');
    expect(needKindText('normal', true)).toBe('normal, für Hörgeschädigte');
    expect(matchText('hash')).toBe('Hash der Videodatei');
    expect(matchText('release:1')).toBe('Release (1 Merkmal)');
    expect(matchText('release:3')).toBe('Release (3 Merkmale)');
    expect(matchText('title')).toBe('nur Titel');
    expect(matchText(null)).toBe('–');
  });

  it('report a search in German', () => {
    expect(searchReportText('Der Film (2019)', report)).toBe(
      '„Der Film (2019)“: Nichts zu suchen – jede gewünschte Sprache ist vorhanden.',
    );
    expect(
      searchReportText('Der Film (2019)', {
        ...report,
        searched: 2,
        downloaded: ['Der Film (2019).de.srt'],
        not_found: 1,
      }),
    ).toBe(
      '„Der Film (2019)“: 2 Untertitel gesucht, 1 geladen (Der Film (2019).de.srt), 1 nicht gefunden.',
    );
    expect(
      searchReportText('X', {
        ...report,
        searched: 3,
        would_download: ['X.2019.1080p-GRP'],
        waiting_for_quota: 1,
        failed: 1,
      }),
    ).toBe(
      '„X“: 3 Untertitel gesucht, Schattenmodus, würde laden: X.2019.1080p-GRP, 1 warten auf das Download-Kontingent, 1 Kandidat nicht ladbar.',
    );
  });
});
