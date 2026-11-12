<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Flash from '#lib/components/Flash.svelte';
  import Icon from '#lib/components/Icon.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import type { ApiFailure } from '#lib/api/http.ts';
  import type { Page } from '#lib/api/generated/Page.ts';
  import type { SearchItem } from '#lib/api/generated/SearchItem.ts';
  import type { WantedItem } from '#lib/api/generated/WantedItem.ts';
  import type { WantedList } from '#lib/api/generated/WantedList.ts';
  import type { WantedMedia } from '#lib/api/generated/WantedMedia.ts';
  import { getWanted, searchBulk } from '#lib/api/library.ts';
  import { mediaHref, runSearch, searchBody } from '#lib/library/actions.ts';
  import { bulkSearchText, dateText, itemCount, searchTone } from '#lib/library/format.ts';
  import { notify } from '#lib/library/notices.svelte.ts';

  // Missing and cutoff-unmet items in two tabs, with searches for one, the selected or all
  // items of the tab (spec web-ui-library AC-08, AC-09).
  const list: WantedList = $derived(
    page.url.searchParams.get('liste') === 'cutoff' ? 'cutoff_unmet' : 'missing',
  );
  const media: WantedMedia = $derived(
    page.url.searchParams.get('art') === 'episoden' ? 'episodes' : 'movies',
  );
  const pageNumber = $derived(Math.max(1, Number(page.url.searchParams.get('seite')) || 1));

  let answer = $state<Page<WantedItem> | null>(null);
  let failure = $state<ApiFailure | null>(null);
  let selected = $state(new Set<number>());
  let running: string | null = $state(null);

  const pages = $derived(
    answer === null ? 1 : Math.max(1, Math.ceil(answer.total / answer.page_size)),
  );

  async function load(): Promise<void> {
    const result = await getWanted(list, media, pageNumber);
    if (result.ok) {
      answer = result.data;
      failure = null;
    } else if (result.failure.kind !== 'unauthorized') {
      failure = result.failure;
    }
  }

  $effect(() => {
    void list;
    void media;
    void pageNumber;
    answer = null;
    selected = new Set();
    void load();
  });

  function show(next: { liste?: WantedList; art?: WantedMedia; seite?: number }): void {
    const params = new URLSearchParams();
    if ((next.liste ?? list) === 'cutoff_unmet') params.set('liste', 'cutoff');
    if ((next.art ?? media) === 'episodes') params.set('art', 'episoden');
    const seite = next.seite ?? 1;
    if (seite > 1) params.set('seite', String(seite));
    const query = params.toString();
    void goto(query === '' ? '/fehlend' : `/fehlend?${query}`, { reset: false });
  }

  const target = (item: WantedItem): SearchItem =>
    item.kind === 'movie'
      ? searchBody({ movie_id: item.id })
      : searchBody({ series_id: item.series_id, season: item.season, episode: item.episode });

  const href = (item: WantedItem): string =>
    item.kind === 'movie' ? mediaHref('movie', item.id) : mediaHref('series', item.series_id ?? 0);

  async function bulk(label: string, items: SearchItem[] | null): Promise<void> {
    running = label;
    const result = await searchBulk(
      items === null ? { items: null, all: { list, media } } : { items, all: null },
    );
    running = null;
    if (result.ok) {
      notify(searchTone(result.data.errors), `${label}: ${bulkSearchText(result.data)}`);
      selected = new Set();
      void load();
    } else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `${label}: Suche fehlgeschlagen – ${result.failure.message}`);
    }
  }

  async function one(item: WantedItem): Promise<void> {
    running = `item-${item.id}`;
    await runSearch(item.label, target(item));
    running = null;
    void load();
  }

  function toggle(id: number, on: boolean): void {
    const next = new Set(selected);
    if (on) next.add(id);
    else next.delete(id);
    selected = next;
  }
</script>

<svelte:head>
  <title>Fehlend – sofarr</title>
</svelte:head>

<h1>Fehlend</h1>

<div class="tabs" role="tablist" aria-label="Liste">
  <button
    type="button"
    role="tab"
    aria-selected={list === 'missing'}
    onclick={() => show({ liste: 'missing' })}
  >
    Fehlend
  </button>
  <button
    type="button"
    role="tab"
    aria-selected={list === 'cutoff_unmet'}
    onclick={() => show({ liste: 'cutoff_unmet' })}
  >
    Cutoff nicht erreicht
  </button>
