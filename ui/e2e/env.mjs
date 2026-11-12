// The login password of the e2e servers, generated per run: the Playwright config sets it
// once in the main process; the servers it starts and its workers inherit it.

import { randomUUID } from 'node:crypto';

const KEY = 'SOFARR_E2E_PASSWORD';

/** The password of this run; created on first use. */
export function e2ePassword() {
  if (!process.env[KEY]) process.env[KEY] = `e2e-${randomUUID()}`;
  return process.env[KEY];
}
