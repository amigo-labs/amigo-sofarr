// Spec ui-overview-pages AC-01 .. AC-09: one page header, the overview's key figures and
// columns, shared tabs on Fehlend, calm list rows, compact system facts, the login card.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Locator, type Page } from '@playwright/test';
import type { OverviewAnswer } from '../src/lib/api/generated/OverviewAnswer';
import { EXTRAS } from './extras-helpers';
import { LIBRARY } from './library-helpers';
import { SERVERS } from './servers';
import { PALETTES, storeTheme, token } from './theme-helpers';

async function box(locator: Locator) {
  const found = await locator.boundingBox();
  if (found === null) throw new Error('not shown');
  return found;
}

const middle = (b: { y: number; height: number }) => b.y + b.height / 2;

async function overviewAnswer(page: Page): Promise<OverviewAnswer> {
  return (await (await page.request.get(`${LIBRARY}/api/overview`)).json()) as OverviewAnswer;
}

test.describe('page header (AC-01)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  for (const [path, testid] of [
    ['/fehlend', 'wanted-total'],
    ['/blocklist', 'blocklist-total'],
  ] as const) {
    test(`${path}: the count sits in the heading's row, right after it`, async ({ page }) => {
      await page.goto(`${LIBRARY}${path}`);
      const count = page.getByTestId(testid);
      await expect(count).toBeVisible();
      const heading = await box(page.getByRole('heading', { level: 1 }));
      const total = await box(count);
      expect(Math.abs(middle(heading) - middle(total))).toBeLessThan(12);
      expect(total.x).toBeGreaterThan(heading.x + heading.width - 1);
    });
  }

  for (const [name, url, api] of [
    ['System', `${SERVERS.open}/system`, '**/api/system'],
    ['Untertitel', `${EXTRAS}/untertitel`, '**/api/subtitles'],
  ] as const) {
    test(`${name}: a failed first load keeps the page header above the error`, async ({ page }) => {
      await page.route(api, (route) =>
        route.fulfill({ status: 503, json: { error: 'Datenbank gesperrt' } }),
      );
      await page.goto(url);
      await expect(page.getByText('Datenbank gesperrt')).toBeVisible();
      const header = page.getByRole('heading', { level: 1, name, exact: true });
      await expect(header).toBeVisible();
      const error = await box(page.getByText('Datenbank gesperrt'));
      expect((await box(header)).y).toBeLessThan(error.y);
    });
  }

  test('Backups: the main action at the end of the heading row; below it on 360 px', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/backups`);
    const create = page.getByRole('button', { name: 'Sicherung erstellen' });
    const heading = await box(page.getByRole('heading', { level: 1 }));
    let action = await box(create);
    const main = await box(page.locator('main'));
    expect(Math.abs(middle(heading) - middle(action))).toBeLessThan(12);
    expect(action.x + action.width).toBeGreaterThan(main.x + main.width - 40);
    await page.setViewportSize({ width: 360, height: 740 });
    action = await box(create);
    const narrow = await box(page.getByRole('heading', { level: 1 }));
    expect(action.y).toBeGreaterThanOrEqual(narrow.y + narrow.height - 1);
  });
});

test.describe('overview (AC-02 .. AC-04)', () => {
  test('three key figures, counted from the boxes, each linking to its box', async ({ page }) => {
    const answer = await overviewAnswer(page);
    await page.goto(`${LIBRARY}/`);
    const figures = page.getByRole('list', { name: 'Kennzahlen' }).getByRole('link');
    await expect(figures).toHaveCount(3);
    await expect(figures.nth(0)).toHaveText(
      new RegExp(`^\\s*${answer.queue.items.length}\\s*Warteschlange\\s*$`),
    );
    await expect(figures.nth(1)).toHaveText(
      new RegExp(`^\\s*${answer.problems.length}\\s*Probleme \\(7 Tage\\)\\s*$`),
    );
    await expect(figures.nth(2)).toHaveText(
      new RegExp(`^\\s*${answer.upcoming.length}\\s*Nächste 7 Tage\\s*$`),
    );
    for (const [i, heading] of [
      'Warteschlange',
      'Probleme der letzten 7 Tage',
      'Die nächsten 7 Tage',
    ].entries()) {
      const href = await figures.nth(i).getAttribute('href');
      expect(href).toMatch(/^#/);
      await expect(page.locator(`${href} `)).toHaveText(heading);
    }
  });

  test('the problems figure is danger while there are problems, neutral without', async ({
    page,
  }) => {
    const real = await overviewAnswer(page);
    const problem: OverviewAnswer['problems'][number] = {
      kind: 'movie',
      grab_id: 1,
      media_id: 1,
      media_title: 'Film 0001',
      release_title: 'Film.0001.1951.1080p-GRP',
      quality: 'Bluray-1080p',
      size: 1_000_000,
      status: 'failed',
      message: 'Download fehlgeschlagen',
      episode_ids: [],
      grabbed_at: '2026-11-16T08:00:00Z',
      updated_at: '2026-11-16T09:00:00Z',
    };
    for (const problems of [[problem], []]) {
      await page.route('**/api/overview', (route) =>
        route.fulfill({ json: { ...real, problems } }),
      );
      await page.goto(`${LIBRARY}/`);
      const tile = page.getByRole('list', { name: 'Kennzahlen' }).getByRole('link').nth(1);
      await expect(tile).toContainText(String(problems.length));
      const ground = await tile.evaluate((el) => getComputedStyle(el).backgroundColor);
      if (problems.length > 0) expect(ground).toBe(token('dark', 'danger-subtle'));
      else expect(ground).not.toBe(token('dark', 'danger-subtle'));
      await page.unroute('**/api/overview');
    }
  });

  test('two columns from 1200 px, one below', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`${LIBRARY}/`);
    const queue = page.getByRole('region', { name: 'Warteschlange' });
    const upcoming = page.getByRole('region', { name: 'Die nächsten 7 Tage' });
    const problems = page.getByRole('region', { name: 'Probleme der letzten 7 Tage' });
    const activity = page.getByRole('region', { name: 'Letzte Aktivität' });
    await expect(activity).toBeVisible();
    const [q, u, p, a] = await Promise.all([queue, upcoming, problems, activity].map(box));
    if (q === undefined || u === undefined || p === undefined || a === undefined) {
      throw new Error('a box is missing');
    }
    expect(Math.round(q.x)).toBe(Math.round(u.x));
    expect(Math.round(p.x)).toBe(Math.round(a.x));
    expect(p.x).toBeGreaterThan(q.x + q.width - 1);
    expect(u.y).toBeGreaterThan(q.y);
    expect(a.y).toBeGreaterThan(p.y);
    await page.setViewportSize({ width: 1100, height: 900 });
    const narrow = await Promise.all([queue, upcoming, problems, activity].map(box));
    for (const r of narrow) expect(Math.round(r.x)).toBe(Math.round(narrow[0]?.x ?? 0));
    for (let i = 1; i < narrow.length; i += 1) {
      expect(narrow[i]?.y ?? 0).toBeGreaterThan(narrow[i - 1]?.y ?? 0);
    }
  });
});

test.describe('overview while loading (AC-02)', () => {
  test('no figures during the first load; the page error, not the figures, after a failed one', async ({
    page,
  }) => {
    const { promise: held, resolve: release } = Promise.withResolvers<void>();
    await page.route('**/api/overview', async (route) => {
      await held;
      await route.fulfill({ status: 503, json: { error: 'Datenbank gesperrt' } });
    });
    await page.goto(`${LIBRARY}/`);
    await expect(page.getByRole('heading', { level: 1, name: 'Übersicht' })).toBeVisible();
    await expect(page.getByText('Übersicht wird geladen …')).toBeVisible();
    await expect(page.getByRole('list', { name: 'Kennzahlen' })).toHaveCount(0);
    release();
    await expect(page.getByText('Datenbank gesperrt')).toBeVisible();
    await expect(page.getByRole('list', { name: 'Kennzahlen' })).toHaveCount(0);
  });
});

const QUEUE_ITEM = {
  download_state: 'downloading',
  progress: 0.4,
  remaining_bytes: 600_000_000,
  remaining_seconds: 300,
  kind: 'movie',
  grab_id: 77,
  media_id: 1,
  media_title: 'Film 0001',
  release_title: 'Film.0001.1951.1080p.BluRay.x264-GRP',
  quality: 'Bluray-1080p',
  size: 1_000_000_000,
  status: 'downloading',
  message: null,
  episode_ids: [],
  grabbed_at: '2026-11-17T08:00:00Z',
  updated_at: '2026-11-17T08:05:00Z',
} as const;

async function expectRowLook(row: Locator, details: Locator, action: Locator): Promise<void> {
  const r = await box(row);
  const a = await box(action);
  expect(a.x + a.width, 'action at the end of the row').toBeGreaterThan(r.x + r.width - 32);
  expect(a.height).toBeGreaterThanOrEqual(32);
  const title = await row
    .getByRole('link')
    .first()
    .evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
  const look = await details.evaluate((el) => ({
    size: parseFloat(getComputedStyle(el).fontSize),
    colour: getComputedStyle(el).color,
  }));
  expect(look.size).toBeLessThan(title);
  expect(look.colour).toBe(token('dark', 'text-secondary'));
}

test.describe('list rows on the other pages (AC-06)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('queue rows: details small and secondary, the action at the end', async ({ page }) => {
    const real = await overviewAnswer(page);
    await page.route('**/api/overview', (route) =>
      route.fulfill({ json: { ...real, queue: { ...real.queue, items: [QUEUE_ITEM] } } }),
    );
    await page.goto(`${LIBRARY}/`);
    const row = page.getByTestId('queue-item');
    await expect(row).toBeVisible();
    await expectRowLook(
      row,
      row.locator('.release'),
      row.getByRole('button', { name: 'Als fehlgeschlagen markieren' }),
    );
  });

  test('blocklist and backup rows: details small and secondary, the actions at the end', async ({
    page,
  }) => {
    await page.route('**/api/blocklist?*', (route) =>
      route.fulfill({
        json: {
          page: 1,
          page_size: 50,
          total: 1,
          entries: [
            {
              id: 1,
              movie_id: 1,
              series_id: null,
              media_title: 'Film 0001',
              title: 'Film.0001.1951.1080p-GRP',
              indexer: 'Testindexer',
              reason: 'Download fehlgeschlagen',
              blocklisted_at: '2026-11-10T08:00:00Z',
            },
          ],
        },
      }),
    );
    await page.goto(`${LIBRARY}/blocklist`);
    const entry = page.getByTestId('blocklist-entry').first();
    await expect(entry).toBeVisible();
    await expectRowLook(entry, entry.locator('.small').first(), entry.getByRole('button').last());
    await page.goto(`${LIBRARY}/backups`);
    const backup = page.getByTestId('backup').first();
    await expect(backup).toBeVisible();
    const backupRow = await box(backup);
    const last = await box(backup.getByRole('button').last());
    expect(last.x + last.width).toBeGreaterThan(backupRow.x + backupRow.width - 32);
    expect(
      await backup
        .locator('.small')
        .first()
        .evaluate((el) => getComputedStyle(el).color),
    ).toBe(token('dark', 'text-secondary'));
  });

  test('the day groups of „Die nächsten 7 Tage“ take no hover ground', async ({ page }) => {
    await page.goto(`${LIBRARY}/`);
    const group = page.locator('li.day-group').first();
    await expect(group).toBeVisible();
    const rest = await group.evaluate((el) => getComputedStyle(el).backgroundColor);
    await group.hover();
    await page.waitForTimeout(200);
    expect(await group.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(rest);
  });
});

test.describe('Fehlend (AC-05)', () => {
  test('shared tabs with the accent underline; Filme/Episoden a segmented control', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/fehlend`);
    const tabs = page.getByRole('tablist', { name: 'Liste' });
    await expect(tabs).toHaveClass(/am-tabs/);
    const selected = tabs.getByRole('tab', { name: 'Fehlend' });
    await expect(selected).toHaveAttribute('aria-selected', 'true');
    expect(await selected.evaluate((el) => getComputedStyle(el).borderBottomColor)).toBe(
      token('dark', 'accent'),
    );
    const kind = page.getByRole('group', { name: 'Art' });
    await expect(kind).toHaveClass(/am-seg/);
    await kind.getByRole('button', { name: 'Episoden' }).click();
    await expect(page).toHaveURL(/art=episoden/);
    await expect(kind.getByRole('button', { name: 'Episoden' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });
});

