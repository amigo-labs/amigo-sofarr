import { expect, type APIRequestContext } from '@playwright/test';
import type { ImdbInfo } from '../src/lib/api/generated/ImdbInfo';
import type { SubtitleOverview } from '../src/lib/api/generated/SubtitleOverview';
import { SERVERS } from './servers';

/** Shadow mode, IMDb ratings from a local dataset (`e2e/seed-extras.mjs`). */
export const RATINGS = SERVERS.ratings;
/** Live mode, ratings off, a fake OpenSubtitles, no TMDB (`e2e/seed-extras.mjs`). */
export const EXTRAS = SERVERS.extras;

/** Waits until the ratings server loaded its dataset (it does at start). */
export async function waitForRatings(request: APIRequestContext): Promise<void> {
  await expect
    .poll(
      async () => {
        const info = (await (await request.get(`${RATINGS}/api/imdb`)).json()) as ImdbInfo;
        return info.loaded_at !== null;
      },
      { timeout: 60_000 },
    )
    .toBe(true);
}

/** Waits until the first subtitle run of the extras server searched every file. */
export async function waitForSubtitleRun(request: APIRequestContext): Promise<void> {
  await expect
    .poll(
      async () => {
        const answer = await request.get(`${EXTRAS}/api/subtitles`);
        const overview = (await answer.json()) as SubtitleOverview;
        return (
          overview.recent.length > 0 &&
          overview.needed.length > 0 &&
          overview.needed.every((n) => n.last_search !== null)
        );
      },
      { timeout: 60_000 },
    )
    .toBe(true);
}
