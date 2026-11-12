<script lang="ts">
  import { onMount } from 'svelte';
  import ImdbAttribution from './ImdbAttribution.svelte';
  import { imdb, loadImdb } from './info.svelte';
  import { ratingText, votesText } from './imdb';

  // The IMDb rating of a movie or series page with IMDb's attribution, nothing while
  // ratings are off (spec imdb-ratings AC-04, AC-08, AC-09).
  type Props = { rating: number | null; votes: number | null };
  let { rating, votes }: Props = $props();

  onMount(() => {
    void loadImdb();
  });
</script>

{#if imdb.shown}
  <div class="rating">
    <p data-testid="rating">
      <span class="muted">IMDb-Bewertung</span>
      <strong>{ratingText(rating)}</strong>
      {#if rating !== null && votes !== null}<span>({votesText(votes)})</span>{/if}
    </p>
    <ImdbAttribution info={imdb.info} />
  </div>
{/if}

<style>
  .rating p {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--base-size-4) var(--base-size-8);
    margin: 0;
  }

  .rating :global(.imdb-attribution) {
    margin-top: var(--base-size-4);
  }
</style>
