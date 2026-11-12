// The add dialog's choices (spec discover AC-07..AC-11): filled from the add options and
// their defaults, turned into the request body, and the answer as German text.

import type { AddAnswer } from '../api/generated/AddAnswer';
import type { AddOptions } from '../api/generated/AddOptions';
import type { MovieAddBody } from '../api/generated/MovieAddBody';
import type { SeasonInfo } from '../api/generated/SeasonInfo';
import type { SeriesAddBody } from '../api/generated/SeriesAddBody';
import type { TitleKind } from '../api/generated/TitleKind';
import type { ApiFailure } from '../api/http';

/** The title the add dialog is open for. */
export type AddTarget = {
  readonly kind: TitleKind;
  readonly tmdbId: number;
  readonly title: string;
  /** Seasons from a preview; the dialog loads them when missing. */
  readonly seasons: readonly SeasonInfo[] | null;
};

export type AddForm = {
  profile: number | null;
  rootFolder: string | null;
  monitored: boolean;
  minimumAvailability: string;
  tags: number[];
  search: boolean;
  /** Series only: the monitored seasons. */
  seasons: number[];
  monitorNewSeasons: boolean;
};

/** Seasons monitored at first: all but the specials (season 0). */
export const defaultSeasons = (seasons: readonly SeasonInfo[]): number[] =>
  seasons.filter((s) => s.number !== 0).map((s) => s.number);

/** The dialog's first state: the stored defaults of the media kind (AC-10). */
export function formOf(options: AddOptions, seasons: readonly SeasonInfo[]): AddForm {
  const d = options.defaults;
  return {
    profile: d.quality_profile_id,
    rootFolder: d.root_folder,
    monitored: d.monitored,
    minimumAvailability: d.minimum_availability,
    tags: [...d.tags],
    search: d.search,
    seasons: defaultSeasons(seasons),
    monitorNewSeasons: d.monitor_new_seasons,
  };
}

/** What is still missing before the title can be added; `null` when nothing is. */
export function missingChoice(form: AddForm): string | null {
  if (form.profile === null) return 'Bitte ein Qualitätsprofil wählen.';
  if (form.rootFolder === null) return 'Bitte einen Stammordner wählen.';
  return null;
}

export function movieBody(
  tmdbId: number,
  form: AddForm,
  confirmExcluded: boolean,
): MovieAddBody | null {
  if (form.profile === null || form.rootFolder === null) return null;
  return {
    tmdb_id: tmdbId,
    quality_profile_id: form.profile,
    root_folder: form.rootFolder,
    monitored: form.monitored,
    minimum_availability: form.minimumAvailability,
    tags: [...form.tags],
    search: form.search,
    confirm_excluded: confirmExcluded,
  };
}

export function seriesBody(
  tmdbId: number,
  form: AddForm,
  confirmExcluded: boolean,
): SeriesAddBody | null {
  if (form.profile === null || form.rootFolder === null) return null;
  return {
    tmdb_id: tmdbId,
    quality_profile_id: form.profile,
    root_folder: form.rootFolder,
    monitored: form.monitored,
    seasons: form.seasons.toSorted((a, b) => a - b),
    monitor_new_seasons: form.monitorNewSeasons,
    tags: [...form.tags],
    search: form.search,
    confirm_excluded: confirmExcluded,
  };
}

/** The excluded title must be confirmed (HTTP 428, AC-11). */
export const needsConfirmation = (failure: ApiFailure): boolean => failure.status === 428;

/** The notice after adding (AC-07, AC-08). */
export function addedText(answer: AddAnswer): string {
  const parts = [`„${answer.title}“ wurde hinzugefügt (${answer.path}).`];
  if (answer.files_found > 0) {
    parts.push(
      answer.files_found === 1
        ? 'Eine vorhandene Datei wurde übernommen.'
        : `${answer.files_found} vorhandene Dateien wurden übernommen.`,
    );
  }
  if (answer.searching) parts.push('Die Suche läuft.');
  return parts.join(' ');
}

/** Toggles a number in a list, keeping the list's order. */
export const toggled = (list: readonly number[], value: number, on: boolean): number[] =>
  on ? (list.includes(value) ? [...list] : [...list, value]) : list.filter((v) => v !== value);
