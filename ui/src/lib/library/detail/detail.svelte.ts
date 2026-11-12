// What the movie and series pages share: the id from the page address, loading with the
// shared refresh, switching monitoring and searching with German results (AC-09, AC-10).

import type { ApiResult } from '../../api/http';
import type { MonitorItem } from '../../api/generated/MonitorItem';
import type { SearchBody } from '../../api/generated/SearchBody';
import { monitor } from '../../api/library';
import { runSearch } from '../actions';
import { notify } from '../notices.svelte';

/** The id of a page address segment; `null` for anything but a positive number. */
export function idOf(segment: string | undefined): number | null {
  if (segment === undefined || !/^\d+$/.test(segment)) return null;
  const id = Number(segment);
  return Number.isSafeInteger(id) && id > 0 ? id : null;
}

export type ItemActions = {
  readonly busy: ReadonlySet<string>;
  /** Switches monitoring of items at once; returns whether it was saved. */
  readonly setMonitored: (
    key: string,
    on: boolean,
    items: readonly MonitorItem[],
  ) => Promise<boolean>;
  readonly search: (key: string, label: string, body: SearchBody) => Promise<void>;
};

export function createItemActions(onchanged: () => void): ItemActions {
  const state = $state<{ busy: Set<string> }>({ busy: new Set() });
  const run = async <T>(key: string, action: () => Promise<T>): Promise<T> => {
    state.busy = new Set([...state.busy, key]);
    try {
      return await action();
    } finally {
      state.busy = new Set([...state.busy].filter((k) => k !== key));
    }
  };
  return {
    get busy() {
      return state.busy;
    },
    setMonitored: (key, on, items) =>
      run(key, async () => {
        const result: ApiResult<unknown> = await monitor(on, items);
        if (result.ok) {
          onchanged();
          return true;
        }
        if (result.failure.kind !== 'unauthorized') {
          notify('danger', `Überwachung nicht geändert: ${result.failure.message}`);
        }
        return false;
      }),
    search: (key, label, body) => run(key, () => runSearch(label, body)),
  };
}
