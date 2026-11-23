// Spec ui-list-toolbar AC-01 .. AC-10: the header and the one-row toolbar of the movie and
// series lists, the filter panel, and the active filters as chips.

import { expect, test, type Page } from '@playwright/test';
import { expectAttribution } from './helpers';
import { RATINGS, waitForRatings } from './extras-helpers';
import { LIBRARY, apiTotal, expectTotal, items, noListRows, openFilters } from './library-helpers';

const toolbar = (page: Page) => page.getByRole('toolbar', { name: 'Liste' });
const filterButton = (page: Page) => page.getByRole('button', { name: /^Filter( \(\d+\))?$/ });
const chips = (page: Page) => page.getByRole('list', { name: 'Aktive Filter' }).getByRole('button');

test.describe('header and toolbar (AC-01)', () => {
  test('heading and count in one row, the toolbar in one row, the list starts high', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    // The list starts high while no row stands above it (spec ui-cinema-library-rows AC-06).
    await noListRows(page);
    await page.goto(`${LIBRARY}/filme`);
    await expect(items(page).first()).toBeVisible();
    const heading = await page.getByRole('heading', { level: 1, name: 'Filme' }).boundingBox();
    const total = await page.getByTestId('total').boundingBox();
    expect(
      Math.abs(
        (heading?.y ?? 0) +
          (heading?.height ?? 0) / 2 -
          ((total?.y ?? 0) + (total?.height ?? 0) / 2),
      ),
    ).toBeLessThan(12);
    // The toolbar's controls in order, on one row.
    const bar = toolbar(page);
    const controls = [
      bar.getByLabel('Suche'),
      bar.getByLabel('Sortierung', { exact: true }),
      bar.getByRole('button', { name: /sortieren$/ }),
      bar.getByRole('group', { name: 'Ansicht' }),
      bar.getByRole('button', { name: /^Filter/ }),
    ];
    const boxes = [];
    for (const control of controls) {
      await expect(control).toBeVisible();
      boxes.push(await control.boundingBox());
    }
    for (let i = 1; i < boxes.length; i += 1) {
      expect(boxes[i]?.x ?? 0, `control ${i} right of control ${i - 1}`).toBeGreaterThan(
        boxes[i - 1]?.x ?? 0,
      );
      expect(
        Math.abs((boxes[i]?.y ?? 0) - (boxes[0]?.y ?? 0)),
        `control ${i} on the first row`,
      ).toBeLessThan(8);
    }
    const main = await page.locator('main').boundingBox();
    const first = await items(page).first().boundingBox();
    expect((first?.y ?? 0) - (main?.y ?? 0)).toBeLessThanOrEqual(200);
  });
});

test.describe('sort direction (AC-02)', () => {
  test('the button flips the direction like the dropdown did, kept in the address', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/filme?suche=Film%20000&sortierung=jahr`);
    const descending = toolbar(page).getByRole('button', { name: 'Absteigend sortieren' });
    await expect(descending).toBeVisible();
    await descending.click();
    await expect(page).toHaveURL(/richtung=ab/);
    await expect(items(page).first()).toContainText('Film 0009');
    const ascending = toolbar(page).getByRole('button', { name: 'Aufsteigend sortieren' });
    await expect(ascending).toBeVisible();
    await ascending.click();
    await expect(page).not.toHaveURL(/richtung=/);
    await expect(items(page).first()).toContainText('Film 0001');
  });
});

test.describe('filter button and panel (AC-03, AC-04)', () => {
  test('reads Filter, then Filter (n); opens and closes the panel; closed at every load', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/filme`);
    await expect(filterButton(page)).toHaveText('Filter');
    await expect(filterButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(page.getByLabel('Überwacht', { exact: true })).toBeHidden();
    await filterButton(page).click();
    await expect(filterButton(page)).toHaveAttribute('aria-expanded', 'true');
    for (const label of [
      'Überwacht',
      'Fehlt',
      'Cutoff nicht erreicht',
      'Mit Datei',
      'Qualitätsprofil',
      'Tag',
    ]) {
      await expect(page.getByLabel(label, { exact: true })).toBeVisible();
    }
    await page.getByLabel('Überwacht', { exact: true }).selectOption({ label: 'Ja' });
    await page.getByLabel('Fehlt', { exact: true }).selectOption({ label: 'Nein' });
    await expect(filterButton(page)).toHaveText('Filter (2)');
    // The panel stays open while a dropdown changes.
    await expect(page.getByLabel('Fehlt', { exact: true })).toBeVisible();
    await filterButton(page).click();
    await expect(filterButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(filterButton(page)).toBeFocused();
    await expect(page.getByLabel('Überwacht', { exact: true })).toBeHidden();
    await page.reload();
    await expect(filterButton(page)).toHaveText('Filter (2)');
    await expect(filterButton(page)).toHaveAttribute('aria-expanded', 'false');
  });

  test('search text alone is no filter', async ({ page }) => {
    await page.goto(`${LIBRARY}/filme?suche=Boot`);
    await expect(filterButton(page)).toHaveText('Filter');
    await expect(chips(page)).toHaveCount(0);
  });
});

