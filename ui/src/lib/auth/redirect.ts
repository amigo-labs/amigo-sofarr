// Where to go after a login, and the address of the login page (spec web-ui-shell AC-04).

import type { Me } from '../api/generated/Me';

export const LOGIN_PATH = '/anmelden';
/** The query parameter of the login page that holds the requested page. */
export const NEXT_PARAM = 'weiter';
export const START_PATH = '/';

const BASE = 'http://sofarr.invalid';

/** Control characters and backslashes are never part of a page address of the UI. */
function isForbidden(char: string): boolean {
  const code = char.charCodeAt(0);
  return char === '\\' || code < 0x20 || code === 0x7f;
}

/**
 * The page to show after a login: `raw` when it is a relative path on this origin (and not
 * the login page itself), otherwise the start page. Guards against open redirects such as
 * `//evil.example` or `/\evil.example`.
 */
export function safeTarget(raw: string | null | undefined): string {
  if (raw === null || raw === undefined) return START_PATH;
  if (!raw.startsWith('/') || raw.startsWith('//') || [...raw].some(isForbidden)) {
    return START_PATH;
  }
  let url: URL;
  try {
    url = new URL(raw, BASE);
  } catch {
    return START_PATH;
  }
  if (url.origin !== BASE || url.pathname === LOGIN_PATH || url.pathname.startsWith('/api/')) {
    return START_PATH;
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

/** The address of the login page that returns to `target` afterwards. */
export function loginAddress(target: string): string {
  const safe = safeTarget(target);
  if (safe === START_PATH) return LOGIN_PATH;
  return `${LOGIN_PATH}?${NEXT_PARAM}=${encodeURIComponent(safe)}`;
}

/** Whether the browser must log in before it sees a page. */
export function loginRequired(me: Me): boolean {
  return me.auth_enabled && !me.authenticated;
}

/** Whether the login page and the logout action exist for this browser (AC-06). */
export function loginOffered(me: Me): boolean {
  return me.auth_enabled && me.method !== 'trusted_network';
}

/** Whether the browser holds a session it can end (logout is shown). */
export function canLogOut(me: Me): boolean {
  return me.auth_enabled && me.authenticated && me.method === 'session';
}

/**
 * The address to go to instead of `pathname`, or `null` to stay: the login page while a
 * login is needed, away from the login page while none is (AC-04, AC-06).
 */
export function routeFor(me: Me, pathname: string, search: string, hash: string): string | null {
  const onLogin = pathname === LOGIN_PATH;
  if (loginRequired(me)) {
    return onLogin ? null : loginAddress(`${pathname}${search}${hash}`);
  }
  if (!onLogin) return null;
  if (!loginOffered(me)) return START_PATH;
  return safeTarget(new URLSearchParams(search).get(NEXT_PARAM));
}
