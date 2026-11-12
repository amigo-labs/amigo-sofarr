<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import type { Snippet } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import type { MediaKind } from '../../api/generated/MediaKind';
  import MonitorButton from '../components/MonitorButton.svelte';
  import Poster from '../components/Poster.svelte';

  // Poster, titles, overview text and the item's main actions (AC-05, AC-06, AC-09, AC-10).
  type Props = {
    kind: MediaKind;
    id: number;
    title: string;
    year: number | null;
    otherTitles: readonly string[];
    overview: string | null;
    hasPoster: boolean;
    monitored: boolean;
    busy: boolean;
    searching: boolean;
    onmonitor: () => void;
    onsearch: () => void;
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
    monitored,
    busy,
    searching,
    onmonitor,
    onsearch,
    facts,
  }: Props = $props();
</script>

<section class="head">
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
    display: grid;
    grid-template-columns: minmax(6rem, 14rem) minmax(0, 1fr);
    gap: var(--base-size-24);
    align-items: start;
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

  .year {
    color: var(--fgColor-muted);
    font-weight: var(--base-text-weight-normal);
  }

  p {
    margin: 0;
  }

  .overview {
    max-width: 50rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-8);
  }

  @media (max-width: 543.98px) {
    .head {
      grid-template-columns: minmax(0, 1fr);
    }

    .poster {
      max-width: 9rem;
    }
  }
</style>