test.describe('chips (AC-05 .. AC-08)', () => {
  test('name the active filters in order, also with the panel closed (AC-05, AC-08)', async ({
    page,
  }) => {
    await page.goto(
      `${LIBRARY}/filme?ueberwacht=ja&datei=nein&profil=4&tag=1&suche=Film&sortierung=jahr&richtung=ab`,
    );
    await expect(chips(page)).toHaveText([
      'Überwacht: Ja',
      'Mit Datei: Nein',
      'Qualitätsprofil: 1080p',
      'Tag: kinder',
    ]);
    await expect(chips(page).first()).toHaveAccessibleName('Filter Überwacht: Ja entfernen');
    await expect(toolbar(page).getByLabel('Suche')).toHaveValue('Film');
    await expect(toolbar(page).getByLabel('Sortierung', { exact: true })).toHaveValue('year');
    await expect(
      toolbar(page).getByRole('button', { name: 'Aufsteigend sortieren' }),
    ).toBeVisible();
  });

  test('a chip removes its filter; the focus moves on (AC-06)', async ({ page, request }) => {
    const expected = await apiTotal(request, '/api/library/movies?has_file=no&tag=1');
    await page.goto(`${LIBRARY}/filme?ueberwacht=ja&datei=nein&tag=1`);
    await chips(page).first().click();
    await expect(page).not.toHaveURL(/ueberwacht=/);
    await expectTotal(page, expected, expected === 1 ? 'Film' : 'Filme');
    await expect(chips(page)).toHaveText(['Mit Datei: Nein', 'Tag: kinder']);
    await expect(chips(page).first()).toBeFocused();
    // The last chip: the focus goes to the one before it.
    await chips(page).last().click();
    await expect(chips(page)).toHaveText(['Mit Datei: Nein']);
    await expect(chips(page).first()).toBeFocused();
    await chips(page).first().click();
    await expect(chips(page)).toHaveCount(0);
    await expect(filterButton(page)).toBeFocused();
    await expect(filterButton(page)).toHaveText('Filter');
  });

  test('"Alle Filter zurücksetzen" clears every filter, keeps search and sort (AC-07)', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/filme?ueberwacht=ja`);
    await expect(page.getByRole('button', { name: 'Alle Filter zurücksetzen' })).toHaveCount(0);
    await page.goto(`${LIBRARY}/filme?ueberwacht=ja&fehlt=nein&suche=Film&sortierung=jahr`);
    await expect(items(page).first()).toBeVisible();
    const requests: string[] = [];
    page.on('request', (r) => {
      // The first page of a new list; scrolling may load more pages after it.
      if (r.url().includes('/api/library/movies') && r.url().includes('page=1&')) {
        requests.push(r.url());
      }
    });
    await page.getByRole('button', { name: 'Alle Filter zurücksetzen' }).click();
    await expect(chips(page)).toHaveCount(0);
    await expect(page).toHaveURL(/\?suche=Film&sortierung=jahr$/);
    await expect(filterButton(page)).toBeFocused();
    await expect(items(page).first()).toBeVisible();
    expect(requests).toHaveLength(1);
  });
});

test.describe('attribution (AC-09)', () => {
  test("IMDb's attribution stands once, below the list", async ({ page, request }) => {
    await waitForRatings(request);
    await page.goto(`${RATINGS}/filme`);
    await expect(items(page).first()).toBeVisible();
    await expectAttribution(page);
    await expect(page.locator('.imdb-attribution')).toHaveCount(1);
    const attribution = await page.locator('.imdb-attribution').boundingBox();
    const list = await page.getByTestId('library-items').boundingBox();
    expect(attribution?.y ?? 0).toBeGreaterThan((list?.y ?? 0) + (list?.height ?? 0) - 1);
  });
});

test.describe('narrow window (AC-10)', () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test('the toolbar wraps, the search takes the full width, nothing scrolls sideways', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/filme?ueberwacht=ja&tag=1`);
    await expect(items(page).first()).toBeVisible();
    await openFilters(page);
    const search = await toolbar(page).getByLabel('Suche').boundingBox();
    const bar = await toolbar(page).boundingBox();
    expect(search?.width ?? 0).toBeGreaterThan((bar?.width ?? 0) - 2);
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
  });
});

test.describe('narrow window between 600 and 767 px (AC-10)', () => {
  test.use({ viewport: { width: 700, height: 800 } });

  test('the panel shows its dropdowns in at most two columns; the chips wrap', async ({ page }) => {
    await page.goto(
      `${LIBRARY}/filme?ueberwacht=ja&fehlt=nein&cutoff=nein&datei=ja&profil=4&tag=1`,
    );
    await openFilters(page);
    const columns = await page
      .locator('#liste-filter .field')
      .evaluateAll(
        (all) => new Set(all.map((el) => Math.round(el.getBoundingClientRect().x))).size,
      );
    expect(columns).toBeGreaterThanOrEqual(1);
    expect(columns).toBeLessThanOrEqual(2);
    const rows = await chips(page).evaluateAll(
      (all) => new Set(all.map((el) => Math.round(el.getBoundingClientRect().y))).size,
    );
    expect(rows).toBeGreaterThan(1);
    const fits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    expect(fits).toBe(true);
  });
});

test('series have the Komplett filter and its chip', async ({ page }) => {
  await page.goto(`${LIBRARY}/serien?komplett=ja`);
  await expect(chips(page)).toHaveText(['Komplett: Ja']);
  await openFilters(page);
  await expect(page.getByLabel('Komplett', { exact: true })).toHaveValue('yes');
  await expect(page.getByLabel('Mit Datei', { exact: true })).toHaveCount(0);
});