</div>

<div class="toolbar" role="tabpanel">
  <div class="group" role="group" aria-label="Art">
    <button
      type="button"
      class="btn"
      aria-pressed={media === 'movies'}
      onclick={() => show({ art: 'movies' })}
    >
      Filme
    </button>
    <button
      type="button"
      class="btn"
      aria-pressed={media === 'episodes'}
      onclick={() => show({ art: 'episodes' })}
    >
      Episoden
    </button>
  </div>
  {#if answer !== null && answer.total > 0}
    <button
      type="button"
      class="btn"
      disabled={selected.size === 0 || running !== null}
      onclick={() =>
        void bulk(
          'Ausgewählte',
          (answer?.items ?? []).filter((i) => selected.has(i.id)).map(target),
        )}
    >
      <Icon svg={searchIcon} />Ausgewählte suchen ({selected.size})
    </button>
    <button
      type="button"
      class="btn"
      disabled={running !== null}
      onclick={() => void bulk('Alle', null)}
    >
      <Icon svg={searchIcon} />Alle {answer.total} suchen
    </button>
  {/if}
  {#if running !== null}<span class="muted" role="status">Suche läuft …</span>{/if}
</div>

{#if failure !== null}
  <Flash tone="danger">
    <p>Die Liste konnte nicht geladen werden: {failure.message}</p>
    <button type="button" class="btn" onclick={() => void load()}>Erneut versuchen</button>
  </Flash>
{/if}

{#if answer === null && failure === null}
  <Loading label="Liste wird geladen …" />
{:else if answer !== null}
  <p class="muted" data-testid="wanted-total">
    {itemCount(answer.total, media === 'movies' ? 'movie' : 'episode')}
  </p>
  {#if answer.items.length === 0}
    <p class="muted">Nichts in dieser Liste.</p>
  {:else}
    <ul class="box rows">
      {#each answer.items as item (item.id)}
        <li class="box-row row" data-testid="wanted-item">
          <label class="select">
            <input
              type="checkbox"
              checked={selected.has(item.id)}
              onchange={(e) => toggle(item.id, e.currentTarget.checked)}
            />
            <span class="sr-only">„{item.label}“ auswählen</span>
          </label>
          <div class="main">
            <a href={href(item)}>{item.label}</a>
            <span class="muted small">
              Profil {item.profile} · <span data-testid="reason">{item.reason}</span>
              {#if item.air_date !== null}
                · ausgestrahlt {dateText(item.air_date)}{/if}
            </span>
          </div>
          <button
            type="button"
            class="btn"
            aria-label={`„${item.label}“ suchen`}
            disabled={running !== null}
            onclick={() => void one(item)}
          >
            <Icon svg={searchIcon} />
          </button>
        </li>
      {/each}
    </ul>
    {#if pages > 1}
      <nav class="pager" aria-label="Seiten">
        <button
          type="button"
          class="btn"
          disabled={pageNumber <= 1}
          onclick={() => show({ seite: pageNumber - 1 })}
        >
          Zurück
        </button>
        <span>Seite {pageNumber} von {pages}</span>
        <button
          type="button"
          class="btn"
          disabled={pageNumber >= pages}
          onclick={() => show({ seite: pageNumber + 1 })}
        >
          Weiter
        </button>
      </nav>
    {/if}
  {/if}
{/if}

<style>
  .tabs {
    display: flex;
    gap: var(--base-size-4);
    border-bottom: var(--borderWidth-thin) solid var(--borderColor-default);
  }

  .tabs button {
    padding: var(--base-size-8) var(--base-size-12);
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--fgColor-default);
    font: inherit;
    cursor: pointer;
  }

  .tabs button[aria-selected='true'] {
    border-bottom-color: var(--underlineNav-borderColor-active);
    font-weight: var(--base-text-weight-semibold);
  }

  .toolbar,
  .group,
  .pager {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .group button[aria-pressed='true'] {
    border-color: var(--borderColor-accent-emphasis);
    box-shadow: inset 0 0 0 1px var(--borderColor-accent-emphasis);
  }

  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--base-size-12);
  }

  .main {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .small {
    font-size: var(--text-body-size-small);
  }

  .select input {
    width: 1.1rem;
    height: 1.1rem;
  }

  p {
    margin: 0;
  }
</style>
