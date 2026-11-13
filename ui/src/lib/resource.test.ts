import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ApiResult } from './api/http';
import { LOADING_DELAY_MS, REFRESH_MS, createResource } from './resource.svelte';

const ok = (data: number): ApiResult<number> => ({ ok: true, data });
const down: ApiResult<number> = {
  ok: false,
  failure: { kind: 'network', status: null, message: 'Der Server ist nicht erreichbar.' },
};

/** A loader whose answers the test releases one by one. */
function controlled() {
  const pending: ((result: ApiResult<number>) => void)[] = [];
  const load = vi.fn<() => Promise<ApiResult<number>>>(
    () => new Promise<ApiResult<number>>((resolve) => pending.push(resolve)),
  );
  const answer = async (result: ApiResult<number>) => {
    pending.shift()?.(result);
    await vi.advanceTimersByTimeAsync(0);
  };
  return { load, answer };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('createResource', () => {
  it('refreshes every 15 seconds while started', async () => {
    const { load, answer } = controlled();
    const resource = createResource(load);
    const stop = resource.start();
    await answer(ok(1));
    expect(resource.data).toBe(1);
    expect(REFRESH_MS).toBe(15_000);
    await vi.advanceTimersByTimeAsync(REFRESH_MS);
    expect(load).toHaveBeenCalledTimes(2);
    await answer(ok(2));
    expect(resource.data).toBe(2);
    stop();
    await vi.advanceTimersByTimeAsync(REFRESH_MS * 3);
    expect(load).toHaveBeenCalledTimes(2);
  });

  it('keeps the last data after a failed refresh and retries within 30 seconds', async () => {
    const { load, answer } = controlled();
    const resource = createResource(load);
    resource.start();
    await answer(ok(1));
    await vi.advanceTimersByTimeAsync(REFRESH_MS);
    await answer(down);
    expect(resource.data).toBe(1);
    expect(resource.failure?.message).toBe('Der Server ist nicht erreichbar.');
    await vi.advanceTimersByTimeAsync(30_000);
    expect(load).toHaveBeenCalledTimes(3);
    await answer(ok(3));
    expect(resource.data).toBe(3);
    expect(resource.failure).toBeNull();
  });

  it('shows the loading indicator only after 300 ms', async () => {
    const { load, answer } = controlled();
    const resource = createResource(load);
    resource.start();
    await vi.advanceTimersByTimeAsync(LOADING_DELAY_MS - 1);
    expect(resource.slow).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(resource.slow).toBe(true);
    await answer(ok(1));
    expect(resource.slow).toBe(false);
  });

  it('leaves a 401 to the login page instead of showing an error', async () => {
    const { load, answer } = controlled();
    const resource = createResource(load);
    resource.start();
    await answer(ok(1));
    await vi.advanceTimersByTimeAsync(REFRESH_MS);
    await answer({
      ok: false,
      failure: { kind: 'unauthorized', status: 401, message: 'Anmeldung erforderlich' },
    });
    expect(resource.failure).toBeNull();
    expect(resource.data).toBe(1);
  });

  it('retries at once on demand', async () => {
    const { load, answer } = controlled();
    const resource = createResource(load);
    resource.start();
    await answer(down);
    expect(resource.data).toBeNull();
    expect(resource.failure).not.toBeNull();
    void resource.reload();
    await answer(ok(5));
    expect(resource.data).toBe(5);
  });
});
