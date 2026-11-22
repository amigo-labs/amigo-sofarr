<script lang="ts">
  import PosterTile from '../../components/PosterTile.svelte';
  import { tileName } from '../../components/tile';
  import type { Row } from '../rows';
  import type { ListView } from '../view';
  import Label from './Label.svelte';
  import MonitorButton from './MonitorButton.svelte';
  import Poster from './Poster.svelte';

  // The rows of a movie or series list as list or poster grid; cards on narrow screens
  // (spec web-ui-library AC-02, AC-06, AC-22); the IMDb rating while ratings are on (spec
  // imdb-ratings AC-04, AC-09). Quiet cards and rows (spec ui-list-items): status chips, the
  // monitor toggle as an icon, the checkbox on hover, focus or while selecting, dimmed
  // posters of unmonitored items. The DOM order is the Tab order: title, checkbox, toggle.
  // In poster view every item is a poster tile (spec ui-cinema-tiles): its information in
  // the overlay, checkbox and toggle in the poster's top corners.
  type Props = {
    rows: readonly Row[];
    view: ListView;
    ratings: boolean;
    selected: ReadonlySet<number>;
    busy: ReadonlySet<number>;
    onselect: (id: number, on: boolean) => void;
    onmonitor: (row: Row) => void;
  };
  let { rows, view, ratings, selected, busy, onselect, onmonitor }: Props = $props();
</script>

{#snippet select(row: Row)}
  <label class="select">
    <input
      type="checkbox"
      checked={selected.has(row.id)}
      onchange={(e) => onselect(row.id, e.currentTarget.checked)}
    />
    <span class="sr-only">„{row.title}“ auswählen</span>
  </label>
{/snippet}

{#snippet monitor(row: Row)}
  <span class="monitor-slot">
    <MonitorButton
      icon
      monitored={row.monitored}
      title={row.title}
      busy={busy.has(row.id)}
      ontoggle={() => onmonitor(row)}
    />
  </span>
{/snippet}

<ul
  class="items"
  class:grid={view === 'poster'}
  class:selecting={selected.size > 0}
  data-testid="library-items"
>
  {#each rows as row (row.id)}
    {#if view === 'poster'}
      <PosterTile
        class="item"
        data-id={row.id}
        href={row.href}
        title={row.title}
        name={tileName(row.title, row.year, ratings ? row.ratingFull : null)}
        mark={row.mark}
        dimmed={!row.monitored}
        selected={selected.has(row.id)}
      >
        {#snippet poster()}
          <Poster
            kind={row.kind}
            id={row.id}
            title={row.title}
            hasPoster={row.hasPoster}
            size="list"
          />
        {/snippet}
        {#snippet overlay()}
          <span class="main">
            <span class="title" aria-hidden="true">{row.title}</span>
            <span class="meta">
              {[row.year, row.subtitle].filter((part) => part !== null).join(' · ')}
            </span>
            {#if ratings}
              <span class="rating" data-testid="rating" aria-hidden="true">{row.rating}</span>
            {/if}
            <span class="labels">
              {#each row.labels as label (label.text)}<Label
                  text={label.text}
                  tone={label.tone}
                />{/each}
            </span>
          </span>
        {/snippet}
        {#snippet controls()}
          {@render select(row)}
          {@render monitor(row)}
        {/snippet}
      </PosterTile>
    {:else}
      <li
        class="item"
        class:selected={selected.has(row.id)}
        class:unmonitored={!row.monitored}
        data-id={row.id}
      >
        <div class="poster-wrap">
          <Poster
            kind={row.kind}
            id={row.id}
            title={row.title}
            hasPoster={row.hasPoster}
            size="list"
          />
        </div>
        <div class="main">
          <span class="headline">
            <a class="title ui-link" href={row.href} title={row.title}>{row.title}</a>
            <span class="meta">
              {[row.year, row.subtitle].filter((part) => part !== null).join(' · ')}
            </span>
          </span>
          <span class="labels">
            {#each row.labels as label (label.text)}<Label
                text={label.text}
                tone={label.tone}
              />{/each}
          </span>
        </div>
        {@render select(row)}
        <span class="size meta">{row.size}</span>
        {#if ratings}
          <span
            class="rating meta"
            data-testid="rating"
            role="img"
            aria-label={row.ratingFull}
            title={row.ratingFull}>{row.rating}</span
          >
        {/if}
        {@render monitor(row)}
      </li>
    {/if}
  {/each}
</ul>

<style>
  .items {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-large);
    background: var(--bgColor-surface);
  }

  /* Rows: checkbox, poster, title and chips, size, rating, toggle. */
  .item {
    display: grid;
    grid-template-columns: 2rem 36px minmax(0, 1fr) auto auto auto;
    grid-template-areas: 'select poster main size rating monitor';
    gap: 0 var(--base-size-12);
    align-items: center;
    padding: 4px var(--base-size-12) 4px var(--base-size-4);
    border-top: var(--borderWidth-thin) solid var(--borderColor-muted);
  }

  .item:first-child {
    border-top: 0;
  }

  .items:not(.grid) .item.selected {
    background: var(--bgColor-accent-muted);
  }

  .poster-wrap {
    grid-area: poster;
    border-radius: var(--borderRadius-medium);
  }

  .unmonitored .poster-wrap {
    opacity: 0.55;
  }

  .main {
    grid-area: main;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .headline {
    display: flex;
    align-items: baseline;
    gap: var(--base-size-8);
    min-width: 0;
  }

  .title {
    overflow: hidden;
    font-weight: var(--base-text-weight-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
    white-space: nowrap;
  }

  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
  }

  .select {
    grid-area: select;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    cursor: pointer;
  }

  .size {
    grid-area: size;
    text-align: right;
  }

  .rating {
    grid-area: rating;
    font-variant-numeric: tabular-nums;
  }

  .monitor-slot {
    grid-area: monitor;
    display: inline-flex;
  }

  /* The checkbox shows on hover or focus, or while selecting; touch devices always show it. */
  @media (hover: hover) {
    .items:not(.selecting) :global(li:not(:hover, :focus-within) .select) {
      opacity: 0;
    }
  }

  /* Poster tiles (spec ui-cinema-tiles): a row is as high as its posters. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr));
    gap: var(--base-size-20) var(--base-size-16);
    border: 0;
    background: none;
  }

  .grid .title,
  .grid .meta {
    white-space: normal;
  }

  .grid .main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: flex-start;
    max-width: 100%;
  }

  .grid .labels {
    margin-top: 2px;
  }

  .grid .select,
  .grid .monitor-slot {
    position: absolute;
    top: var(--base-size-4);
    z-index: 2;
  }

  .grid .select {
    left: var(--base-size-4);
    border-radius: var(--borderRadius-full);
    background: color-mix(in srgb, var(--am-surface) 85%, transparent);
  }

  .grid .monitor-slot {
    right: var(--base-size-4);
  }

  /* Rows on narrow screens: cards with the rating below the chips (AC-22). */
  @media (max-width: 767.98px) {
    .items:not(.grid) {
      gap: var(--base-size-8);
      border: 0;
      background: none;
    }

    .items:not(.grid) .item {
      grid-template-columns: 2rem 36px minmax(0, 1fr) auto;
      grid-template-areas:
        'select poster main monitor'
        'select poster rating monitor';
      border: var(--borderWidth-thin) solid var(--borderColor-default);
      border-radius: var(--borderRadius-large);
      background: var(--bgColor-surface);
    }

    .items:not(.grid) .size {
      display: none;
    }

    .items:not(.grid) .rating {
      justify-self: start;
    }

    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
