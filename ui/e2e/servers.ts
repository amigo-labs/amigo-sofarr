// The sofarr instances the end-to-end tests run against (started by `serve.mjs`). Each
// variant has a fixed port; the login data are test-only values.

export const USERNAME = 'admin';
export const PASSWORD = 'e2e-geheim';

export const SERVERS = {
  /** Auth off; with a download client and an indexer to show on the system page. */
  open: 'http://127.0.0.1:47811',
  /** Auth on: every browser has to log in. */
  auth: 'http://127.0.0.1:47812',
  /** Auth on, but 127.0.0.1 is a trusted network: no login page. */
  trusted: 'http://127.0.0.1:47813',
  /** Auth on, for the failed-login and throttling test only (it locks out 127.0.0.1). */
  throttle: 'http://127.0.0.1:47814',
} as const;

export type Variant = keyof typeof SERVERS;

/** The secret values in the open server's configuration; the UI must never show them. */
export const SECRETS = ['nzbget-passwort-e2e', 'indexer-schluessel-e2e', 'apikey=abc123'] as const;
