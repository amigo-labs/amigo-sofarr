<script lang="ts">
  import type { TitleCard } from '../../api/generated/TitleCard';
  import type { State } from '../states.svelte';
  import TitleTile from './TitleTile.svelte';

  // Titles as a grid, or as one row that scrolls sideways (a shelf, spec discover AC-01).
  type Props = {
    cards: readonly TitleCard[];
    stateOf: (card: TitleCard) => State;
    noteOf?: (card: TitleCard) => string | null;
    canAdd: boolean;
    onadd: (card: TitleCard, state: State) => void;
    row?: boolean;
    label?: string;
  };
  let { cards, stateOf, noteOf, canAdd, onadd, row = false, label }: Props = $props();
</script>

<ul class="tiles" class:row aria-label={label}>
  {#each cards as card (`${card.kind}/${card.tmdb_id}`)}
    <TitleTile {card} state={stateOf(card)} note={noteOf?.(card) ?? null} {canAdd} {onadd} />
  {/each}
</ul>

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
    gap: var(--base-size-16);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* Positioned, so hidden texts of tiles scrolled out of view stay inside the row. */
  .row {
    position: relative;
    grid-template-columns: none;
    grid-auto-flow: column;
    grid-auto-columns: 9rem;
    overflow-x: auto;
    padding-bottom: var(--base-size-8);
    overscroll-behavior-x: contain;
  }

  @media (max-width: 543.98px) {
    .tiles:not(.row) {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .row {
      grid-auto-columns: 8rem;
    }
  }
</style>
