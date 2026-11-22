<script lang="ts">
  import Flash from '../../components/Flash.svelte';
  import type { Freshness } from '../../api/generated/Freshness';
  import { fetchedText } from '../texts';

  // When TMDB's answer was fetched (spec discover AC-13), and the German notice with its
  // age while TMDB cannot be reached (AC-14). The line stands once at the end of a page, the
  // notice where the answer is shown (spec ui-polish AC-10): `part` picks one of the two.
  type Props = {
    freshness: Freshness | null;
    notice?: boolean;
    part?: 'both' | 'notice' | 'line';
  };
  let { freshness, notice = true, part = 'both' }: Props = $props();
</script>

{#if freshness !== null}
  {#if part !== 'line' && notice && freshness.stale && freshness.notice !== null}
    <Flash tone="warning"><p data-testid="stale-notice">{freshness.notice}</p></Flash>
  {/if}
  {#if part !== 'notice'}
    <p class="fetched muted" data-testid="fetched">{fetchedText(freshness)}</p>
  {/if}
{/if}

<style>
  .fetched {
    margin: 0;
    font-size: var(--text-body-size-small);
  }
</style>
