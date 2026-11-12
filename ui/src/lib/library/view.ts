// List or poster grid, remembered per list in the browser (spec web-ui-library AC-02).

export type ListView = 'liste' | 'poster';

export const viewKey = (list: 'filme' | 'serien'): string => `sofarr-ansicht-${list}`;

export type ViewStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** The remembered view; the list when nothing (or nothing readable) is stored. */
export function readView(storage: () => ViewStorage, key: string): ListView {
  try {
    return storage().getItem(key) === 'poster' ? 'poster' : 'liste';
  } catch {
    return 'liste';
  }
}

/** Remembers the view; a browser that blocks storage keeps it for this page only. */
export function storeView(storage: () => ViewStorage, key: string, view: ListView): void {
  try {
    storage().setItem(key, view);
  } catch {
    // Not remembered: the page still shows the chosen view.
  }
}