test.describe('list rows (AC-06)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('actions at the end of the row, at least 32 px; rows take the hover ground', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/fehlend`);
    const row = page.getByTestId('wanted-item').first();
    await expect(row).toBeVisible();
    const rowBox = await box(row);
    const action = await box(row.getByRole('button', { name: /suchen$/ }));
    expect(action.x + action.width).toBeGreaterThan(rowBox.x + rowBox.width - 32);
    expect(action.width).toBeGreaterThanOrEqual(32);
    expect(action.height).toBeGreaterThanOrEqual(32);
    const rest = await row.evaluate((el) => getComputedStyle(el).backgroundColor);
    await row.hover();
    await expect
      .poll(() => row.evaluate((el) => getComputedStyle(el).backgroundColor))
      .not.toBe(rest);
    const title = row.getByRole('link').first();
    expect(await title.evaluate((el) => getComputedStyle(el).fontWeight)).toBe('600');
  });
});

test.describe('system and login (AC-07, AC-08)', () => {
  test.use({ viewport: { width: 1440, height: 900 } });

  test('the system overview is a compact facts panel in up to three columns', async ({ page }) => {
    await page.goto(`${SERVERS.open}/system`);
    const facts = page.getByRole('region', { name: 'Überblick' }).locator('dl');
    await expect(facts).toBeVisible();
    const terms = await facts.locator(':scope > div').evaluateAll((all) =>
      all.map((el) => ({
        y: Math.round(el.getBoundingClientRect().y),
        term: parseFloat(getComputedStyle(el.querySelector('dt') ?? el).fontSize),
        value: parseFloat(getComputedStyle(el.querySelector('dd') ?? el).fontSize),
      })),
    );
    expect(new Set(terms.map((t) => t.y)).size).toBe(1);
    for (const t of terms) expect(t.term).toBeLessThan(t.value);
  });

  test('the login form is a centred card at most 400 px wide with the icon above', async ({
    page,
  }) => {
    await page.goto(`${SERVERS.auth}/anmelden`);
    const card = page.locator('section.login');
    const heading = page.getByRole('heading', { name: 'Bei sofarr anmelden' });
    await expect(heading).toBeVisible();
    const c = await box(card);
    const main = await box(page.locator('main'));
    expect(c.width).toBeLessThanOrEqual(400);
    expect(Math.abs(c.x - main.x - (main.x + main.width - (c.x + c.width)))).toBeLessThan(4);
    const icon = await box(card.locator('.login-icon'));
    expect(icon.y + icon.height).toBeLessThanOrEqual((await box(heading)).y + 1);
  });
});

async function expectNoViolations(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  expect(
    result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`),
  ).toEqual([]);
}

const PAGES = [
  ['Übersicht', `${LIBRARY}/`],
  ['Fehlend', `${LIBRARY}/fehlend`],
  ['Blocklist', `${LIBRARY}/blocklist`],
  ['Backups', `${LIBRARY}/backups`],
  ['Untertitel', `${EXTRAS}/untertitel`],
  ['System', `${SERVERS.open}/system`],
  ['Anmelden', `${SERVERS.auth}/anmelden`],
] as const;

for (const palette of PALETTES) {
  test.describe(`axe, ${palette} (AC-09)`, () => {
    for (const [name, url] of PAGES) {
      test(`${name} at 1440 and 360 px`, async ({ page }) => {
        await storeTheme(page, palette);
        await page.setViewportSize({ width: 1440, height: 900 });
        await page.goto(url);
        await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
        await expect(page.getByText(/wird geladen|werden geladen/)).toHaveCount(0);
        await expectNoViolations(page);
        await page.setViewportSize({ width: 360, height: 740 });
        await expectNoViolations(page);
        expect(
          await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
        ).toBe(true);
      });
    }
  });
}
