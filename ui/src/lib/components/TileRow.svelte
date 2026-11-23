<script lang="ts">
  import type { Snippet } from 'svelte';
  import ShelfArrows from './ShelfArrows.svelte';

  // A row of poster tiles (spec ui-cinema-home): its heading, „Alle anzeigen“ and the arrows
  // of the discover shelves (spec ui-discover-polish AC-03), then one line of tiles that
  // scrolls sideways. The list is named by the heading. `children` are the tiles (`<li>`).
  // `keepRowFocus` finds the row by `data-row` and its link by `data-row-more`.
  type Props = {
    /** Unique on the page: the heading's id and the row's name for `keepRowFocus`. */
    id: string;
    title: string;
    /** Where „Alle anzeigen“ leads. */
    href: string;
    /** The number of tiles: the arrows measure the row's ends again when it changes. */
    count: number;
    /** Called on a click of „Alle anzeigen“ before the browser follows the link. */
    onmore?: (event: MouseEvent) => void;
    children: Snippet;
  };
  let { id, title, href, count, onmore, children }: Props = $props();
</script>

<section class="tile-row" aria-labelledby="{id}-title" data-row={id} data-testid="tile-row">
  <div class="row-head">
    <h2 id="{id}-title">{title}</h2>
    <span class="row-tools">
      <a class="ui-link" {href} data-row-more onclick={onmore}
        >Alle anzeigen<span class="sr-only">{`: ${title}`}</span></a
      >
      <ShelfArrows name={title} {count} />
    </span>
  </div>
  <ul class="tiles" aria-labelledby="{id}-title">
    {@render children()}
  </ul>
</section>

<style>
  .tile-row {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    min-width: 0;
  }

  .row-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--base-size-8);
  }

  h2 {
    margin: 0;
  }

  .row-tools {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-12);
  }

  /* As the discover shelves (TitleList with `row`): positioned, so hidden texts of tiles
     scrolled out of view stay inside the row; room above and below for an open tile, which
     grows by 7 % (spec ui-cinema-tiles AC-03). */
  .tiles {
    position: relative;
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 9rem;
    gap: var(--base-size-16);
    margin: 0;
    padding: var(--base-size-12) 0 var(--base-size-16);
    overflow-x: auto;
    overscroll-behavior-x: contain;
    list-style: none;
  }

  @media (max-width: 543.98px) {
    .tiles {
      grid-auto-columns: 8rem;
    }
  }
</style>
