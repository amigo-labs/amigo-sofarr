<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import type { Snippet } from 'svelte';
  import HeadBackdrop from '../../components/HeadBackdrop.svelte';
  import Icon from '../../components/Icon.svelte';
  import type { MediaKind } from '../../api/generated/MediaKind';
  import { posterAddress } from '../../api/library';
  import MonitorButton from '../components/MonitorButton.svelte';
  import Poster from '../components/Poster.svelte';

  // Poster, titles, overview text and the item's main actions (AC-05, AC-06, AC-09, AC-10),
  // over the title's backdrop, or else its poster, blurred (spec ui-cinema-backdrops AC-05,
  // AC-06; ui-detail-head AC-02): titles, status chips, rating, overview, actions, facts.
  type Props = {
    kind: MediaKind;
    id: number;
    title: string;
    year: number | null;
    otherTitles: readonly string[];
    overview: string | null;
    hasPoster: boolean;
    /** sofarr's address of the title's backdrop, if it has one. */
    backdrop: string | null;
    monitored: boolean;
    busy: boolean;
    searching: boolean;
    onmonitor: () => void;
    onsearch: () => void;
    chips: Snippet;
    rating: Snippet;
    facts: Snippet;
  };
  let {
    kind,
    id,
    title,
    year,
    otherTitles,
    overview,
    hasPoster,
    backdrop,
    monitored,
    busy,
    searching,
    onmonitor,
    onsearch,
    chips,
    rating,
    facts,
  }: Props = $props();
</script>

<section class="head">
  <HeadBackdrop image={backdrop} fallback={hasPoster ? posterAddress(kind, id, 'detail') : null} />
  <div class="poster">
    <Poster {kind} {id} {title} {hasPoster} size="detail" />
  </div>
  <div class="text">
    <h1>
      {title}<span class="year">{year === null ? '' : ` (${year})`}</span>
    </h1>
    {#if otherTitles.length > 0}
      <p class="muted">Auch: {otherTitles.join(' · ')}</p>
    {/if}
    <div class="labels">{@render chips()}</div>
    {@render rating()}
    {#if overview !== null}<p class="overview">{overview}</p>{/if}
    <div class="actions">
      <MonitorButton {monitored} {title} {busy} ontoggle={onmonitor} />
      <button type="button" class="btn" disabled={searching} onclick={onsearch}>
        <Icon svg={searchIcon} />{searching ? 'Suche läuft …' : 'Suchen'}
      </button>
    </div>
    {@render facts()}
  </div>
</section>

<style>
  .head {
    position: relative;
    isolation: isolate;
    display: grid;
    grid-template-columns: minmax(6rem, 14rem) minmax(0, 1fr);
    gap: var(--base-size-24);
    align-items: start;
    padding: var(--base-size-24);
    margin: calc(-1 * var(--base-size-24)) calc(-1 * var(--base-size-16)) 0;
    overflow: hidden;
  }

  /* Room for the backdrop to read as an image (spec ui-cinema-backdrops AC-05). */
  @media (min-width: 1024px) {
    .head {
      min-height: 420px;
    }
  }

  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
  }

  .labels:empty {
    display: none;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
    min-width: 0;
  }

  h1 {
    overflow-wrap: anywhere;
  }

  /* The head's content keeps to the measure the backdrop's scrim is strong under, which
     leaves the image free beside it (spec ui-cinema-backdrops AC-05, AC-07). */
  .text > :global(*) {
    max-width: 42rem;
  }

  .year {
    color: var(--fgColor-muted);
    font-weight: var(--base-text-weight-normal);
  }

  p {
    margin: 0;
  }

  .overview {
    max-width: 42rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-8);
  }

  @media (max-width: 767.98px) {
    .head {
      grid-template-columns: minmax(0, 1fr);
      padding: var(--base-size-16);
    }

    .poster {
      max-width: 9rem;
    }
  }
</style>
