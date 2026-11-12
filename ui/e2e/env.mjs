// The login password of the e2e servers, generated per run: the Playwright config sets it
// once in the main process; the servers it starts and its workers inherit it.

import { randomUUID } from 'node:crypto';

const KEY = 'SOFARR_E2E_PASSWORD';

/**
 * The first port of this run's servers: `SOFARR_E2E_PORT_BASE`, else 47810. Two checkouts
 * running their end-to-end tests at the same time need different bases.
 */
export function portBase() {
  const raw = process.env.SOFARR_E2E_PORT_BASE;
  const base = raw === undefined || raw === '' ? 47810 : Number(raw);
  if (!Number.isInteger(base) || base < 1024 || base > 65000) {
    throw new Error(`SOFARR_E2E_PORT_BASE must be a port between 1024 and 65000, not ${raw}`);
  }
  return base;
}

/** The password of this run; created on first use. */
export function e2ePassword() {
  if (!process.env[KEY]) process.env[KEY] = `e2e-${randomUUID()}`;
  return process.env[KEY];
}
