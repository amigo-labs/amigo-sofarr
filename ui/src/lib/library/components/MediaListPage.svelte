<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import TileRow from '../../components/TileRow.svelte';
  import type { MonitorItem } from '../../api/generated/MonitorItem';
  import type { MovieListItem } from '../../api/generated/MovieListItem';
  import type { SeriesListItem } from '../../api/generated/SeriesListItem';
  import { listMovies, listSeries, monitor } from '../../api/library';
  import ImdbAttribution from '../../imdb/ImdbAttribution.svelte';
  import { imdb, loadImdb } from '../../imdb/info.svelte';
  import { ensureOptions, options } from '../options.svelte';
  import { itemCount } from '../format';
  import {
    apiQuery,
    filterParams,
    isFiltered,
    parseFilters,
    usesRatings,
    withoutRatings,
    type ListFilters,
  } from '../list-query';
  import { notify } from '../notices.svelte';
  import { PAGE_SIZE, createPaged } from '../paged.svelte';
  import {
    LIST_ROWS,
    isPlain,
    keepsRows,
    listRowHref,
    listRowQuery,
    listRowTiles,
    type ListRow,
    type ListRowId,
  } from '../list-rows';
  import { movieRow, seriesRow } from '../rows';
  import { readView, storeView, viewKey, type ListView } from '../view';
  import Dialog from './Dialog.svelte';
  import ListControls from './ListControls.svelte';
  import MediaItems from './MediaItems.svelte';
  import RowTile from './RowTile.svelte';

  // The movie or series list (spec web-ui-library AC-02 to AC-04, AC-06, AC-10, AC-22) with
  // IMDb ratings while they are on (spec imdb-ratings AC-04, AC-06 to AC-09). While the list
  // is plain, rows of poster tiles above it (spec ui-cinema-library-rows).
  type Props = { kind: 'movie' | 'series' };
  let { kind }: Props = $props();

  type Item = MovieListItem | SeriesListItem;

  const list = createPaged<Item>();
  const storage = (): Storage => window.localStorage;
  const key = $derived(viewKey(kind === 'movie' ? 'filme' : 'serien'));
  const heading = $derived(kind === 'movie' ? 'Filme' : 'Serien');
  const listHeading = $derived(kind === 'movie' ? 'Alle Filme' : 'Alle Serien');

  let view: ListView = $state('liste');
  let search = $state(page.url.searchParams.get('suche') ?? '');
  let selected = $state(new Set<number>());
  let busy = $state(new Set<number>());
  let bulk: boolean | null = $state(null);
  let bulkBusy = $state(false);
  let sentinel: HTMLElement | undefined = $state();
  let allTitle: HTMLElement | undefined = $state();
  /** The items of each row; `null` before it loaded or after it failed (AC-12). */
  let rowItems: Record<ListRowId, Item[] | null> = $state({
    added: null,
    missing: null,
    cutoff: null,
    rated: null,
  });

  const addressFilters = $derived(parseFilters(kind, page.url.searchParams));
  // While ratings are off the server refuses rating sort and minimums (AC-09).
  const filters = $derived(
    imdb.info?.enabled === false ? withoutRatings(addressFilters) : addressFilters,
  );
  // A list sorted or filtered by rating waits for the ratings state; others load at once.
  const ready = $derived(imdb.settled || !usesRatings(addressFilters));
  const query = $derived(apiQuery(kind, filters, 1, PAGE_SIZE));
  const rows = $derived(
    list.items.map((item) => ('episodes' in item ? seriesRow(item) : movieRow(item))),
  );

  // No search text and no filter: the rows show (AC-04).
  const plain = $derived(isPlain(filters));
  const shownRows = $derived(
    LIST_ROWS.filter((r) => !r.ratings || imdb.shown)
      .map((r) => ({ row: r, tiles: listRowTiles(r, rowItems[r.id] ?? [], imdb.shown) }))
      .filter((r) => r.tiles.length > 0),
  );

  /** Loads one row; one that cannot be loaded does not show until it loads again (AC-12). */
  const rowGeneration: Record<ListRowId, number> = { added: 0, missing: 0, cutoff: 0, rated: 0 };
  async function loadRow(row: ListRow): Promise<void> {
    rowGeneration[row.id] += 1;
    const mine = rowGeneration[row.id];
    const rowQuery = listRowQuery(kind, row);
    const result = await (kind === 'movie' ? listMovies(rowQuery) : listSeries(rowQuery));
    if (mine !== rowGeneration[row.id]) return;
    rowItems[row.id] = result.ok ? result.data.items : null;
  }

  const load = (pageNumber: number) => {
    const q = apiQuery(kind, filters, pageNumber, PAGE_SIZE);
    return kind === 'movie' ? listMovies(q) : listSeries(q);
  };

  onMount(() => {
    view = readView(storage, key);
    void loadImdb();
  });

  // A new query (filters, search, sorting) replaces the list and the selection.
  $effect(() => {
    if (!ready) return;
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

  // The rows load when the page opens and when the list becomes plain again (AC-12), after
  // the list's first page has been asked for; „Am besten bewertet“ once ratings are on.
  $effect(() => {
    if (!plain) return;
    untrack(() => {
      for (const row of LIST_ROWS) if (!row.ratings) void loadRow(row);
    });
  });
  $effect(() => {
    if (!plain || !imdb.shown) return;
    untrack(() => {
      for (const row of LIST_ROWS) if (row.ratings) void loadRow(row);
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

  const withMonitored = (i: Item, on: boolean): Item => ({ ...i, monitored: on });

  /** Shows a change of monitoring on every tile of these titles: rows and full list (AC-07). */
  function showMonitored(ids: ReadonlySet<number>, on: boolean): void {
    list.update(
      (i) => ids.has(i.id),
      (i) => ({ ...i, monitored: on }),
    );
    for (const row of LIST_ROWS) {
      const items = rowItems[row.id];
      if (items === null || !items.some((i) => ids.has(i.id))) continue;
      rowItems[row.id] = items.map((i) => (ids.has(i.id) ? withMonitored(i, on) : i));
    }
  }

  async function toggle(id: number, monitored: boolean): Promise<void> {
    busy = new Set([...busy, id]);
    const result = await monitor(!monitored, [item(id)]);
    busy = new Set([...busy].filter((b) => b !== id));
    if (result.ok) {
      showMonitored(new Set([id]), !monitored);
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

  /**
   * „Alle anzeigen“ of a row (AC-05): the full list with the row's sort and filter, in the
   * toolbar and the address; then the page shows the full list, below the rows that stay or
   * at its top when they hide. A click with a modifier key opens the link as the browser does.
   */
  async function showAll(event: MouseEvent, row: ListRow): Promise<void> {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return;
    }
    event.preventDefault();
    search = '';
    await goto(listRowHref(kind, row), { reset: false });
    await tick();
    if (keepsRows(row)) allTitle?.scrollIntoView({ block: 'start' });
    else window.scrollTo({ top: 0 });
    allTitle?.focus({ preventScroll: true });
  }

  async function bulkMonitor(on: boolean): Promise<void> {
    bulkBusy = true;
    const ids = [...selected];
    const result = await monitor(on, ids.map(item));
    bulkBusy = false;
    bulk = null;
    if (result.ok) {
      showMonitored(new Set(ids), on);
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

{#if plain}
  {#each shownRows as { row, tiles } (row.id)}
    <TileRow
      id="row-{row.id}"
      title={row.title}
      href={listRowHref(kind, row)}
      count={tiles.length}
      onmore={(event) => void showAll(event, row)}
    >
      {#each tiles as tile (tile.key)}
        <RowTile
          {tile}
          monitor={{
            monitored: !tile.dimmed,
            busy: busy.has(tile.id),
            ontoggle: () => void toggle(tile.id, !tile.dimmed),
          }}
        />
      {/each}
    </TileRow>
  {/each}
{/if}

<h2 id="all-title" class="all-title" tabindex="-1" bind:this={allTitle}>{listHeading}</h2>

<ListControls
  {kind}
  {filters}
  ratings={imdb.shown}
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
    ratings={imdb.shown}
    {selected}
    {busy}
    onselect={select}
    onmonitor={(row) => void toggle(row.id, row.monitored)}
  />
{/if}

<div bind:this={sentinel} class="sentinel">
  {#if list.loading}
    <Loading label={`${heading} werden geladen …`} />
  {:else if list.total !== null && !list.done}
    <button type="button" class="btn" onclick={() => void list.more()}>Weitere laden</button>
  {/if}
</div>

<ImdbAttribution info={imdb.info} />

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
    align-items: center;
    gap: var(--base-size-8) var(--base-size-16);
  }

  .all-title {
    margin: 0;
    scroll-margin-top: var(--base-size-16);
  }

  .all-title:focus {
    outline: none;
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
    border-radius: var(--borderRadius-large);
    background: var(--bgColor-surface);
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
