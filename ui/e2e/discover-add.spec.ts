// Adding titles and person import lists (spec discover AC-06..AC-12) against the real
// binary with a fake TMDB and TVDB. One after the other: adding stores the defaults.

import { expect, test, type Page } from '@playwright/test';
import type { AddOptions } from '../src/lib/api/generated/AddOptions';
import type { MovieDetail } from '../src/lib/api/generated/MovieDetail';
import type { SeriesDetail } from '../src/lib/api/generated/SeriesDetail';
import { DISCOVER, cards } from './discover-helpers';

test.describe.configure({ mode: 'serial' });

const dialog = (page: Page) => page.getByRole('dialog');

async function openShelf(page: Page, shelf: string) {
  await page.goto(`${DISCOVER}/entdecken`);
  const section = page
    .getByTestId('shelf')
    .filter({ has: page.getByRole('heading', { name: shelf }) });
  await expect(section).toBeVisible();
  return section;
}

test('a movie added from a shelf goes into the library with the chosen settings (AC-07, AC-10)', async ({
  page,
}) => {
  const shelf = await openShelf(page, 'Demnächst im Kino');
  const card = cards(shelf, 'movie/510');
  await card.getByRole('button', { name: 'Hinzufügen „Standardwerte“' }).click();
  await expect(
    dialog(page).getByRole('heading', { name: '„Standardwerte“ hinzufügen' }),
  ).toBeVisible();
  // First-time defaults: first profile, the only root folder, monitored, released, search.
  await expect(dialog(page).getByLabel('Qualitätsprofil')).toHaveValue('4');
  await expect(dialog(page).getByLabel('Stammordner')).toHaveValue(/media\/filme$/);
  await expect(dialog(page).getByLabel('Mindestverfügbarkeit')).toHaveValue('released');
  await expect(dialog(page).getByLabel('Überwachen')).toBeChecked();
  await expect(dialog(page).getByLabel('kinder')).not.toBeChecked();
  await expect(dialog(page).getByLabel('Suchen, sobald verfügbar')).toBeChecked();

  await dialog(page).getByLabel('kinder').check();
  await dialog(page).getByLabel('Suchen, sobald verfügbar').uncheck();
  await dialog(page).getByLabel('Mindestverfügbarkeit').selectOption({ label: 'im Kino' });
  await dialog(page).getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(dialog(page)).toBeHidden();
  await expect(
    page.getByText(/„Standardwerte“ wurde hinzugefügt \(.*Standardwerte \(2016\)\)\./),
  ).toBeVisible();
  // The card changes its state without a reload and links to the library page.
  await expect(card.getByText('In der Bibliothek')).toBeVisible();
  await card.getByRole('link', { name: 'Standardwerte' }).click();
  await expect(page).toHaveURL(/\/filme\/\d+$/);
  const id = new URL(page.url()).pathname.split('/').pop() ?? '';
  const movie = (await (
    await page.request.get(`${DISCOVER}/api/library/movies/${id}`)
  ).json()) as MovieDetail;
  expect(movie.minimum_availability).toBe('inCinemas');
  expect(movie.tags.map((t) => t.name)).toEqual(['kinder']);

  // The choices are the new defaults of movies.
  const options = (await (
    await page.request.get(`${DISCOVER}/api/discover/add/movie/options`)
  ).json()) as AddOptions;
  expect(options.defaults).toMatchObject({
    tags: [1],
    search: false,
    minimum_availability: 'inCinemas',
  });
  await page.goto(`${DISCOVER}/entdecken/filme/506`);
  await page.getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(dialog(page).getByLabel('kinder')).toBeChecked();
  await expect(dialog(page).getByLabel('Suchen, sobald verfügbar')).not.toBeChecked();
  await dialog(page).getByRole('button', { name: 'Abbrechen' }).click();
  await expect(dialog(page)).toBeHidden();
});

test('an existing folder is used and its file taken up (AC-07)', async ({ page }) => {
  const shelf = await openShelf(page, 'Jetzt im Kino');
  await cards(shelf, 'movie/508').getByRole('button', { name: 'Hinzufügen „Mit Ordner“' }).click();
  await dialog(page).getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(
    page.getByText(/„Mit Ordner“ wurde hinzugefügt .* Eine vorhandene Datei wurde übernommen\./),
  ).toBeVisible();
});

test('an excluded title is added only after confirming (AC-11)', async ({ page }) => {
  const shelf = await openShelf(page, 'Jetzt im Kino');
  const card = cards(shelf, 'movie/503');
  await expect(card.getByTestId('library-state')).toHaveText('Ausgeschlossen');
  await card.getByRole('button', { name: 'Hinzufügen „Ausgeschlossen“' }).click();
  await dialog(page).getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(
    dialog(page).getByText(/steht auf der Ausschlussliste der Importlisten/),
  ).toBeVisible();
  await dialog(page).getByRole('button', { name: 'Trotzdem hinzufügen' }).click();
  await expect(dialog(page)).toBeHidden();
  await expect(card.getByText('In der Bibliothek')).toBeVisible();
});

