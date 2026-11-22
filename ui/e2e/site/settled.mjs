// The wait of the website screenshots for IMDb ratings (spec ui-polish AC-14): a pending
// rating shows no text on a tile, so a picture taken too early would look finished.

/** The ratings on the page that are still pending. */
const pendingRatings = (page) => page.locator('[data-testid="rating"].pending');

/**
 * Waits until no rating on the page is pending, reloading in between as the ratings fill in
 * in the background; throws if one is still pending after `tries` looks.
 */
export async function ratingsSettled(page, { tries = 60, pauseMs = 1000 } = {}) {
  for (let i = 0; i < tries; i += 1) {
    await page.waitForLoadState('networkidle');
    if ((await pendingRatings(page).count()) === 0) return;
    if (i + 1 < tries) {
      await new Promise((r) => setTimeout(r, pauseMs));
      await page.reload();
    }
  }
  throw new Error(`a rating on ${page.url()} is still pending: no picture taken`);
}
