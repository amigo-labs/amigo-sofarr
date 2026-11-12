import { describe, expect, it } from 'vitest';
import type { ApiResult } from '../api/http';
import type { Page } from '../api/generated/Page';
import { createPaged } from './paged.svelte';

const pageOf = (page: number, total: number, items: number[]): ApiResult<Page<number>> => ({
  ok: true,
  data: { page, page_size: 2, total, items },
});

describe('paged list (AC-02)', () => {
  it('loads further pages until every match is there', async () => {
    const list = createPaged<number>();
    const pages = new Map([
      [1, [1, 2]],
      [2, [3, 4]],
      [3, [5]],
    ]);
    await list.reset((p) => Promise.resolve(pageOf(p, 5, pages.get(p) ?? [])));
    expect(list.items).toEqual([1, 2]);
    expect(list.done).toBe(false);
    await list.more();
    await list.more();
    expect(list.items).toEqual([1, 2, 3, 4, 5]);
    expect(list.done).toBe(true);
    await list.more();
    expect(list.items).toHaveLength(5);
  });

  it('drops the answer of an older query', async () => {
    const list = createPaged<number>();
    const pending: ((value: ApiResult<Page<number>>) => void)[] = [];
    const slow = list.reset(
      () => new Promise<ApiResult<Page<number>>>((resolve) => pending.push(resolve)),
    );
    await list.reset(() => Promise.resolve(pageOf(1, 1, [9])));
    pending[0]?.(pageOf(1, 2, [1, 2]));
    await slow;
    expect(list.items).toEqual([9]);
    expect(list.total).toBe(1);
  });

  it('keeps a failure until the next query and updates single items', async () => {
    const list = createPaged<number>();
    await list.reset(() =>
      Promise.resolve({ ok: false, failure: { kind: 'server', status: 500, message: 'kaputt' } }),
    );
    expect(list.failure?.message).toBe('kaputt');
    await list.reset(() => Promise.resolve(pageOf(1, 2, [1, 2])));
    expect(list.failure).toBeNull();
    list.update(
      (i) => i === 2,
      () => 20,
    );
    expect(list.items).toEqual([1, 20]);
  });
});
