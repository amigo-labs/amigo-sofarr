<script lang="ts">
  import type { GenreChoice } from '../../api/generated/GenreChoice';
  import type { TitleKind } from '../../api/generated/TitleKind';
  import { ratingText } from '../../imdb/imdb';
  import {
    FIRST_YEAR,
    LANGUAGES,
    LAST_YEAR,
    MIN_RATINGS,
    NO_FILTERS,
    isFiltered,
    type DiscoverFilters,
  } from '../filters';

  // The filters of the discover page (spec discover AC-02); the rating filter only while
  // ratings are on.
  type Props = {
    filters: DiscoverFilters;
    genres: readonly GenreChoice[];
    ratings: boolean;
    onchange: (next: DiscoverFilters) => void;
  };
  let { filters, genres, ratings, onchange }: Props = $props();

  const shownGenres = $derived(
    genres.filter((g) => filters.kind === null || (filters.kind === 'movie' ? g.movie : g.series)),
  );

  const numberOf = (text: string): number | null => (text === '' ? null : Number(text));

  function year(text: string): number | null {
    if (!/^\d{4}$/.test(text)) return null;
    const value = Number(text);
    return value >= FIRST_YEAR && value <= LAST_YEAR ? value : null;
  }

  function kind(text: string): void {
    const next: TitleKind | null = text === 'movie' || text === 'series' ? text : null;
    const genre = genres.find((g) => g.id === filters.genre);
    const fits =
      genre === undefined || next === null || (next === 'movie' ? genre.movie : genre.series);
    onchange({ ...filters, kind: next, genre: fits ? filters.genre : null });
  }
</script>

<form class="filters" aria-label="Filter" onsubmit={(e) => e.preventDefault()}>
  <label class="form-field">
    Art
    <select value={filters.kind ?? ''} onchange={(e) => kind(e.currentTarget.value)}>
      <option value="">Filme und Serien</option>
      <option value="movie">Filme</option>
      <option value="series">Serien</option>
    </select>
  </label>
  <label class="form-field">
    Genre
    <select
      value={filters.genre === null ? '' : String(filters.genre)}
      onchange={(e) => onchange({ ...filters, genre: numberOf(e.currentTarget.value) })}
    >
      <option value="">Alle Genres</option>
      {#each shownGenres as genre (genre.id)}
        <option value={String(genre.id)}>{genre.name}</option>
      {/each}
    </select>
  </label>
  <label class="form-field">
    Jahr von
    <input
      type="number"
      inputmode="numeric"
      min={FIRST_YEAR}
      max={LAST_YEAR}
      value={filters.yearFrom ?? ''}
      onchange={(e) => onchange({ ...filters, yearFrom: year(e.currentTarget.value) })}
    />
  </label>
  <label class="form-field">
    Jahr bis
    <input
      type="number"
      inputmode="numeric"
      min={FIRST_YEAR}
      max={LAST_YEAR}
      value={filters.yearTo ?? ''}
      onchange={(e) => onchange({ ...filters, yearTo: year(e.currentTarget.value) })}
    />
  </label>
  <label class="form-field">
    Originalsprache
    <select
      value={filters.language ?? ''}
      onchange={(e) =>
        onchange({
          ...filters,
          language: e.currentTarget.value === '' ? null : e.currentTarget.value,
        })}
    >
      <option value="">Alle Sprachen</option>
      {#each LANGUAGES as language (language.code)}
        <option value={language.code}>{language.name}</option>
      {/each}
    </select>
  </label>
  {#if ratings}
    <label class="form-field">
      IMDb-Bewertung
      <select
        value={filters.minRating === null ? '' : String(filters.minRating)}
        onchange={(e) => onchange({ ...filters, minRating: numberOf(e.currentTarget.value) })}
      >
        <option value="">Alle Bewertungen</option>
        {#each MIN_RATINGS as rating (rating)}
          <option value={String(rating)}>ab {ratingText(rating)}</option>
        {/each}
      </select>
    </label>
  {/if}
  {#if isFiltered(filters)}
    <button type="button" class="btn reset" onclick={() => onchange(NO_FILTERS)}>
      Filter zurücksetzen
    </button>
  {/if}
</form>

<style>
  .filters {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 10rem), 1fr));
    gap: var(--base-size-8) var(--base-size-12);
    align-items: end;
  }

  .form-field input {
    width: 100%;
    box-sizing: border-box;
  }

  .reset {
    justify-self: start;
  }
</style>
