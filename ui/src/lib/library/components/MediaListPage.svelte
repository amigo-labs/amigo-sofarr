<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import type { MonitorItem } from '../../api/generated/MonitorItem';
  import type { MovieListItem } from '../../api/generated/MovieListItem';
  import type { SeriesListItem } from '../../api/generated/SeriesListItem';
  import { listMovies, listSeries, monitor } from '../../api/library';
  import { ensureOptions, options } from '../options.svelte';
  import { itemCount } from '../format';
  import {
    apiQuery,
    filterParams,
    isFiltered,
    parseFilters,
    type ListFilters,
  } from '../list-query';
  import { notify } from '../notices.svelte';
  import { PAGE_SIZE, createPaged } from '../paged.svelte';
  import { movieRow, seriesRow, type Row } from '../rows';
  import { readView, storeView, viewKey, type ListView } from '../view';
  import Dialog from './Dialog.svelte';
  import ListControls from './ListControls.svelte';
  import MediaItems from './MediaItems.svelte';

  // The movie or series list (spec web-ui-library AC-02 to AC-04, AC-06, AC-10, AC-22).
  type Props = { kind: 'movie' | 'series' };
  let { kind }: Props = $props();

  type Item = MovieListItem | SeriesListItem;

  const list = createPaged<Item>();
  const storage = (): Storage => window.localStorage;
  const key = $derived(viewKey(kind === 'movie' ? 'filme' : 'serien'));
  const heading = $derived(kind === 'movie' ? 'Filme' : 'Serien');

  let view: ListView = $state('liste');
  let search = $state(page.url.searchParams.get('suche') ?? '');
  let selected = $state(new Set<number>());
  let busy = $state(new Set<number>());
  let bulk: boolean | null = $state(null);
  let bulkBusy = $state(false);
  let sentinel: HTMLElement | undefined = $state();

  const filters = $derived(parseFilters(kind, page.url.searchParams));
  const query = $derived(apiQuery(kind, filters, 1, PAGE_SIZE));
  const rows = $derived(
    list.items.map((item) => ('episodes' in item ? seriesRow(item) : movieRow(item))),
  );

  const load = (pageNumber: number) => {
    const q = apiQuery(kind, filters, pageNumber, PAGE_SIZE);
    return kind === 'movie' ? listMovies(q) : listSeries(q);
  };

  onMount(() => {
    view = readView(storage, key);
  });

  // A new query (filters, search, sorting) replaces the list and the selection.
  $effect(() => {
    void query;
    untrack(() => {
      selected = new Set();
      // The choices of the filters load after the first entries: they are not needed first.
      void (async () => {
        await list.reset(load);
        await ensureOptions();
      })();
    });
  });

  // Further entries load while the end of the list comes into view.
  $effect(() => {
    if (sentinel === undefined) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) void list.more();
      },
      { rootMargin: '400px' },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  function apply(next: ListFilters): void {
    const params = filterParams(kind, next).toString();
    void goto(params === '' ? page.url.pathname : `?${params}`, {
      replace: true,
      reset: false,
    });
  }

  function onsearch(text: string): void {
    search = text;
    apply({ ...filters, q: text });
  }

  function onview(next: ListView): void {
    view = next;
    storeView(storage, key, next);
  }

  const item = (id: number): MonitorItem =>
    kind === 'movie' ? { kind: 'movie', id } : { kind: 'series', id };

  async function toggle(row: Row): Promise<void> {
    busy = new Set([...busy, row.id]);
    const result = await monitor(!row.monitored, [item(row.id)]);
    busy = new Set([...busy].filter((id) => id !== row.id));
    if (result.ok) {
      list.update(
        (i) => i.id === row.id,
        (i) => ({ ...i, monitored: !row.monitored }),
      );
    } else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `Überwachung nicht geändert: ${result.failure.message}`);
    }
  }

  function select(id: number, on: boolean): void {
    const next = new Set(selected);
    if (on) next.add(id);
    else next.delete(id);
    selected = next;
  }

  async function bulkMonitor(on: boolean): Promise<void> {
    bulkBusy = true;
    const ids = [...selected];
    const result = await monitor(on, ids.map(item));
    bulkBusy = false;
    bulk = null;
    if (result.ok) {
      const chosen = new Set(ids);
      list.update(
        (i) => chosen.has(i.id),
        (i) => ({ ...i, monitored: on }),
      );
      selected = new Set();
      const what = itemCount(result.data.changed, kind);
      notify('info', on ? `${what} werden überwacht.` : `${what} werden nicht mehr überwacht.`);
    } else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `Nichts geändert: ${result.failure.message}`);
    }
  }
