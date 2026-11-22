<script lang="ts">
  import plusIcon from '@primer/octicons/build/svg/plus-16.svg?raw';
  import { tick } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import PosterTile from '../../components/PosterTile.svelte';
  import { tileName } from '../../components/tile';
  import Label from '../../library/components/Label.svelte';
  import type { TitleCard } from '../../api/generated/TitleCard';
  import { titleHref } from '../links';
  import type { State } from '../states.svelte';
  import { KIND_TEXT, libraryLabel, libraryMark, ratingCompact } from '../texts';
  import TmdbImage from './TmdbImage.svelte';

  // One title of a shelf, list, search or person page (spec discover AC-01, AC-03, AC-05):
  // a poster tile (spec ui-cinema-tiles) whose overlay shows title, year, IMDb rating,
  // library state and the add action.
  type Props = {
    card: TitleCard;
    state: State;
    /** The role or job on a person page. */
    note?: string | null;
    /** Add actions are offered (a TMDB key is configured, AC-15). */
    canAdd: boolean;
    onadd: (card: TitleCard, state: State) => void;
  };
  let { card, state: current, note = null, canAdd, onadd }: Props = $props();

  const label = $derived(libraryLabel(current.library.status));
  const rating = $derived(ratingCompact(current.rating));
  const canOffer = $derived(canAdd && current.library.status !== 'in_library');

  let link: HTMLAnchorElement | undefined = $state();
  /** The add dialog was opened from this tile's button. */
  let adding = false;

  function add(): void {
    adding = true;
    onadd(card, current);
  }

  // A title added from this tile: its button gives way to the library state, and the focus,
  // which the closing dialog cannot return to the button, moves to the tile's link.
  $effect(() => {
    if (current.library.status !== 'in_library' || !adding) return;
    adding = false;
    void tick().then(() =>
      requestAnimationFrame(() => {
        const active = document.activeElement;
        if (active === null || active === document.body) link?.focus();
      }),
    );
  });
</script>

<PosterTile
  data-testid="title-card"
  data-key="{card.kind}/{card.tmdb_id}"
  href={titleHref(card.kind, card.tmdb_id, current.library)}
  title={card.title}
  name={tileName(card.title, card.year, rating?.full ?? null)}
  mark={libraryMark(current.library.status)}
  bind:link
>
  {#snippet poster()}
    <TmdbImage src={card.poster} name={card.title} />
  {/snippet}
  {#snippet overlay()}
    <span class="title" aria-hidden="true">{card.title}</span>
    <span class="meta"
      >{KIND_TEXT[card.kind]}{#if card.year !== null}&nbsp;· {card.year}{/if}</span
    >
    {#if note !== null && note !== ''}<span class="meta note">{note}</span>{/if}
    {#if rating !== null}
      <!-- A pending rating keeps an empty line (ui-polish AC-02). -->
      <span
        class="rating"
        class:pending={current.rating.state === 'pending'}
        data-testid="rating"
        aria-hidden="true">{rating.text}</span
      >
    {/if}
    {#if label !== null}
      <span class="state" data-testid="library-state"
        ><Label text={label.text} tone={label.tone} /></span
      >
    {/if}
    {#if canOffer}
      <button type="button" class="btn add" onclick={add} onfocus={() => (adding = false)}>
        <Icon svg={plusIcon} /><span>Hinzufügen</span><span class="sr-only"
          >{` „${card.title}“`}</span
        >
      </button>
    {/if}
  {/snippet}
</PosterTile>

<style>
  .rating {
    min-width: 1.5em;
    min-height: 1lh;
  }

  .state {
    display: flex;
    margin-top: 2px;
  }

  .state :global(.label),
  .add {
    box-sizing: border-box;
    height: 1.75rem;
    min-height: 1.75rem;
  }

  /* One line also on a narrow tile. */
  .add {
    gap: var(--base-size-4);
    max-width: 100%;
    margin-top: 2px;
    padding: 0 var(--base-size-8);
    white-space: nowrap;
  }
</style>
