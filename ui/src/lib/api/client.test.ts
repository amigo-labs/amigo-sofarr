import { describe, expect, it, vi } from 'vitest';
import { createApiClient } from './client';
import { UNREACHABLE, send, type FetchFn } from './http';

type Answer = { status: number; body?: string; headers?: Record<string, string> };

/** A fetch that answers from a list and records every request. */
function fakeFetch(...answers: Answer[]) {
  const calls: { path: string; init: RequestInit }[] = [];
  const fn: FetchFn = (path, init) => {
    calls.push({ path, init });
    const answer = answers.shift();
    if (answer === undefined) return Promise.reject(new TypeError('Failed to fetch'));
    return Promise.resolve(
      new Response(answer.body ?? null, { status: answer.status, headers: answer.headers ?? {} }),
    );
  };
  return { fn, calls };
}

const json = (value: unknown) => JSON.stringify(value);
const NOW = () => Date.parse('2026-11-12T12:00:00Z');

describe('send', () => {
  it('returns the data of a success', async () => {
    const { fn, calls } = fakeFetch({ status: 200, body: json({ ok: true }) });
    const result = await send<{ ok: boolean }>(fn, 'POST', '/api/auth/login', {
      username: 'a',
      password: 'b',
    });
    expect(result).toEqual({ ok: true, data: { ok: true } });
    expect(calls[0]?.init.body).toBe(json({ username: 'a', password: 'b' }));
    expect(calls[0]?.init.credentials).toBe('same-origin');
  });

  it('turns a 5xx with an ErrorBody into a server failure with its German message', async () => {
    const { fn } = fakeFetch({ status: 503, body: json({ error: 'Datenbank gesperrt' }) });
    const result = await send(fn, 'GET', '/api/system');
    expect(result).toEqual({
      ok: false,
      failure: { kind: 'server', status: 503, message: 'Datenbank gesperrt' },
    });
  });

  it('gives a German message when a 5xx has no ErrorBody', async () => {
    const { fn } = fakeFetch({ status: 502, body: '<html>Bad Gateway</html>' });
    const result = await send(fn, 'GET', '/api/system');
    expect(result).toEqual({
      ok: false,
      failure: { kind: 'server', status: 502, message: 'Serverfehler (Status 502).' },
    });
  });

  it('reports an unreachable server in German', async () => {
    const { fn } = fakeFetch();
    const result = await send(fn, 'GET', '/api/system');
    expect(result).toEqual({
      ok: false,
      failure: { kind: 'network', status: null, message: UNREACHABLE },
    });
  });

  it('reads the wait of a throttled answer from Retry-After', async () => {
    const { fn } = fakeFetch(
      { status: 429, body: json({ error: 'Zu viele' }), headers: { 'Retry-After': '17' } },
      { status: 429, body: json({ error: 'Zu viele' }) },
    );
    const first = await send(fn, 'POST', '/api/auth/login', {}, NOW);
    const second = await send(fn, 'POST', '/api/auth/login', {}, NOW);
    expect(first).toEqual({
      ok: false,
      failure: { kind: 'throttled', status: 429, message: 'Zu viele', retryAfterSeconds: 17 },
    });
    expect(
      !second.ok && second.failure.kind === 'throttled' && second.failure.retryAfterSeconds,
    ).toBe(60);
  });

  it('keeps the German message of a 401', async () => {
    const { fn } = fakeFetch({
      status: 401,
      body: json({ error: 'Benutzername oder Passwort falsch' }),
    });
    const result = await send(fn, 'POST', '/api/auth/login', {});
    expect(result).toEqual({
      ok: false,
      failure: { kind: 'unauthorized', status: 401, message: 'Benutzername oder Passwort falsch' },
    });
  });
});

describe('createApiClient', () => {
  it('reports a 401 of a read as an expired session without an action', async () => {
    const { fn } = fakeFetch({ status: 401, body: json({ error: 'Anmeldung erforderlich' }) });
    const onUnauthorized = vi.fn<(action: string | null) => void>();
    const result = await createApiClient(fn, onUnauthorized).read('/api/system');
    expect(result.ok).toBe(false);
    expect(onUnauthorized).toHaveBeenCalledExactlyOnceWith(null);
  });

  it('sends a write action once and names it after a 401, without repeating it', async () => {
    const { fn, calls } = fakeFetch(
      { status: 401, body: json({ error: 'Anmeldung erforderlich' }) },
      { status: 200, body: json({ ok: true }) },
    );
    const onUnauthorized = vi.fn<(action: string | null) => void>();
    const result = await createApiClient(fn, onUnauthorized).write(
      'Sicherung starten',
      'POST',
      '/api/backups',
    );
    expect(result.ok).toBe(false);
    expect(calls).toHaveLength(1);
    expect(onUnauthorized).toHaveBeenCalledExactlyOnceWith('Sicherung starten');
  });

  it('does not repeat a write action after a server error either', async () => {
    const { fn, calls } = fakeFetch({ status: 500, body: json({ error: 'kaputt' }) });
    const onUnauthorized = vi.fn<(action: string | null) => void>();
    const result = await createApiClient(fn, onUnauthorized).write(
      'Abmelden',
      'POST',
      '/api/auth/logout',
    );
    expect(result).toEqual({
      ok: false,
      failure: { kind: 'server', status: 500, message: 'kaputt' },
    });
    expect(calls).toHaveLength(1);
    expect(onUnauthorized).not.toHaveBeenCalled();
  });

  it('leaves the 401 of an open request (the login) to the caller', async () => {
    const { fn } = fakeFetch({
      status: 401,
      body: json({ error: 'Benutzername oder Passwort falsch' }),
    });
    const onUnauthorized = vi.fn<(action: string | null) => void>();
    await createApiClient(fn, onUnauthorized).open('POST', '/api/auth/login', {});
    expect(onUnauthorized).not.toHaveBeenCalled();
  });
});