test('a title added meanwhile is refused with a German message (AC-11)', async ({ page }) => {
  const shelf = await openShelf(page, 'Demnächst im Kino');
  const card = cards(shelf, 'movie/509');
  await card.getByRole('button', { name: 'Hinzufügen „Doppelt“' }).click();
  await expect(dialog(page).getByLabel('Qualitätsprofil')).toBeVisible();
  // Added in the meantime, as from a second tab.
  const options = (await (
    await page.request.get(`${DISCOVER}/api/discover/add/movie/options`)
  ).json()) as AddOptions;
  const other = await page.request.post(`${DISCOVER}/api/discover/add/movie`, {
    data: {
      tmdb_id: 509,
      quality_profile_id: options.defaults.quality_profile_id,
      root_folder: options.defaults.root_folder,
      monitored: true,
      minimum_availability: 'released',
      tags: [],
      search: false,
      confirm_excluded: false,
    },
  });
  expect(other.ok()).toBe(true);
  await dialog(page).getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(
    dialog(page).getByText('Nicht hinzugefügt: „Doppelt“ ist schon in der Bibliothek'),
  ).toBeVisible();
  await dialog(page).getByRole('button', { name: 'Abbrechen' }).click();
  // The page learns the new state by itself.
  await expect(card.getByText('In der Bibliothek')).toBeVisible({ timeout: 25_000 });
});

test('a series is added with the chosen seasons, without specials by default (AC-08, AC-06)', async ({
  page,
}) => {
  await page.goto(`${DISCOVER}/entdecken/serien/601`);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Großes Meer (2020)');
  await page.getByRole('button', { name: 'Hinzufügen' }).click();
  const d = dialog(page);
  await expect(d.getByLabel('Specials (2 Folgen)')).not.toBeChecked();
  await expect(d.getByLabel('Staffel 1 (2 Folgen)')).toBeChecked();
  await expect(d.getByLabel('Staffel 2 (2 Folgen)')).toBeChecked();
  await expect(d.getByLabel('Neue Staffeln überwachen')).toBeChecked();
  await d.getByLabel('Staffel 2 (2 Folgen)').uncheck();
  await d.getByLabel('Fehlende Folgen gleich suchen').uncheck();
  await d.getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(page.getByText(/„Großes Meer“ wurde hinzugefügt/)).toBeVisible();
  await page.getByRole('link', { name: 'Zur Bibliothek' }).click();
  await expect(page).toHaveURL(/\/serien\/\d+$/);
  const id = Number(new URL(page.url()).pathname.split('/').pop());
  const series = (await (
    await page.request.get(`${DISCOVER}/api/library/series/${id}`)
  ).json()) as SeriesDetail;
  expect(series.seasons.map((s) => [s.number, s.episodes_monitored])).toEqual([
    [0, 0],
    [1, 2],
    [2, 0],
  ]);
  // A series added from discover has its TMDB id: the cast shows.
  await expect(
    page.getByRole('list', { name: 'Besetzung' }).getByRole('link', { name: 'Anna Beispiel' }),
  ).toBeVisible();
});

test('a series TVDB cannot give is not added, with a German reason (AC-09)', async ({ page }) => {
  await page.goto(`${DISCOVER}/entdecken/serien/602`);
  await page.getByRole('button', { name: 'Hinzufügen' }).click();
  await dialog(page).getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(
    dialog(page).getByText(/Nicht hinzugefügt: TMDB kennt keine TVDB-ID für „Ohne TVDB“/),
  ).toBeVisible();
  await dialog(page).getByRole('button', { name: 'Abbrechen' }).click();

  await page.goto(`${DISCOVER}/entdecken/serien/604`);
  await page.getByRole('button', { name: 'Hinzufügen' }).click();
  await dialog(page).getByRole('button', { name: 'Hinzufügen' }).click();
  await expect(
    dialog(page).getByText(/Nicht hinzugefügt: TVDB ist nicht erreichbar/),
  ).toBeVisible();
});

test('a person import list is created once (AC-12)', async ({ page }) => {
  await page.goto(`${DISCOVER}/entdecken/personen/9000`);
  await page.getByRole('button', { name: 'Import-Liste anlegen' }).click();
  const d = dialog(page);
  await expect(
    d.getByRole('heading', { name: 'Import-Liste für Anna Beispiel anlegen' }),
  ).toBeVisible();
  await expect(d.getByLabel('Besetzung')).toBeChecked();
  await expect(d.getByLabel('Regie')).not.toBeChecked();
  await d.getByLabel('Regie').check();
  await d.getByRole('button', { name: 'Import-Liste anlegen' }).click();
  await expect(d).toBeHidden();
  await expect(page.getByText(/Import-Liste „.*“ angelegt/)).toBeVisible();

  await page.getByRole('button', { name: 'Import-Liste anlegen' }).click();
  await d.getByRole('button', { name: 'Import-Liste anlegen' }).click();
  await expect(
    d.getByText(/Nicht angelegt: Für Anna Beispiel gibt es schon eine Importliste/),
  ).toBeVisible();
});
