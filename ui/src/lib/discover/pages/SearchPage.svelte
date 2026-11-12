<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { page } from '$app/state';
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import ImdbAttribution from '../../imdb/ImdbAttribution.svelte';
  import { notify } from '../../library/notices.svelte';
  import type { ApiFailure } from '../../api/http';
  import type { Freshness } from '../../api/generated/Freshness';
  import type { PersonCard } from '../../api/generated/PersonCard';
  import type { TitleCard } from '../../api/generated/TitleCard';
  import { addedText, type AddTarget } from '../add';
  import { searchTitles } from '../api';
  import { fresher, newCards } from '../pages.svelte';
  import { createTitleStates } from '../states.svelte';
  import { discover } from '../status.svelte';
  import { departmentText } from '../texts';
  import AddDialog from '../components/AddDialog.svelte';
  import FreshnessNote from '../components/FreshnessNote.svelte';
  import PeopleList from '../components/PeopleList.svelte';
  import SearchForm from '../components/SearchForm.svelte';
  import StatusGate from '../components/StatusGate.svelte';
  import TitleList from '../components/TitleList.svelte';

  // Search for movies, series and people in three groups (spec discover AC-03).
  const states = createTitleStates();
  const text = $derived((page.url.searchParams.get('begriff') ?? '').trim().slice(0, 200));

  let movies: TitleCard[] = $state([]);
  let series: TitleCard[] = $state([]);
  let people: PersonCard[] = $state([]);
  let freshness: Freshness | null = $state(null);
  let next: number | null = $state(null);
  let loading = $state(false);
  let failure: ApiFailure | null = $state(null);
  let adding: AddTarget | null = $state(null);
  let generation = 0;

  async function load(q: string, pageNumber: number): Promise<void> {
    const mine = pageNumber === 1 ? ++generation : generation;
    if (pageNumber === 1) {
      movies = [];
      series = [];
      people = [];
      freshness = null;
      next = null;
    }
    failure = null;
    loading = true;
    const result = await searchTitles(q, pageNumber);
    if (mine !== generation) return;
    loading = false;
    if (!result.ok) {
      if (result.failure.kind !== 'unauthorized') failure = result.failure;
      return;
    }
    const a = result.data;
    movies = [...movies, ...newCards(movies, a.movies)];
    series = [...series, ...newCards(series, a.series)];
    const known = new Set(people.map((p) => p.tmdb_id));
    people = [...people, ...a.people.filter((p) => !known.has(p.tmdb_id))];
    freshness = pageNumber === 1 ? a.freshness : fresher(freshness, a.freshness);
    next = a.next_page;
  }

  onMount(() => states.start());

  $effect(() => {
    const q = text;
    const available = discover.status?.available === true;
    untrack(() => {
      if (q !== '' && available) void load(q, 1);
    });
  });

  $effect(() => {
    states.show([...movies, ...series]);
  });

  function onadd(card: TitleCard): void {
    adding = { kind: card.kind, tmdbId: card.tmdb_id, title: card.title, seasons: null };
  }

  const roleOf = (p: PersonCard): string =>
    [p.department === null ? null : departmentText(p.department), p.known_for.join(', ')]
      .filter((t) => t !== null && t !== '')
      .join(' · ');
</script>

<svelte:head>
  <title>{text === '' ? 'Suche' : `Suche: ${text}`} – sofarr</title>
</svelte:head>

<StatusGate title="Suche">
  <h1>Suche</h1>
  <SearchForm value={text} />
  {#if text === ''}
    <p class="muted">Suchbegriff eingeben, um Filme, Serien und Personen zu finden.</p>
  {:else}
    <FreshnessNote {freshness} />
    {#if failure !== null}
      <Flash tone="danger">
        <p>Die Suche ist fehlgeschlagen: {failure.message}</p>
        <button type="button" class="btn" onclick={() => void load(text, 1)}
          >Erneut versuchen</button
        >
      </Flash>
    {/if}
    {#if freshness !== null}
      <section class="group" aria-labelledby="search-movies">
        <h2 id="search-movies">Filme</h2>
        {#if movies.length > 0}
          <TitleList cards={movies} stateOf={states.of} canAdd={true} {onadd} label="Filme" />
        {:else}
          <p class="muted">Keine Filme gefunden.</p>
        {/if}
      </section>
      <section class="group" aria-labelledby="search-series">
        <h2 id="search-series">Serien</h2>
        {#if series.length > 0}
          <TitleList cards={series} stateOf={states.of} canAdd={true} {onadd} label="Serien" />
        {:else}
          <p class="muted">Keine Serien gefunden.</p>
        {/if}
      </section>
      <section class="group" aria-labelledby="search-people">
        <h2 id="search-people">Personen</h2>
        {#if people.length > 0}
          <PeopleList
            label="Personen"
            people={people.map((p) => ({
              id: p.tmdb_id,
              name: p.name,
              role: roleOf(p),
              photo: p.photo,
            }))}
          />
        {:else}
          <p class="muted">Keine Personen gefunden.</p>
        {/if}
      </section>
    {/if}
    <div class="more">
      {#if loading}
        <Loading label="Ergebnisse werden geladen …" />
      {:else if next !== null && failure === null}
        <button type="button" class="btn" onclick={() => next !== null && void load(text, next)}
          >Weitere Ergebnisse</button
        >
      {/if}
    </div>
    <ImdbAttribution info={discover.imdb} />
  {/if}
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
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
  }

  h2 {
    margin: 0;
  }

  .more {
    display: flex;
    justify-content: center;
    min-height: 2rem;
  }
</style>
