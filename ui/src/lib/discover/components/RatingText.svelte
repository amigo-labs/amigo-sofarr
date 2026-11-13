<script lang="ts">
  import type { Rating } from '../../api/generated/Rating';
  import { ratingLine } from '../texts';

  // A title's IMDb rating and votes (spec imdb-ratings AC-04): a placeholder while it is
  // still loading, "–" without one, nothing while ratings are off.
  type Props = { rating: Rating };
  let { rating }: Props = $props();

  const text = $derived(ratingLine(rating));
</script>

{#if text !== null}
  <span class="rating" class:pending={rating.state === 'pending'} data-testid="rating"
    >IMDb {text}{#if rating.state === 'pending'}<span class="sr-only">{' (Bewertung folgt)'}</span
      >{/if}</span
  >
{/if}

<style>
  .rating {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }
</style>