</script>

<svelte:head>
  <title>{heading} – sofarr</title>
</svelte:head>

<div class="head">
  <h1>{heading}</h1>
  {#if list.total !== null}
    <span class="muted" role="status" data-testid="total">{itemCount(list.total, kind)}</span>
  {/if}
</div>

<ListControls
  {kind}
  {filters}
  {search}
  profiles={(kind === 'movie' ? options.value?.movie_profiles : options.value?.series_profiles) ??
    []}
  tags={(kind === 'movie' ? options.value?.movie_tags : options.value?.series_tags) ?? []}
  {view}
  {onsearch}
  onchange={apply}
  {onview}
/>

{#if selected.size > 0}
  <div class="bulk" role="region" aria-label="Auswahl">
    <span><strong>{selected.size}</strong> ausgewählt</span>
    <button type="button" class="btn" onclick={() => (bulk = true)}>Überwachen</button>
    <button type="button" class="btn" onclick={() => (bulk = false)}>Nicht überwachen</button>
    <button type="button" class="btn" onclick={() => (selected = new Set())}>
      Auswahl aufheben
    </button>
  </div>
{/if}

{#if list.failure !== null}
  <Flash tone="danger">
    <p>Die Liste konnte nicht geladen werden: {list.failure.message}</p>
    <button type="button" class="btn" onclick={() => void list.reset(load)}>Erneut versuchen</button
    >
  </Flash>
{/if}

{#if list.total === 0}
  <p class="empty">
    {isFiltered(filters)
      ? `Keine ${heading} passen zu Suche und Filtern.`
      : `Die Bibliothek enthält noch keine ${heading}.`}
  </p>
{:else if list.total !== null}
  <MediaItems
    {rows}
    {view}
    {selected}
    {busy}
    onselect={select}
    onmonitor={(row) => void toggle(row)}
  />
{/if}

<div bind:this={sentinel} class="sentinel">
  {#if list.loading}
    <Loading label={`${heading} werden geladen …`} />
  {:else if list.total !== null && !list.done}
    <button type="button" class="btn" onclick={() => void list.more()}>Weitere laden</button>
  {/if}
</div>

<Dialog
  open={bulk !== null}
  title={bulk ? 'Auswahl überwachen' : 'Auswahl nicht mehr überwachen'}
  onclose={() => (bulk = null)}
>
  <p>
    {itemCount(selected.size, kind)}
    {bulk ? 'überwachen' : 'nicht mehr überwachen'}? Es werden alle oder – bei einem Fehler – keine
    geändert.
  </p>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => (bulk = null)}>Abbrechen</button>
    <button
      type="button"
      class="btn btn-primary"
      disabled={bulkBusy}
      onclick={() => void bulkMonitor(bulk === true)}
    >
      {itemCount(selected.size, kind)}
      {bulk ? 'überwachen' : 'nicht überwachen'}
    </button>
  {/snippet}
</Dialog>

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--base-size-8) var(--base-size-16);
  }

  .bulk {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
    padding: var(--base-size-8) var(--base-size-12);
    border: var(--borderWidth-thin) solid var(--borderColor-accent-emphasis);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-default);
  }

  .empty {
    margin: 0;
    color: var(--fgColor-muted);
  }

  .sentinel {
    display: flex;
    justify-content: center;
    min-height: 2rem;
  }
</style>
