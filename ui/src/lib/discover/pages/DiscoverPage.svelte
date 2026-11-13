<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import ImdbAttribution from '../../imdb/ImdbAttribution.svelte';
  import { notify } from '../../library/notices.svelte';
  import type { GenreChoice } from '../../api/generated/GenreChoice';
  import type { ShelvesAnswer } from '../../api/generated/ShelvesAnswer';
  import type { ApiFailure } from '../../api/http';
  import type { TitleCard } from '../../api/generated/TitleCard';
  import { addedText, type AddTarget } from '../add';
  import { getGenres, getList, getShelves } from '../api';
  import { effective, filterParams, isFiltered, keeps, listQuery, parseFilters } from '../filters';
  import type { DiscoverFilters } from '../filters';
  import { DISCOVER_PATH, shelfHref } from '../links';
  import { createTitlePages } from '../pages.svelte';
  import { createTitleStates } from '../states.svelte';
  import { discover, ensureStatus } from '../status.svelte';
  import AddDialog from '../components/AddDialog.svelte';
  import FilterForm from '../components/DiscoverFilters.svelte';
  import FreshnessNote from '../components/FreshnessNote.svelte';
  import SearchForm from '../components/SearchForm.svelte';
  import StatusGate from '../components/StatusGate.svelte';
  import TitleList from '../components/TitleList.svelte';

  // The discover page (spec discover AC-01, AC-02, AC-13..AC-15): the shelves, or with a
  // filter set one result list that loads while scrolling.

  /** The result list loads on until at least this many titles are visible (AC-02). */
  const VISIBLE = 20;

  const states = createTitleStates();
  const list = createTitlePages();
  let shelves: ShelvesAnswer | null = $state(null);
  let shelvesFailure: ApiFailure | null = $state(null);
  let genres: readonly GenreChoice[] = $state([]);
  let adding: AddTarget | null = $state(null);
  let sentinel: HTMLElement | undefined = $state();

  const status = $derived(discover.status);
  const ratings = $derived(status?.ratings === true);
  const filters = $derived(effective(parseFilters(page.url.searchParams), ratings));
  const filtered = $derived(isFiltered(filters));
  const visible = $derived(list.items.filter((c) => keeps(states.of(c).rating, filters.minRating)));

  async function loadShelves(): Promise<void> {
    shelvesFailure = null;
    const result = await getShelves();
    if (result.ok) shelves = result.data;
    else if (result.failure.kind !== 'unauthorized') shelvesFailure = result.failure;
  }

  onMount(() => {
    const stop = states.start();
    void (async () => {
      await ensureStatus();
      if (discover.status?.available !== true) return;
      const answer = await getGenres();
      if (answer.ok) genres = answer.data.genres;
    })();
    return stop;
  });

  // The shelves, or the result list of the filters in the page address.
  $effect(() => {
    if (status?.available !== true) return;
    const current = filters;
    untrack(() => {
      if (isFiltered(current)) void list.reset((n) => getList(listQuery(current, n)));
      else if (shelves === null) void loadShelves();
    });
  });

  $effect(() => {
    const shown: TitleCard[] = filtered
      ? [...list.items]
      : (shelves?.shelves.flatMap((s) => s.items) ?? []);
    untrack(() => states.show(shown));
  });

  // Titles whose rating turns out too low leave the list: read on until enough are visible.
  $effect(() => {
    if (filtered && !list.loading && list.more && list.failure === null) {
      if (visible.length < VISIBLE) void list.next();
    }
  });

  $effect(() => {
    // Svelte sets a bound element to null once it leaves the page.
    if (!(sentinel instanceof HTMLElement)) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) void list.next();
      },
      { rootMargin: '400px' },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  function apply(next: DiscoverFilters): void {
    const params = filterParams(next).toString();
    void goto(params === '' ? DISCOVER_PATH : `?${params}`, {
      replace: true,
      reset: false,
    });
  }

  function onadd(card: TitleCard): void {
    adding = { kind: card.kind, tmdbId: card.tmdb_id, title: card.title, seasons: null };
  }
</script>

<svelte:head>
  <title>Entdecken – sofarr</title>
</svelte:head>

<StatusGate title="Entdecken" home={false}>
  <h1>Entdecken</h1>
  <SearchForm />
  <FilterForm {filters} {genres} {ratings} onchange={apply} />

  {#if filtered}
    <section aria-labelledby="results-title" class="section">
      <h2 id="results-title">Ergebnisse</h2>
      <FreshnessNote freshness={list.freshness} />
      {#if list.failure !== null}
        <Flash tone="danger">
          <p>Die Ergebnisse konnten nicht geladen werden: {list.failure.message}</p>
          <button
            type="button"
            class="btn"
            onclick={() => void list.reset((n) => getList(listQuery(filters, n)))}
            >Erneut versuchen</button
          >
        </Flash>
      {/if}
      {#if visible.length > 0}
        <TitleList cards={visible} stateOf={states.of} canAdd={true} {onadd} label="Ergebnisse" />
      {:else if !list.loading && list.freshness !== null && !list.more}
        <p class="muted">Keine Titel passen zu diesen Filtern.</p>
      {/if}
      <div bind:this={sentinel} class="sentinel">
        {#if list.loading}
          <Loading label="Titel werden geladen …" />
        {:else if list.more && list.failure === null}
          <button type="button" class="btn" onclick={() => void list.next()}>Weitere laden</button>
        {/if}
      </div>
    </section>
  {:else if shelves !== null}
    <FreshnessNote freshness={shelves.freshness} />
    {#each shelves.shelves as shelf (shelf.id)}
      <section class="section" aria-labelledby="shelf-{shelf.id}" data-testid="shelf">
        <div class="shelf-head">
          <h2 id="shelf-{shelf.id}">{shelf.title}</h2>
          {#if shelf.has_more}
            <a href={shelfHref(shelf.id)}
              >Mehr<span class="sr-only">{` aus „${shelf.title}“`}</span></a
            >
          {/if}
        </div>
        {#if shelf.error !== null}
          <p class="muted">{shelf.error}</p>
        {:else if shelf.items.length === 0}
          <p class="muted">Hier gibt es gerade keine Titel.</p>
        {:else}
          <TitleList
            cards={shelf.items}
            stateOf={states.of}
            canAdd={true}
            {onadd}
            row={true}
            label={shelf.title}
          />
        {/if}
      </section>
    {/each}
  {:else if shelvesFailure !== null}
    <Flash tone="danger">
      <p>Die Regale konnten nicht geladen werden: {shelvesFailure.message}</p>
      <button type="button" class="btn" onclick={() => void loadShelves()}>Erneut versuchen</button>
    </Flash>
  {:else}
    <Loading label="Regale werden geladen …" />
  {/if}
  <ImdbAttribution info={discover.imdb} />
</StatusGate>

<AddDialog
  target={adding}
  onclose={() => (adding = null)}
  onadded={(target, answer) => {
    states.added(target.kind, target.tmdbId, answer.id);
    notify('info', addedText(answer));
    adding = null;
  }}
/>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    min-width: 0;
  }

  .shelf-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--base-size-8);
  }

  h2 {
    margin: 0;
  }

  .sentinel {
    display: flex;
    justify-content: center;
    min-height: 2rem;
  }
</style>
