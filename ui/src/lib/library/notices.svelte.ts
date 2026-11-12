// Results of actions (searches, marking, saving, file actions) shown on the page where they
// started, and kept across one navigation (e.g. back to the list after removing an item).

export type NoticeTone = 'danger' | 'warning' | 'info';

export type Notice = {
  readonly id: number;
  readonly tone: NoticeTone;
  readonly text: string;
  /** Survives the next change of page (e.g. back to the list after removing). */
  readonly keep: boolean;
};

const state = $state<{ list: Notice[] }>({ list: [] });
let next = 0;

export const notices = {
  get list(): readonly Notice[] {
    return state.list;
  },
};

/** Shows a result; the newest comes first, at most five stay. */
export function notify(tone: NoticeTone, text: string, keep = false): void {
  next += 1;
  state.list = [{ id: next, tone, text, keep }, ...state.list].slice(0, 5);
}

export function dismiss(id: number): void {
  state.list = state.list.filter((n) => n.id !== id);
}

/** A new page shows only the notices meant to survive the change, once. */
export function pageChanged(): void {
  state.list = state.list
    .filter((n) => n.keep)
    .map((n) => ({ id: n.id, tone: n.tone, text: n.text, keep: false }));
}
