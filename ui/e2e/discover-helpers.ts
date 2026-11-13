import { expect, type APIRequestContext, type Locator, type Page } from '@playwright/test';
import { portBase } from './env.mjs';
import { SERVERS } from './servers';

/** The discover server: fake TMDB, TVDB and IMDb data, a small library (`discover-seed.mjs`). */
export const DISCOVER = SERVERS.discover;
/** The fake TMDB with IMDb ratings switched off. */
export const PLAIN = SERVERS.discoverPlain;
/** The fake TMDB of `DISCOVER` (`FAKE_PORTS` in `discover-seed.mjs`). */
const FAKE = `http://127.0.0.1:${portBase() + 30}`;

/** Lets the fake TMDB answer for a title it holds back (its rating stays pending until then). */
export async function release(
  request: APIRequestContext,
  kind: 'movie' | 'tv',
  id: number,
): Promise<void> {
  const answer = await request.post(`${FAKE}/control/release/${kind}/${id}`);
  expect(answer.ok()).toBe(true);
}

/** The title cards of one title (a title can be on several shelves). */
export const cards = (scope: Page | Locator, key: string): Locator =>
  scope.locator(`[data-testid="title-card"][data-key="${key}"]`);

/** A page has its heading and nothing is loading any more. */
export async function settle(page: Page): Promise<void> {
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  await expect(page.getByText(/wird geladen|werden geladen/)).toHaveCount(0);
}
