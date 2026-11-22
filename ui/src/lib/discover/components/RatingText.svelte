<script lang="ts">
  import type { Rating } from '../../api/generated/Rating';
  import { ratingCompact, ratingLine } from '../texts';

  // A title's IMDb rating and votes (spec imdb-ratings AC-04): a placeholder while it is
  // still loading (an empty line on the tiles), "–" without one, nothing while ratings are off. Compact on the title
  // tiles, with the votes in tooltip and name, as the library rows (ui-consistency AC-03).
  type Props = { rating: Rating; compact?: boolean };
  let { rating, compact = false }: Props = $props();

  const text = $derived(ratingLine(rating));
  const short = $derived(compact ? ratingCompact(rating) : null);
</script>

{#if short !== null}
  <span
    class="rating"
    class:pending={rating.state === 'pending'}
    data-testid="rating"
    role="img"
    aria-label={short.full}
    title={short.full}>{short.text}</span
  >
{:else if !compact && text !== null}
  <span class="rating" class:pending={rating.state === 'pending'} data-testid="rating"
    >IMDb {text}{#if rating.state === 'pending'}<span class="sr-only">{' (Bewertung folgt)'}</span
      >{/if}</span
  >
{/if}

<style>
  .rating {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
    white-space: nowrap;
  }

  /* A pending rating keeps its line, so the tile does not jump (ui-polish AC-02). */
  .rating[role='img'] {
    display: block;
    width: fit-content;
    min-width: 1.5em;
    min-height: 1lh;
  }
</style>
