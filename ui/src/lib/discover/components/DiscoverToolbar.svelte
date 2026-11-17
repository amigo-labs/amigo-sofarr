<script lang="ts">
  import filterIcon from '@primer/octicons/build/svg/filter-16.svg?raw';
  import xIcon from '@primer/octicons/build/svg/x-16.svg?raw';
  import { tick } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import type { GenreChoice } from '../../api/generated/GenreChoice';
  import { discoverChips, type DiscoverFilters } from '../filters';
  import FilterForm from './DiscoverFilters.svelte';
  import SearchForm from './SearchForm.svelte';

  // The search row of the discover start page with the filters in a panel behind the filter
  // button and the active filters as chips (spec ui-discover-polish AC-01, AC-02, AC-05).
  type Props = {
    filters: DiscoverFilters;
    genres: readonly GenreChoice[];
    ratings: boolean;
    onchange: (next: DiscoverFilters) => void;
  };
  let { filters, genres, ratings, onchange }: Props = $props();

  const chips = $derived(discoverChips(filters, genres, ratings));
  let open = $state(false);
  let filterButton: HTMLButtonElement | undefined = $state();
  let chipList: HTMLUListElement | undefined = $state();
  /** Where the focus goes once the chips changed: a chip's index, or -1 for the button. */
  let focusAfter: number | null = null;

  async function focusLater(wanted: number, count: number): Promise<void> {
    await tick();
    const buttons = chipList?.querySelectorAll<HTMLButtonElement>('button.chip') ?? [];
    if (wanted >= 0 && count > 0) buttons[Math.min(wanted, count - 1)]?.focus();
    else filterButton?.focus();
  }

  $effect(() => {
    const count = chips.length;
    if (focusAfter === null) return;
    const wanted = focusAfter;
    focusAfter = null;
    void focusLater(wanted, count);
  });

  function removeChip(index: number): void {
    const chip = chips[index];
    if (chip === undefined) return;
    focusAfter = chips.length > 1 ? index : -1;
    onchange(chip.remove(filters));
  }
</script>

<div class="search-row">
  <SearchForm>
    <button
      bind:this={filterButton}
      type="button"
      class="am-btn filter-button"
      class:active={chips.length > 0}
      aria-expanded={open}
      aria-controls="entdecken-filter"
      onclick={() => (open = !open)}
    >
      <Icon svg={filterIcon} />{chips.length > 0 ? `Filter (${chips.length})` : 'Filter'}
    </button>
  </SearchForm>
</div>

<div id="entdecken-filter" class="panel" hidden={!open}>
  <FilterForm {filters} {genres} {ratings} {onchange} />
</div>

{#if chips.length > 0}
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
{/if}

<style>
  .search-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .search-row :global(.search) {
    flex: 1 1 24rem;
    max-width: 44rem;
  }

  .filter-button {
    white-space: nowrap;
  }

  .filter-button.active {
    border-color: var(--borderColor-accent-emphasis);
    color: var(--fgColor-accent);
  }

  .panel {
    padding: var(--base-size-12) var(--base-size-16);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-large);
    background: var(--bgColor-surface);
  }

  .panel[hidden] {
    display: none;
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

  @media (max-width: 767.98px) {
    .search-row :global(.search) {
      flex-basis: 100%;
    }

    .panel :global(.filters) {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
