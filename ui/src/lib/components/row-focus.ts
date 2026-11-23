// Keeps the keyboard focus in rows of tiles that refresh on their own (spec ui-cinema-home
// AC-05). A tile that stays keeps the focus, even when the refresh moves it within its row;
// when the focused tile leaves its row the focus moves to the row's „Alle anzeigen“ link
// (`data-row-more`), and when the whole row goes, to `fallback` (the page's heading).
// Rows are elements with `data-row` inside `container`.

/** Watches `container`; returns the function that stops watching. */
export function keepRowFocus(container: HTMLElement, fallback: () => HTMLElement | null) {
  let focused: HTMLElement | null = null;
  let rowId: string | null = null;

  const onFocusIn = (event: FocusEvent): void => {
    const target = event.target instanceof HTMLElement ? event.target : null;
    const row = target?.closest<HTMLElement>('[data-row]') ?? null;
    focused = row === null ? null : target;
    rowId = row?.dataset['row'] ?? null;
  };
  // Focus that moves outside the rows or a click elsewhere ends the watch over that tile.
  const onOutside = (event: Event): void => {
    if (event.target instanceof Node && focused?.contains(event.target)) return;
    if (event.target instanceof Node && container.contains(event.target)) return;
    focused = null;
    rowId = null;
  };

  const restore = (): void => {
    if (focused === null) return;
    const active = document.activeElement;
    // The focus is where it was, or somewhere the user put it.
    if (active === focused || (active !== null && active !== document.body)) return;
    if (focused.isConnected) {
      focused.focus({ preventScroll: true });
      return;
    }
    const row = [...container.querySelectorAll<HTMLElement>('[data-row]')].find(
      (r) => r.dataset['row'] === rowId,
    );
    const more = row?.querySelector<HTMLElement>('[data-row-more]') ?? null;
    const target = more ?? fallback();
    focused = null;
    if (target === null) return;
    if (more === null && !target.hasAttribute('tabindex')) target.setAttribute('tabindex', '-1');
    target.focus();
  };

  const observer = new MutationObserver(restore);
  observer.observe(container, { childList: true, subtree: true });
  container.addEventListener('focusin', onFocusIn);
  document.addEventListener('focusin', onOutside);
  document.addEventListener('pointerdown', onOutside);
  return () => {
    observer.disconnect();
    container.removeEventListener('focusin', onFocusIn);
    document.removeEventListener('focusin', onOutside);
    document.removeEventListener('pointerdown', onOutside);
  };
}
