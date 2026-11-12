<script lang="ts">
  import type { Row } from '../rows';
  import type { ListView } from '../view';
  import Label from './Label.svelte';
  import MonitorButton from './MonitorButton.svelte';
  import Poster from './Poster.svelte';

  // The rows of a movie or series list as list or poster grid; cards on narrow screens
  // (spec web-ui-library AC-02, AC-06, AC-22); the IMDb rating while ratings are on (spec
  // imdb-ratings AC-04, AC-09).
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

<ul class="items" class:grid={view === 'poster'} data-testid="library-items">
  {#each rows as row (row.id)}
    <li class="item" data-id={row.id}>
      <div class="poster">
        <Poster
          kind={row.kind}
          id={row.id}
          title={row.title}
          hasPoster={row.hasPoster}
          size="list"
        />
      </div>
      <div class="main">
        <a class="title" href={row.href}>{row.title}</a>
        <span class="meta">
          {#if row.year !== null}{row.year}{/if}
          {#if row.subtitle !== null}· {row.subtitle}{/if}
        </span>
        <span class="meta file"
          >{row.file}{#if view === 'liste'}
            · {row.size}{/if}</span
        >
        {#if ratings}
          <span class="meta" data-testid="rating">IMDb {row.rating}</span>
        {/if}
        {#if row.labels.length > 0}
          <span class="labels">
            {#each row.labels as label (label.text)}<Label
                text={label.text}
                tone={label.tone}
              />{/each}
          </span>
        {/if}
      </div>
      <div class="actions">
        <label class="select">
          <input
            type="checkbox"
            checked={selected.has(row.id)}
            onchange={(e) => onselect(row.id, e.currentTarget.checked)}
          />
          <span class="sr-only">„{row.title}“ auswählen</span>
        </label>
        <MonitorButton
          monitored={row.monitored}
          title={row.title}
          busy={busy.has(row.id)}
          ontoggle={() => onmonitor(row)}
        />
      </div>
    </li>
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
    border-radius: var(--borderRadius-medium);
  }

  .item {
    display: grid;
    grid-template-columns: 2.5rem minmax(0, 1fr) auto;
    gap: var(--base-size-12);
    align-items: center;
    padding: var(--base-size-8) var(--base-size-12);
    border-top: var(--borderWidth-thin) solid var(--borderColor-muted);
  }

  .item:first-child {
    border-top: 0;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .title {
    font-weight: var(--base-text-weight-semibold);
    overflow-wrap: anywhere;
  }

  .meta {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }

  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--base-size-8);
  }

  .select {
    display: inline-flex;
    padding: var(--base-size-4);
  }

  .select input {
    width: 1.1rem;
    height: 1.1rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
    gap: var(--base-size-16);
    border: 0;
  }

  .grid .item {
    grid-template-columns: minmax(0, 1fr);
    align-items: start;
    padding: 0;
    border: 0;
  }

  .grid .actions {
    justify-content: space-between;
  }

  /* Cards on narrow screens (AC-22). */
  @media (max-width: 767.98px) {
    .items:not(.grid) {
      gap: var(--base-size-8);
      border: 0;
    }

    .items:not(.grid) .item {
      grid-template-columns: 3rem minmax(0, 1fr);
      border: var(--borderWidth-thin) solid var(--borderColor-default);
      border-radius: var(--borderRadius-medium);
    }

    .items:not(.grid) .actions {
      grid-column: 1 / -1;
      justify-content: space-between;
    }

    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
