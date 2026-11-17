<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import type { Snippet } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import type { MediaKind } from '../../api/generated/MediaKind';
  import { posterAddress } from '../../api/library';
  import MonitorButton from '../components/MonitorButton.svelte';
  import Poster from '../components/Poster.svelte';

  // Poster, titles, overview text and the item's main actions (AC-05, AC-06, AC-09, AC-10),
  // over the title's own poster, blurred (spec ui-detail-head AC-01, AC-02): titles, status
  // chips, rating, overview, actions, facts.
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
    monitored,
    busy,
    searching,
    onmonitor,
    onsearch,
    chips,
    rating,
    facts,
  }: Props = $props();

  let backdropFailed = $state(false);
</script>

<section class="head">
  {#if hasPoster && !backdropFailed}
    <div class="backdrop" aria-hidden="true">
      <img
        src={posterAddress(kind, id, 'detail')}
        alt=""
        decoding="async"
        onerror={() => (backdropFailed = true)}
      />
      <div class="backdrop-veil"></div>
    </div>
  {/if}
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

  .backdrop {
    position: absolute;
    inset: 0;
    z-index: -1;
    overflow: hidden;
  }

  .backdrop img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(28px) saturate(1.2);
    transform: scale(1.15);
  }

  .backdrop-veil {
    position: absolute;
    inset: 0;
    background: var(--am-bg);
    opacity: 0.78;
  }

  .backdrop::after {
    content: '';
    position: absolute;
    inset: auto 0 0;
    height: 35%;
    background: linear-gradient(transparent, var(--am-bg));
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
