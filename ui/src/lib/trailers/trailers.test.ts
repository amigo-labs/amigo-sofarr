import { describe, expect, it } from 'vitest';
import type { TrailerInfo } from '../api/generated/TrailerInfo';
import { downloadResultText, hasTrailer, STATUS_TEXT, supportText } from './trailers';

const info: TrailerInfo = {
  status: 'downloaded',
  files: ['trailers/Heat (1995) - Trailer (Deutsch).mkv'],
  language: 'Deutsch',
  link: 'https://www.youtube.com/watch?v=abcdefghijk',
  message: null,
  attempts: 1,
  next_attempt: null,
  blocked: null,
};

describe('trailer texts (trailers AC-09, AC-10, AC-12)', () => {
  it('tell whether there is a trailer', () => {
    expect(hasTrailer(info)).toBe(true);
    expect(hasTrailer({ ...info, status: 'foreign' })).toBe(true);
    expect(hasTrailer({ ...info, status: 'failed' })).toBe(false);
    expect(STATUS_TEXT.low_space.text).toBe('zu wenig Speicherplatz');
  });

  it('report a manual download in German', () => {
    expect(downloadResultText('Heat', info)).toBe('Trailer für „Heat“ geladen (Deutsch).');
    expect(downloadResultText('Heat', { ...info, language: null })).toBe(
      'Trailer für „Heat“ geladen.',
    );
    expect(
      downloadResultText('Heat', {
        ...info,
        status: 'failed',
        message: 'Video nicht verfügbar',
        language: null,
      }),
    ).toBe('Kein Trailer für „Heat“ geladen: Video nicht verfügbar');
    expect(downloadResultText('Heat', { ...info, status: 'no_candidate', message: null })).toBe(
      'Kein Trailer für „Heat“ geladen: kein Trailer gefunden',
    );
  });

  it('say on the system page whether trailers are downloaded', () => {
    expect(supportText({ active: true, message: null })).toBe('werden heruntergeladen');
    expect(supportText({ active: false, message: '[tmdb] fehlt in der Konfiguration' })).toBe(
      'keine Downloads – [tmdb] fehlt in der Konfiguration',
    );
    expect(supportText({ active: false, message: null })).toBe('keine Downloads – Grund unbekannt');
  });
});
