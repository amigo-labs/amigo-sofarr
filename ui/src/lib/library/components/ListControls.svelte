<script lang="ts">
  import listIcon from '@primer/octicons/build/svg/list-unordered-16.svg?raw';
  import gridIcon from '@primer/octicons/build/svg/apps-16.svg?raw';
  import Icon from '../../components/Icon.svelte';
  import type { Choice } from '../../api/generated/Choice';
  import type { Order } from '../../api/generated/Order';
  import type { Sort } from '../../api/generated/Sort';
  import type { Tri } from '../../api/generated/Tri';
  import { ratingText } from '../../imdb/imdb';
  import {
    SORTS,
    TRI_FILTERS,
    sortsFor,
    type ListFilters,
    type ListKind,
    type TriKey,
  } from '../list-query';
  import type { ListView } from '../view';

  // Search, filters, sorting and the view switch of a library list (AC-02, AC-03, AC-06);
  // rating sort and minimums only while IMDb ratings are on (spec imdb-ratings AC-06,
  // AC-07, AC-09).
  type Props = {
    kind: ListKind;
    filters: ListFilters;
    ratings: boolean;
    search: string;
    profiles: readonly Choice[];
    tags: readonly Choice[];
    view: ListView;
    onsearch: (text: string) => void;
    onchange: (filters: ListFilters) => void;
    onview: (view: ListView) => void;
  };
  let { kind, filters, ratings, search, profiles, tags, view, onsearch, onchange, onview }: Props =
    $props();

  const RATINGS: readonly number[] = [5, 6, 6.5, 7, 7.5, 8, 8.5, 9];
  const VOTES: readonly number[] = [100, 1000, 10_000, 100_000];
  const VOTES_TEXT = new Intl.NumberFormat('de-DE');

  /** The offered choices plus the one from the page address, sorted. */
  const choices = (offered: readonly number[], current: number | null): readonly number[] =>
    current === null || offered.includes(current)
      ? offered
      : [...offered, current].toSorted((a, b) => a - b);
  const numberOf = (value: string): number | null => (value === '' ? null : Number(value));

  const setTri = (key: TriKey, value: string): void => {
    const tri: Tri = value === 'yes' || value === 'no' ? value : 'any';
    onchange({ ...filters, tris: { ...filters.tris, [key]: tri } });
  };
  const idOf = (value: string): number | null => (value === '' ? null : Number(value));
  const sortOf = (value: string): Sort => SORTS.find((s) => s.value === value)?.value ?? 'title';
  const sorts = $derived(sortsFor(ratings));
  const orderOf = (value: string): Order => (value === 'desc' ? 'desc' : 'asc');
</script>

