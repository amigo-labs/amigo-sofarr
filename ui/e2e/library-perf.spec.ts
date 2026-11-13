// spec web-ui-library AC-04: the time measurement with 2,501 movies. It runs in the project
// `perf` after every other test, alone, against the `bench` server nobody changes.

import { expect, test } from '@playwright/test';
import { apiTotal, items } from './library-helpers';
import { SERVERS } from './servers';

test.describe('large library (AC-04)', () => {
  test('first entries within 1 s, filter and search updates within 200 ms', async ({
    page,
    request,
  }) => {
    // The page itself notes when the first entry is in the document and the first frame
    // after that, in milliseconds since it began to open. Headless Chromium draws frames only
    // on demand, so a frame loop asks for them as a visible browser draws them (60 per s).
    await page.addInitScript(() => {
      const marks: Record<string, number> = {};
      Object.assign(window, { sofarrMarks: marks });
      const frame = (now: number): void => {
        if (marks['inserted'] !== undefined && now >= marks['inserted']) {
          marks['shown'] = now;
          return;
        }
        requestAnimationFrame(frame);
      };
      requestAnimationFrame(frame);
      const observer = new MutationObserver(() => {
        if (document.querySelector('[data-testid="library-items"] li') === null) return;
        observer.disconnect();
        marks['inserted'] = performance.now();
      });
      observer.observe(document, { childList: true, subtree: true });
    });
    // A warm browser, as a user's is: a fresh headless Chromium holds its very first frame
    // for about a second, whatever the page; the warm-up page waits until it is drawn.
    await page.goto(`${SERVERS.bench}/system`);
    await expect(page.getByRole('heading', { level: 1, name: 'System' })).toBeVisible();
    await page.evaluate(() => new Promise<number>((resolve) => requestAnimationFrame(resolve)));
    await page.goto(`${SERVERS.bench}/filme`);
    await expect(items(page).first()).toBeVisible();
    const marks = await page.waitForFunction(() => {
      const m = (window as unknown as { sofarrMarks?: Record<string, number> }).sofarrMarks;
      return m?.['shown'] === undefined ? null : m;
    });
    const measured = (await marks.jsonValue()) ?? {};
    const firstEntries = Math.round(Number(measured['shown']));
    const inserted = Math.round(Number(measured['inserted']));

    const all = await apiTotal(request, '/api/library/movies', SERVERS.bench);
    const monitored = await apiTotal(request, '/api/library/movies?monitored=yes', SERVERS.bench);
    await expect(page.getByTestId('total')).toHaveText(`${all} Filme`);

    /** Milliseconds from a change to the frame the list shows the new total. */
    const measure = async (change: () => Promise<void>, total: string): Promise<number> => {
      const started = await page.evaluate(() => performance.now());
      await change();
      const shown = await page.waitForFunction(
        (text) =>
          document.querySelector('[data-testid="total"]')?.textContent === text
            ? performance.now()
            : null,
        total,
        { polling: 'raf' },
      );
      return Number(await shown.jsonValue()) - started;
    };
    const search = await measure(() => page.getByLabel('Suche').fill('Film 2490'), '1 Film');
    await expect(items(page)).toHaveText([/Film 2490/]);
    await page.getByLabel('Suche').fill('');
    await expect(page.getByTestId('total')).toHaveText(`${all} Filme`);
    const filter = await measure(async () => {
      await page.getByLabel('Überwacht', { exact: true }).selectOption({ label: 'Ja' });
    }, `${monitored} Filme`);
    test.info().annotations.push({
      type: 'Messung',
      description: `erste Einträge ${firstEntries} ms (im Dokument ${inserted} ms), Suche ${Math.round(search)} ms, Filter ${Math.round(filter)} ms`,
    });
    expect(firstEntries).toBeLessThan(1000);
    expect(search).toBeLessThan(200);
    expect(filter).toBeLessThan(200);
  });
});
