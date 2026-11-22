// What a poster tile tells at rest (spec ui-cinema-tiles AC-02, AC-05): its state mark and
// the accessible name of its link, which carries what the hidden overlay shows.

/** A tile's state mark: a round dot in the poster's lower left corner. */
export type Mark = { readonly text: string; readonly tone: 'accent' | 'danger' };

/**
 * The name of a tile's link: the full title, its year and its rating with the votes, e.g.
 * „Nordlicht über Tromsø, 2023, IMDb 7,6 (48.210 Stimmen)“; `rating` is `null` while
 * ratings are off.
 */
export function tileName(title: string, year: number | null, rating: string | null): string {
  return [title, year === null ? null : String(year), rating]
    .filter((part) => part !== null)
    .join(', ');
}
