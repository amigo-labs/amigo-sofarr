// The sidebar's collapsed state (spec ui-shell-redesign AC-03 .. AC-05): expanded until the
// user collapses it, remembered in the browser when it lets us. `app.html` applies the stored
// state before the first paint with the same key and attribute.

export const SIDEBAR_KEY = 'sofarr-sidebar';
const COLLAPSED = 'collapsed';

/** The part of `Storage` the sidebar needs; tests pass a fake. */
export type SidebarStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

/** Whether the stored state is collapsed; anything else, or blocked storage, is expanded. */
export function readCollapsed(storage: () => SidebarStorage): boolean {
  try {
    return storage().getItem(SIDEBAR_KEY) === COLLAPSED;
  } catch {
    return false;
  }
}

/** Remembers the state; returns whether the browser kept it. */
export function storeCollapsed(storage: () => SidebarStorage, collapsed: boolean): boolean {
  try {
    if (collapsed) storage().setItem(SIDEBAR_KEY, COLLAPSED);
    else storage().removeItem(SIDEBAR_KEY);
    return true;
  } catch {
    return false;
  }
}

/** Marks the document element, which the layout's styles read (`data-sidebar`). */
export function applyCollapsed(
  root: Pick<HTMLElement, 'setAttribute' | 'removeAttribute'>,
  collapsed: boolean,
): void {
  if (collapsed) root.setAttribute('data-sidebar', COLLAPSED);
  else root.removeAttribute('data-sidebar');
}
