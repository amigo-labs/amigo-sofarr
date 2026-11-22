// Spec ui-overview-polish AC-01 .. AC-07: key figures with icons, a muted line without a
// download client, the queue's „Stand“ once, „Heute“ and „Morgen“ day headings, empty
// states with an icon.

import { AxeBuilder } from '@axe-core/playwright';
import { expect, test, type Locator, type Page } from '@playwright/test';
import type { OverviewAnswer } from '../src/lib/api/generated/OverviewAnswer';
import type { QueueItem } from '../src/lib/api/generated/QueueItem';
import { LIBRARY } from './library-helpers';
import { octiconPath as octicon } from './octicons.mjs';
import { PALETTES, storeTheme } from './theme-helpers';

const NO_CLIENT = 'Kein Download-Client eingerichtet – sofarr lädt nichts herunter.';

const iconOf = (scope: Locator): Promise<string | null> =>
  scope.locator('svg path').first().getAttribute('d');

const QUEUE_ITEM: QueueItem = {
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
};

const real = async (page: Page): Promise<OverviewAnswer> =>
  (await (await page.request.get(`${LIBRARY}/api/overview`)).json()) as OverviewAnswer;

/** Serves the overview with `change` applied to the real answer. */
async function overviewWith(
  page: Page,
  change: (answer: OverviewAnswer) => OverviewAnswer,
): Promise<void> {
  const answer = change(await real(page));
  await page.route('**/api/overview', (route) => route.fulfill({ json: answer }));
}

const queueBox = (page: Page) => page.getByRole('region', { name: 'Warteschlange' });

/** The colour a CSS colour expression resolves to in the page. */
async function resolvedColour(page: Page, value: string): Promise<string> {
  return page.evaluate((v) => {
    const probe = document.createElement('span');
    probe.style.color = v;
    document.body.append(probe);
    const out = getComputedStyle(probe).color;
    probe.remove();
    return out;
  }, value);
}

/** The browser's local date in `days` days as `YYYY-MM-DD`. */
async function localDay(page: Page, days: number): Promise<string> {
  return page.evaluate((d) => {
    const at = new Date();
    at.setDate(at.getDate() + d);
    const month = String(at.getMonth() + 1).padStart(2, '0');
    return `${at.getFullYear()}-${month}-${String(at.getDate()).padStart(2, '0')}`;
  }, days);
}

test.describe('key figures (AC-01)', () => {
  test('each figure has a decorative icon before its label and tabular figures', async ({
    page,
  }) => {
    await page.goto(`${LIBRARY}/`);
    // A link while its box shows, plain text otherwise (ui-polish AC-08).
    const figures = page.getByRole('list', { name: 'Kennzahlen' }).locator('.figure');
    await expect(figures).toHaveCount(3);
    for (const figure of await figures.all()) {
      const icon = figure.locator('svg');
      await expect(icon).toHaveCount(1);
      expect(await icon.evaluate((e) => e.closest('[aria-hidden="true"]') !== null)).toBe(true);
      // The label is plain text, not a status chip (no dot, no ground).
      const label = icon.locator('xpath=ancestor::span[not(@aria-hidden)][1]');
      expect(await label.evaluate((e) => getComputedStyle(e, '::before').content)).toBe('none');
      expect(await label.evaluate((e) => getComputedStyle(e).backgroundColor)).toBe(
        'rgba(0, 0, 0, 0)',
      );
      const number = figure.locator('strong');
      expect(await number.evaluate((e) => getComputedStyle(e).fontVariantNumeric)).toContain(
        'tabular-nums',
      );
    }
    // Warteschlange: download, Probleme: alert, Nächste 7 Tage: calendar; the icon first.
    for (const [i, name] of ['download', 'alert', 'calendar'].entries()) {
      const figure = figures.nth(i);
      expect(await iconOf(figure)).toBe(octicon(name));
      const icon = await figure.locator('svg').boundingBox();
      const label = await figure.locator('strong + span').evaluate((e) => {
        const range = document.createRange();
        const words = [...e.childNodes].find((n) => n.nodeType === Node.TEXT_NODE);
        if (words !== undefined) range.selectNodeContents(words);
        return range.getBoundingClientRect().x;
      });
      expect((icon?.x ?? 99) + (icon?.width ?? 0)).toBeLessThanOrEqual(label);
    }
    // The links keep their names of today (ui-overview-pages AC-02).
    await expect(figures.nth(0)).toHaveAccessibleName(/^\d+ Warteschlange$/);
  });
});

