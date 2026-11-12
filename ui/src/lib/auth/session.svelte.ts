// The browser's login state, shared by the layout, the login page and every request.

import { createApiClient } from '../api/client';
import type { ApiFailure, ApiResult } from '../api/http';
import type { LoginBody } from '../api/generated/LoginBody';
import type { Me } from '../api/generated/Me';
import type { OkBody } from '../api/generated/OkBody';

type SessionState = {
  /** `null` until the first answer of `/api/auth/me`. */
  me: Me | null;
  /** Why `/api/auth/me` could not be read (no answer yet). */
  failure: ApiFailure | null;
  loading: boolean;
  /** The session ended while the user worked (a request got 401). */
  expired: boolean;
  /** The German label of a write action that got 401 and was not carried out. */
  interrupted: string | null;
};

export const session: SessionState = $state({
  me: null,
  failure: null,
  loading: false,
  expired: false,
  interrupted: null,
});

function unauthorized(action: string | null): void {
  if (session.me !== null && session.me.auth_enabled) {
    session.me = { ...session.me, authenticated: false, method: null };
  }
  session.expired = true;
  if (action !== null) session.interrupted = action;
}

export const api = createApiClient((input, init) => fetch(input, init), unauthorized);

/** Reads the login state; keeps the last known one when the API cannot answer. */
export async function loadSession(): Promise<void> {
  session.loading = true;
  const result = await api.open<Me>('GET', '/api/auth/me');
  session.loading = false;
  if (result.ok) {
    session.me = result.data;
    session.failure = null;
  } else {
    session.failure = result.failure;
  }
}

export async function logIn(body: LoginBody): Promise<ApiResult<OkBody>> {
  const result = await api.open<OkBody>('POST', '/api/auth/login', body);
  if (result.ok) {
    session.expired = false;
    await loadSession();
  }
  return result;
}

export const LOGOUT_LABEL = 'Abmelden';

export async function logOut(): Promise<ApiResult<OkBody>> {
  const result = await api.write<OkBody>(LOGOUT_LABEL, 'POST', '/api/auth/logout');
  if (result.ok) {
    session.interrupted = null;
    session.expired = false;
    await loadSession();
  }
  return result;
}

/** Forgets the notice about an interrupted write action. */
export function dismissInterrupted(): void {
  session.interrupted = null;
}
