import { expect, type APIRequestContext, type Page } from '@playwright/test';
import type { MovieListItem } from '../src/lib/api/generated/MovieListItem';
import type { Page as ListPage } from '../src/lib/api/generated/Page';
import type { SeriesListItem } from '../src/lib/api/generated/SeriesListItem';
import { SERVERS } from './servers';

export const LIBRARY = SERVERS.library;
export const LIVE = SERVERS.live;

/** The id of the movie with exactly this title on a server. */
export async function movieId(
  request: APIRequestContext,
  base: string,
  title: string,
): Promise<number> {
  const answer = await request.get(`${base}/api/library/movies?q=${encodeURIComponent(title)}`);
  const page = (await answer.json()) as ListPage<MovieListItem>;
  const movie = page.items.find((m) => m.title === title);
  if (movie === undefined) throw new Error(`no movie ${title}`);
  return movie.id;
}

export async function seriesId(
  request: APIRequestContext,
  base: string,
  title: string,
): Promise<number> {
  const answer = await request.get(`${base}/api/library/series?q=${encodeURIComponent(title)}`);
  const page = (await answer.json()) as ListPage<SeriesListItem>;
  const series = page.items.find((s) => s.title === title);
  if (series === undefined) throw new Error(`no series ${title}`);
  return series.id;
}

/** The number of matches the API reports for a list query. */
export async function apiTotal(
  request: APIRequestContext,
  path: string,
  base: string = LIBRARY,
): Promise<number> {
  const answer = await request.get(`${base}${path}`);
  return ((await answer.json()) as ListPage<unknown>).total;
}

/** The rows of the list or grid shown now. */
export const items = (page: Page) => page.getByTestId('library-items').locator(':scope > li');

/** The total the list page shows, e.g. "2501 Filme". */
export async function expectTotal(page: Page, total: number, noun: string): Promise<void> {
  await expect(page.getByTestId('total')).toHaveText(`${total} ${noun}`);
}
