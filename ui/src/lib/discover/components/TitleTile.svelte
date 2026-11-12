<script lang="ts">
  import plusIcon from '@primer/octicons/build/svg/plus-16.svg?raw';
  import Icon from '../../components/Icon.svelte';
  import Label from '../../library/components/Label.svelte';
  import type { TitleCard } from '../../api/generated/TitleCard';
  import { titleHref } from '../links';
  import type { State } from '../states.svelte';
  import { KIND_TEXT, libraryLabel } from '../texts';
  import RatingText from './RatingText.svelte';
  import TmdbImage from './TmdbImage.svelte';

  // One title of a shelf, list, search or person page (spec discover AC-01, AC-03, AC-05):
  // poster, title, year, IMDb rating, library state and the add action.
  type Props = {
    card: TitleCard;
    state: State;
    /** The role or job on a person page. */
    note?: string | null;
    /** Add actions are offered (a TMDB key is configured, AC-15). */
    canAdd: boolean;
    onadd: (card: TitleCard, state: State) => void;
  };
  let { card, state, note = null, canAdd, onadd }: Props = $props();

  const label = $derived(libraryLabel(state.library.status));
</script>

<li class="tile" data-testid="title-card" data-key="{card.kind}/{card.tmdb_id}">
  <a class="link" href={titleHref(card.kind, card.tmdb_id, state.library)}>
    <TmdbImage src={card.poster} name={card.title} />
    <span class="title">{card.title}</span>
  </a>
  <span class="meta"
    >{KIND_TEXT[card.kind]}{#if card.year !== null}&nbsp;· {card.year}{/if}</span
  >
  {#if note !== null && note !== ''}<span class="meta note">{note}</span>{/if}
  <RatingText rating={state.rating} />
  {#if label !== null}
    <span data-testid="library-state"><Label text={label.text} tone={label.tone} /></span>
  {/if}
  {#if canAdd && state.library.status !== 'in_library'}
    <button type="button" class="btn add" onclick={() => onadd(card, state)}>
      <Icon svg={plusIcon} /><span>Hinzufügen</span><span class="sr-only">{` „${card.title}“`}</span
      >
    </button>
  {/if}
</li>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    min-width: 0;
  }

  .link {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
  }

  .title {
    font-weight: var(--base-text-weight-semibold);
    overflow-wrap: anywhere;
  }

  .meta {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
    overflow-wrap: anywhere;
  }

  .add {
    align-self: flex-start;
    margin-top: auto;
  }
</style>
