// The HTTP layer of the API client: one request, its answer turned into a result.
// Pure apart from the injected `fetch`, so the tests drive it with fake answers.

import { parseRetryAfter } from '../auth/retry-after';

/** Why a request did not succeed. Every `message` is German and shown to the user. */
export type ApiFailure =
  | { readonly kind: 'unauthorized'; readonly status: 401; readonly message: string }
  | {
      readonly kind: 'throttled';
      readonly status: 429;
      readonly message: string;
      readonly retryAfterSeconds: number;
    }
  | { readonly kind: 'server'; readonly status: number; readonly message: string }
  | { readonly kind: 'client'; readonly status: number; readonly message: string }
  | { readonly kind: 'network'; readonly status: null; readonly message: string };

export type ApiResult<T> =
  | { readonly ok: true; readonly data: T }
  | { readonly ok: false; readonly failure: ApiFailure };

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE';

export type FetchFn = (input: string, init: RequestInit) => Promise<Response>;

export const UNREACHABLE = 'Der Server ist nicht erreichbar. Läuft sofarr noch?';

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** The German message of an `ErrorBody` answer, if the body is one. */
export function errorMessage(body: unknown): string | null {
  if (isRecord(body) && typeof body['error'] === 'string' && body['error'].trim() !== '') {
    return body['error'];
  }
  return null;
}

/** The message shown when an answer carries no `ErrorBody`. */
export function fallbackMessage(status: number): string {
  if (status === 401) return 'Anmeldung erforderlich.';
  if (status === 404) return 'Nicht gefunden (Status 404).';
  if (status === 429) return 'Zu viele Versuche.';
  if (status >= 500) return `Serverfehler (Status ${status}).`;
  return `Die Anfrage wurde abgelehnt (Status ${status}).`;
}

async function readJson(response: Response): Promise<unknown> {
  try {
    const text = await response.text();
    return text === '' ? null : (JSON.parse(text) as unknown);
  } catch {
    return null;
  }
}

/** Turns a failed answer into an [`ApiFailure`]. */
export function failureOf(
  status: number,
  body: unknown,
  retryAfter: string | null,
  now: number,
): ApiFailure {
  const message = errorMessage(body) ?? fallbackMessage(status);
  if (status === 401) return { kind: 'unauthorized', status, message };
  if (status === 429) {
    return {
      kind: 'throttled',
      status,
      message,
      retryAfterSeconds: parseRetryAfter(retryAfter, now),
    };
  }
  if (status >= 500) return { kind: 'server', status, message };
  return { kind: 'client', status, message };
}

/**
 * Sends one request to the sofarr API. The answer body of a success is returned as is:
 * its type is generated from the server's Rust types (`generated/`), which a server test
 * keeps in sync (spec web-ui-shell AC-15).
 */
export async function send<T>(
  fetchFn: FetchFn,
  method: HttpMethod,
  path: string,
  body?: unknown,
  now: () => number = Date.now,
): Promise<ApiResult<T>> {
  const init: RequestInit = {
    method,
    credentials: 'same-origin',
    headers:
      body === undefined
        ? { Accept: 'application/json' }
        : { Accept: 'application/json', 'Content-Type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  };
  let response: Response;
  try {
    response = await fetchFn(path, init);
  } catch {
    return { ok: false, failure: { kind: 'network', status: null, message: UNREACHABLE } };
  }
  const parsed = await readJson(response);
  if (response.ok) {
    return { ok: true, data: parsed as T };
  }
  return {
    ok: false,
    failure: failureOf(response.status, parsed, response.headers.get('Retry-After'), now()),
  };
}
