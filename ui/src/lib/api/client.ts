// The API client of the UI: reads and write actions, with the shared handling of an
// expired session (spec web-ui-shell AC-07).

import { send, type ApiResult, type FetchFn, type HttpMethod } from './http';

/** Called when a request needing a credential got 401; `action` names a write action. */
export type UnauthorizedHandler = (action: string | null) => void;

export type ApiClient = {
  /** Reads data; a 401 reports the expired session. Reads may be repeated freely. */
  readonly read: <T>(path: string) => Promise<ApiResult<T>>;
  /**
   * A write action, sent exactly once: it is never repeated on its own, also not after a
   * 401 (the handler learns its German `label` so the UI can say it was not carried out).
   */
  readonly write: <T>(
    label: string,
    method: HttpMethod,
    path: string,
    body?: unknown,
  ) => Promise<ApiResult<T>>;
  /** A request whose 401 is part of its answer (the login itself). */
  readonly open: <T>(method: HttpMethod, path: string, body?: unknown) => Promise<ApiResult<T>>;
};

export function createApiClient(fetchFn: FetchFn, onUnauthorized: UnauthorizedHandler): ApiClient {
  const guarded = async <T>(
    action: string | null,
    method: HttpMethod,
    path: string,
    body?: unknown,
  ) => {
    const result = await send<T>(fetchFn, method, path, body);
    if (!result.ok && result.failure.kind === 'unauthorized') onUnauthorized(action);
    return result;
  };
  return {
    read: (path) => guarded(null, 'GET', path),
    write: (label, method, path, body) => guarded(label, method, path, body),
    open: (method, path, body) => send(fetchFn, method, path, body),
  };
}
