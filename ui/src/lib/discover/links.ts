// Addresses of the discover pages (spec discover AC-01, AC-04, AC-05): German like the
// library pages; a title in the library opens its library page.

import type { LibraryState } from '../api/generated/LibraryState';
import type { ShelfId } from '../api/generated/ShelfId';
import type { TitleKind } from '../api/generated/TitleKind';
import { mediaHref } from '../library/actions';

export const DISCOVER_PATH = '/entdecken';
export const SEARCH_PATH = '/entdecken/suche';

/** The shelves by their address segment, with their German names. */
export const SHELVES: readonly {
  readonly id: ShelfId;
  readonly slug: string;
  readonly title: string;
}[] = [
  { id: 'trending_day', slug: 'trending-heute', title: 'Trending heute' },
  { id: 'trending_week', slug: 'trending-woche', title: 'Trending diese Woche' },
  { id: 'popular_movies', slug: 'beliebte-filme', title: 'Beliebte Filme' },
  { id: 'upcoming', slug: 'demnaechst-im-kino', title: 'Demnächst im Kino' },
  { id: 'now_playing', slug: 'jetzt-im-kino', title: 'Jetzt im Kino' },
  { id: 'new_at_home', slug: 'neu-als-stream-oder-disc', title: 'Neu als Stream oder Disc' },
  { id: 'popular_series', slug: 'beliebte-serien', title: 'Beliebte Serien' },
];

export const shelfHref = (id: ShelfId): string =>
  `${DISCOVER_PATH}/regal/${SHELVES.find((s) => s.id === id)?.slug ?? id}`;

export const shelfOf = (slug: string | undefined) => SHELVES.find((s) => s.slug === slug) ?? null;

/** The preview page of a title. */
export const previewHref = (kind: TitleKind, tmdbId: number): string =>
  `${DISCOVER_PATH}/${kind === 'movie' ? 'filme' : 'serien'}/${tmdbId}`;

/** Where a title links to: its library page while it is in the library (AC-04). */
export const titleHref = (kind: TitleKind, tmdbId: number, library: LibraryState): string =>
  library.status === 'in_library' && library.id !== null
    ? mediaHref(kind, library.id)
    : previewHref(kind, tmdbId);

export const personHref = (id: number): string => `${DISCOVER_PATH}/personen/${id}`;

export const searchHref = (text: string): string =>
  `${SEARCH_PATH}?${new URLSearchParams({ begriff: text.trim() }).toString()}`;
