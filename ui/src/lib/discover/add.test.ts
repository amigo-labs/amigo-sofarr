import { describe, expect, it } from 'vitest';
import type { AddOptions } from '../api/generated/AddOptions';
import {
  addedText,
  defaultSeasons,
  formOf,
  missingChoice,
  movieBody,
  needsConfirmation,
  seriesBody,
  toggled,
} from './add';

const OPTIONS: AddOptions = {
  profiles: [
    { id: 4, name: 'HD-1080p' },
    { id: 6, name: 'UHD' },
  ],
  root_folders: ['/filme', '/filme2'],
  tags: [{ id: 1, name: 'kinder' }],
  minimum_availability: ['announced', 'inCinemas', 'released'],
  defaults: {
    quality_profile_id: 4,
    root_folder: null,
    monitored: true,
    minimum_availability: 'released',
    tags: [],
    search: true,
    monitor_new_seasons: true,
  },
};

const SEASONS = [0, 1, 2].map((n) => ({
  number: n,
  name: n === 0 ? 'Specials' : `Staffel ${n}`,
  episode_count: 2,
  air_date: null,
}));

describe('add dialog (discover AC-07..AC-11)', () => {
  it('starts with the defaults; specials are not monitored', () => {
    const form = formOf(OPTIONS, SEASONS);
    expect(form).toMatchObject({ profile: 4, rootFolder: null, monitored: true, search: true });
    expect(form.seasons).toEqual([1, 2]);
    expect(defaultSeasons([])).toEqual([]);
  });

  it('asks for a root folder when there are several and none is stored', () => {
    const form = formOf(OPTIONS, []);
    expect(missingChoice(form)).toBe('Bitte einen Stammordner wählen.');
    expect(movieBody(1, form, false)).toBeNull();
    expect(missingChoice({ ...form, profile: null })).toBe('Bitte ein Qualitätsprofil wählen.');
  });

  it('builds the request bodies', () => {
    const form = { ...formOf(OPTIONS, SEASONS), rootFolder: '/filme', tags: [1] };
    expect(movieBody(603, form, true)).toEqual({
      tmdb_id: 603,
      quality_profile_id: 4,
      root_folder: '/filme',
      monitored: true,
      minimum_availability: 'released',
      tags: [1],
      search: true,
      confirm_excluded: true,
    });
    const body = seriesBody(9, { ...form, seasons: [2, 0], monitorNewSeasons: false }, false);
    expect(body).toMatchObject({
      seasons: [0, 2],
      monitor_new_seasons: false,
      confirm_excluded: false,
    });
  });

  it('recognizes the confirmation an excluded title needs', () => {
    expect(needsConfirmation({ kind: 'client', status: 428, message: 'x' })).toBe(true);
    expect(needsConfirmation({ kind: 'client', status: 409, message: 'x' })).toBe(false);
  });

  it('says what was added in German', () => {
    const answer = {
      kind: 'movie' as const,
      id: 7,
      title: 'Sternenfahrt',
      path: '/filme/Sternenfahrt (2024)',
      files_found: 0,
      searching: true,
    };
    expect(addedText(answer)).toBe(
      '„Sternenfahrt“ wurde hinzugefügt (/filme/Sternenfahrt (2024)). Die Suche läuft.',
    );
    expect(addedText({ ...answer, files_found: 2, searching: false })).toContain(
      '2 vorhandene Dateien wurden übernommen.',
    );
  });

  it('toggles choices', () => {
    expect(toggled([1, 2], 3, true)).toEqual([1, 2, 3]);
    expect(toggled([1, 2], 2, true)).toEqual([1, 2]);
    expect(toggled([1, 2], 1, false)).toEqual([2]);
  });
});