test.describe('queue without and with a client (AC-02, AC-03)', () => {
  test('without a client: one muted line, no warning, no empty text', async ({ page }) => {
    await page.goto(`${LIBRARY}/`);
    const queue = queueBox(page);
    await expect(queue.getByText(NO_CLIENT)).toBeVisible();
    expect(await queue.getByText(NO_CLIENT).evaluate((e) => getComputedStyle(e).color)).toBe(
      await resolvedColour(page, 'var(--fgColor-muted)'),
    );
    await expect(queue.getByRole('status')).toHaveCount(0);
    await expect(queue.getByText('Nichts in der Warteschlange.')).toHaveCount(0);
    await expect(queue.getByText('Kein Download-Client konfiguriert')).toHaveCount(0);
  });

  test('without a client, active grabs still show as rows below the line', async ({ page }) => {
    await overviewWith(page, (a) => ({ ...a, queue: { ...a.queue, items: [QUEUE_ITEM] } }));
    await page.goto(`${LIBRARY}/`);
    await expect(queueBox(page).getByText(NO_CLIENT)).toBeVisible();
    await expect(queueBox(page).getByTestId('queue-item')).toHaveCount(1);
  });

  test('an unreachable client with a known state names its time once', async ({ page }) => {
    await overviewWith(page, (a) => ({
      ...a,
      queue: {
        items: [QUEUE_ITEM],
        notice: 'Download-Client nicht erreichbar – Stand von 17.11.2026 09:05',
        as_of: '2026-11-17T08:05:00Z',
        client_configured: true,
      },
    }));
    await page.goto(`${LIBRARY}/`);
    const warning = queueBox(page).getByRole('status');
    await expect(warning).toHaveText(
      'Download-Client nicht erreichbar – Stand von 17.11.2026 09:05',
    );
    expect(((await warning.textContent()) ?? '').match(/Stand/g)?.length).toBe(1);
  });

  test('an unreachable client without a state shows no time', async ({ page }) => {
    await overviewWith(page, (a) => ({
      ...a,
      queue: {
        items: [],
        notice: 'Download-Client nicht erreichbar',
        as_of: null,
        client_configured: true,
      },
    }));
    await page.goto(`${LIBRARY}/`);
    await expect(queueBox(page).getByRole('status')).toHaveText('Download-Client nicht erreichbar');
  });
});

test.describe('day headings (AC-05)', () => {
  test('„Heute“, „Morgen“, then weekday and date; the full date as tooltip', async ({ page }) => {
    await page.goto(`${LIBRARY}/`);
    const days = [await localDay(page, 0), await localDay(page, 1), await localDay(page, 3)];
    await overviewWith(page, (a) => {
      const sample = a.upcoming[0];
      if (sample === undefined) throw new Error('the library has no upcoming entry');
      return { ...a, upcoming: days.map((date) => ({ ...sample, date })) };
    });
    await page.reload();
    const region = page.getByRole('region', { name: 'Die nächsten 7 Tage' });
    const headings = region.getByRole('heading', { level: 3 });
    await expect(headings).toHaveCount(3);
    await expect(headings.nth(0)).toHaveText('Heute');
    await expect(headings.nth(1)).toHaveText('Morgen');
    await expect(headings.nth(2)).toHaveText(
      /^(Montag|Dienstag|Mittwoch|Donnerstag|Freitag|Samstag|Sonntag), \d{2}\.\d{2}\.$/,
    );
    const [y, m, d] = (days[2] ?? '').split('-');
    await expect(headings.nth(2)).toContainText(`, ${d}.${m}.`);
    await expect(headings.nth(0).locator('time')).toHaveAttribute('datetime', days[0] ?? '');
    const [ty, tm, td] = (days[0] ?? '').split('-');
    await expect(headings.nth(0).locator('[title]')).toHaveAttribute('title', `${td}.${tm}.${ty}`);
    expect(y).toBeTruthy();
  });
});

