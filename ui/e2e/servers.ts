// The sofarr instances the end-to-end tests run against (started by `serve.mjs`). Each
// variant has a fixed offset from the run's port base; the password is generated per run
// (both in `env.mjs`).

import { portBase } from './env.mjs';

const at = (offset: number): string => `http://127.0.0.1:${portBase() + offset}`;

export const USERNAME = 'admin';

export const SERVERS = {
  /** Auth off; with a download client and an indexer to show on the system page. */
  open: at(1),
  /** Auth on: every browser has to log in. */
  auth: at(2),
  /** Auth on, but 127.0.0.1 is a trusted network: no login page. */
  trusted: at(3),
  /** Auth on, for the failed-login and throttling test only (it locks out 127.0.0.1). */
  throttle: at(4),
  /** Auth off, shadow mode, 2,500 movies and 23 series (`seed.mjs`). */
  library: at(5),
  /** The same library as `library`, only read: the measurement of AC-04 runs alone on it. */
  bench: at(7),
  /** Auth off, live mode, movies with real files for the file actions. */
  live: at(6),
};

export type Variant = keyof typeof SERVERS;

/** The secret values in the open server's configuration; the UI must never show them. */
export const SECRETS = ['nzbget-passwort-e2e', 'indexer-schluessel-e2e', 'apikey=abc123'] as const;