<div class="controls">
  <div class="field search">
    <label for="liste-suche">Suche</label>
    <input
      id="liste-suche"
      type="search"
      value={search}
      placeholder={kind === 'movie' ? 'Titel, Original- oder Alternativtitel' : 'Titel'}
      oninput={(e) => onsearch(e.currentTarget.value)}
    />
  </div>
  {#each TRI_FILTERS[kind] as filter (filter.key)}
    <div class="field">
      <label for={`liste-${filter.key}`}>{filter.label}</label>
      <select
        id={`liste-${filter.key}`}
        value={filters.tris[filter.key] ?? 'any'}
        onchange={(e) => setTri(filter.key, e.currentTarget.value)}
      >
        <option value="any">Alle</option>
        <option value="yes">Ja</option>
        <option value="no">Nein</option>
      </select>
    </div>
  {/each}
  <div class="field">
    <label for="liste-profil">Qualitätsprofil</label>
    <select
      id="liste-profil"
      value={filters.profile === null ? '' : String(filters.profile)}
      onchange={(e) => onchange({ ...filters, profile: idOf(e.currentTarget.value) })}
    >
      <option value="">Alle</option>
      {#each profiles as profile (profile.id)}
        <option value={String(profile.id)}>{profile.name}</option>
      {/each}
    </select>
  </div>
  <div class="field">
    <label for="liste-tag">Tag</label>
    <select
      id="liste-tag"
      value={filters.tag === null ? '' : String(filters.tag)}
      onchange={(e) => onchange({ ...filters, tag: idOf(e.currentTarget.value) })}
    >
      <option value="">Alle</option>
      {#each tags as tag (tag.id)}
        <option value={String(tag.id)}>{tag.name}</option>
      {/each}
    </select>
  </div>
  {#if ratings}
    <div class="field">
      <label for="liste-mindestbewertung">Mindestbewertung</label>
      <select
        id="liste-mindestbewertung"
        value={filters.minRating === null ? '' : String(filters.minRating)}
        onchange={(e) => onchange({ ...filters, minRating: numberOf(e.currentTarget.value) })}
      >
        <option value="">Alle</option>
        {#each choices(RATINGS, filters.minRating) as rating (rating)}
          <option value={String(rating)}>ab {ratingText(rating)}</option>
        {/each}
      </select>
    </div>
    <div class="field">
      <label for="liste-mindeststimmen">Mindeststimmen</label>
      <select
        id="liste-mindeststimmen"
        value={filters.minVotes === null ? '' : String(filters.minVotes)}
        onchange={(e) => onchange({ ...filters, minVotes: numberOf(e.currentTarget.value) })}
      >
        <option value="">Alle</option>
        {#each choices(VOTES, filters.minVotes) as votes (votes)}
          <option value={String(votes)}>ab {VOTES_TEXT.format(votes)}</option>
        {/each}
      </select>
    </div>
  {/if}
  <div class="field">
    <label for="liste-sortierung">Sortierung</label>
    <select
      id="liste-sortierung"
      value={filters.sort}
      onchange={(e) => onchange({ ...filters, sort: sortOf(e.currentTarget.value) })}
    >
      {#each sorts as sort (sort.value)}
        <option value={sort.value}>{sort.label}</option>
      {/each}
    </select>
  </div>
  <div class="field">
    <label for="liste-richtung">Reihenfolge</label>
    <select
      id="liste-richtung"
      value={filters.order}
      onchange={(e) => onchange({ ...filters, order: orderOf(e.currentTarget.value) })}
    >
      <option value="asc">aufsteigend</option>
      <option value="desc">absteigend</option>
    </select>
  </div>
  <div class="view" role="group" aria-label="Ansicht">
    <button
      type="button"
      class="btn"
      aria-pressed={view === 'liste'}
      onclick={() => onview('liste')}
    >
      <Icon svg={listIcon} />Liste
    </button>
    <button
      type="button"
      class="btn"
      aria-pressed={view === 'poster'}
      onclick={() => onview('poster')}
    >
      <Icon svg={gridIcon} />Poster
    </button>
  </div>
</div>

<style>
  .controls {
    container-type: inline-size;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr));
    gap: var(--base-size-8) var(--base-size-12);
    align-items: end;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    min-width: 0;
    font-size: var(--text-body-size-small);
    font-weight: var(--base-text-weight-semibold);
  }

  .search {
    grid-column: 1 / -1;
  }

  input,
  select {
    min-height: 2rem;
    min-width: 0;
    padding: 0 var(--base-size-8);
    border: var(--borderWidth-thin) solid var(--control-borderColor-emphasis);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-default);
    color: var(--fgColor-default);
    font: inherit;
    font-weight: var(--base-text-weight-normal);
    font-size: var(--text-body-size-medium);
  }

  .view {
    display: flex;
    gap: var(--base-size-4);
  }

  /* Two buttons with icons need more than one column; with a single column (below 2 x
     9.5rem plus the gap) the group keeps its one cell, which is then wide enough. */
  @container (min-width: 20rem) {
    .view {
      grid-column: span 2;
    }
  }

  .view button {
    white-space: nowrap;
  }

  .view button[aria-pressed='true'] {
    border-color: var(--borderColor-accent-emphasis);
    box-shadow: inset 0 0 0 1px var(--borderColor-accent-emphasis);
  }
</style>