/** The sentence of `empty` stands muted after the decorative Octicon `name`. */
async function expectEmptyLine(page: Page, empty: Locator, text: string, name: string) {
  const muted = await resolvedColour(page, 'var(--fgColor-muted)');
  await expect(empty).toHaveText(text);
  expect(await iconOf(empty)).toBe(octicon(name));
  const icon = empty.locator('svg');
  await expect(icon).toHaveCount(1);
  expect(await icon.evaluate((e) => e.closest('[aria-hidden="true"]') !== null)).toBe(true);
  expect(await empty.evaluate((e) => getComputedStyle(e).color)).toBe(muted);
  // The icon comes before the sentence.
  const iconBox = await icon.boundingBox();
  const textStart = await empty.evaluate((e) => {
    const range = document.createRange();
    const sentence = [...e.childNodes].find((n) => n.nodeType === Node.TEXT_NODE);
    if (sentence === undefined) return 0;
    range.selectNodeContents(sentence);
    return range.getBoundingClientRect().x;
  });
  expect((iconBox?.x ?? 99) + (iconBox?.width ?? 0)).toBeLessThanOrEqual(textStart);
}

test.describe('empty states (AC-06, revised by ui-polish AC-08)', () => {
  test('empty boxes are hidden; one muted line after a decorative icon says so', async ({
    page,
  }) => {
    await overviewWith(page, (a) => ({
      ...a,
      queue: { items: [], notice: null, as_of: '2026-11-17T08:05:00Z', client_configured: true },
      upcoming: [],
      problems: [],
      activity: [],
    }));
    await page.goto(`${LIBRARY}/`);
    const empty = page.locator('.empty');
    await expect(empty).toHaveCount(1);
    await expectEmptyLine(
      page,
      empty,
      'Alles ruhig – keine Downloads, Probleme oder Termine.',
      'check',
    );
    await expect(page.getByRole('region')).toHaveCount(0);
  });

  test('an unreachable client without downloads keeps the queue’s empty sentence', async ({
    page,
  }) => {
    await overviewWith(page, (a) => ({
      ...a,
      queue: {
        items: [],
        notice: 'Download-Client nicht erreichbar',
        as_of: null,
        client_configured: true,
      },
    }));
    await page.goto(`${LIBRARY}/`);
    await expectEmptyLine(
      page,
      queueBox(page).locator('.empty'),
      'Nichts in der Warteschlange.',
      'download',
    );
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

for (const palette of PALETTES) {
  test.describe(`accessibility (AC-07), ${palette}`, () => {
    test.beforeEach(async ({ page }) => storeTheme(page, palette));
    for (const width of [1440, 360]) {
      test(`empty boxes at ${width} px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await overviewWith(page, (a) => ({ ...a, upcoming: [], problems: [], activity: [] }));
        await page.goto(`${LIBRARY}/`);
        await expect(page.getByText(NO_CLIENT)).toBeVisible();
        await expectNoViolations(page);
      });

      test(`a queue item, a problem and upcoming entries at ${width} px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await overviewWith(page, (a) => {
          const problem = { ...QUEUE_ITEM, status: 'failed', message: 'FAILURE/PAR' };
          return {
            ...a,
            queue: { ...a.queue, items: [QUEUE_ITEM] },
            problems: [problem],
            activity: [problem],
          };
        });
        await page.goto(`${LIBRARY}/`);
        await expect(queueBox(page).getByTestId('queue-item')).toBeVisible();
        await expectNoViolations(page);
      });
    }
  });
}
