<script lang="ts">
  import listIcon from '@primer/octicons/build/svg/list-unordered-16.svg?raw';
  import gridIcon from '@primer/octicons/build/svg/apps-16.svg?raw';
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import filterIcon from '@primer/octicons/build/svg/filter-16.svg?raw';
  import ascIcon from '@primer/octicons/build/svg/sort-asc-16.svg?raw';
  import descIcon from '@primer/octicons/build/svg/sort-desc-16.svg?raw';
  import xIcon from '@primer/octicons/build/svg/x-16.svg?raw';
  import { tick } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import type { Choice } from '../../api/generated/Choice';
  import type { Order } from '../../api/generated/Order';
  import type { Sort } from '../../api/generated/Sort';
  import type { Tri } from '../../api/generated/Tri';
  import { ratingText } from '../../imdb/imdb';
  import { filterChips, withoutFilters } from '../filter-chips';
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
  // AC-07, AC-09). One toolbar row; the filters in a panel behind the filter button, the
  // active ones as chips that remove themselves (spec ui-list-toolbar AC-01 .. AC-07).
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
  const flipped = (order: Order): Order => (order === 'desc' ? 'asc' : 'desc');

  const chips = $derived(filterChips(kind, filters, ratings, profiles, tags));
  let open = $state(false);
  let filterButton: HTMLButtonElement | undefined = $state();
  let chipList: HTMLUListElement | undefined = $state();
  /** Where the focus goes once the chips changed: a chip's index, or -1 for the button. */
  let focusAfter: number | null = null;

  $effect(() => {
    const count = chips.length;
    if (focusAfter === null) return;
    const wanted = focusAfter;
    focusAfter = null;
    void focusLater(wanted, count);
  });

  async function focusLater(wanted: number, count: number): Promise<void> {
    await tick();
    const buttons = chipList?.querySelectorAll<HTMLButtonElement>('button.chip') ?? [];
    if (wanted >= 0 && count > 0) buttons[Math.min(wanted, count - 1)]?.focus();
    else filterButton?.focus();
  }

  function removeChip(index: number): void {
    const chip = chips[index];
    if (chip === undefined) return;
    focusAfter = chips.length > 1 ? index : -1;
    onchange(chip.remove(filters));
  }

  function resetFilters(): void {
    focusAfter = -1;
    onchange(withoutFilters(filters));
  }
</script>

<div class="toolbar" role="toolbar" aria-label="Liste">
  <div class="search">
    <label for="liste-suche" class="sr-only">Suche</label>
    <span class="search-icon"><Icon svg={searchIcon} /></span>
    <input
      id="liste-suche"
      class="am-input"
      type="search"
      value={search}
      placeholder={kind === 'movie' ? 'Titel, Original- oder Alternativtitel' : 'Titel'}
      oninput={(e) => onsearch(e.currentTarget.value)}
    />
  </div>
  <label for="liste-sortierung" class="sr-only">Sortierung</label>
  <select
    id="liste-sortierung"
    class="am-input sort"
    value={filters.sort}
    onchange={(e) => onchange({ ...filters, sort: sortOf(e.currentTarget.value) })}
  >
    {#each sorts as sort (sort.value)}
      <option value={sort.value}>{sort.label}</option>
    {/each}
  </select>
  <button
    type="button"
    class="am-btn icon-only"
    aria-label={filters.order === 'desc' ? 'Aufsteigend sortieren' : 'Absteigend sortieren'}
    title={filters.order === 'desc' ? 'Absteigend sortiert' : 'Aufsteigend sortiert'}
    onclick={() => onchange({ ...filters, order: flipped(filters.order) })}
  >
    <Icon svg={filters.order === 'desc' ? descIcon : ascIcon} />
  </button>
  <div class="am-seg view" role="group" aria-label="Ansicht">
    <button type="button" aria-pressed={view === 'liste'} onclick={() => onview('liste')}>
      <Icon svg={listIcon} />Liste
    </button>
    <button type="button" aria-pressed={view === 'poster'} onclick={() => onview('poster')}>
      <Icon svg={gridIcon} />Poster
    </button>
  </div>
  <button
    bind:this={filterButton}
    type="button"
    class="am-btn filter-button"
    class:active={chips.length > 0}
    aria-expanded={open}
    aria-controls="liste-filter"
    onclick={() => (open = !open)}
  >
    <Icon svg={filterIcon} />{chips.length > 0 ? `Filter (${chips.length})` : 'Filter'}
  </button>
</div>

<div id="liste-filter" class="panel" hidden={!open}>
  {#each TRI_FILTERS[kind] as filter (filter.key)}
    <div class="field">
      <label for={`liste-${filter.key}`}>{filter.label}</label>
      <select
        class="am-input"
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
      class="am-input"
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
      class="am-input"
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
        class="am-input"
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
        class="am-input"
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
</div>

{#if chips.length > 0}
  <div class="chips-row">
    <ul class="chips" aria-label="Aktive Filter" bind:this={chipList}>
      {#each chips as chip, index (chip.key)}
        <li>
          <button
            type="button"
            class="am-chip am-chip--accent chip"
            aria-label={`Filter ${chip.text} entfernen`}
            onclick={() => removeChip(index)}
          >
            {chip.text}<Icon svg={xIcon} />
          </button>
        </li>
      {/each}
    </ul>
    {#if chips.length > 1}
      <button type="button" class="am-btn am-btn--ghost am-btn--sm" onclick={resetFilters}>
        Alle Filter zurücksetzen
      </button>
    {/if}
  </div>
{/if}

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .search {
    position: relative;
    flex: 1 1 16rem;
    min-width: 0;
  }

  .search input {
    padding-left: 2rem;
  }

  .search-icon {
    position: absolute;
    top: 50%;
    left: var(--base-size-8);
    display: inline-flex;
    color: var(--fgColor-caption);
    transform: translateY(-50%);
    pointer-events: none;
  }

  .sort {
    width: auto;
  }

  .icon-only {
    width: var(--control-medium-size);
    padding: 0;
  }

  .view button {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-4);
    white-space: nowrap;
    overflow-wrap: normal;
  }

  .filter-button {
    white-space: nowrap;
  }

  .filter-button.active {
    border-color: var(--borderColor-accent-emphasis);
    color: var(--fgColor-accent);
  }

  .panel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
    gap: var(--base-size-8) var(--base-size-12);
    padding: var(--base-size-12) var(--base-size-16);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-large);
    background: var(--bgColor-surface);
  }

  .panel[hidden] {
    display: none;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    min-width: 0;
    font-size: var(--text-body-size-small);
    font-weight: var(--base-text-weight-medium);
    color: var(--fgColor-muted);
  }

  .chips-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-8);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .chip {
    border: 0;
    cursor: pointer;
  }

  .chip:hover {
    background: color-mix(in srgb, var(--am-accent-subtle) 80%, var(--am-accent));
  }

  @media (max-width: 767.98px) {
    .search {
      flex-basis: 100%;
    }

    .panel {
      grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
    }
  }
</style>
