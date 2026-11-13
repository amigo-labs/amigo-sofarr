<script lang="ts">
  import Flash from '../../components/Flash.svelte';
  import type { Freshness } from '../../api/generated/Freshness';
  import { fetchedText } from '../texts';

  // When TMDB's answer was fetched (spec discover AC-13), and the German notice with its
  // age while TMDB cannot be reached (AC-14).
  type Props = { freshness: Freshness | null; notice?: boolean };
  let { freshness, notice = true }: Props = $props();
</script>

{#if freshness !== null}
  {#if notice && freshness.stale && freshness.notice !== null}
    <Flash tone="warning"><p data-testid="stale-notice">{freshness.notice}</p></Flash>
  {/if}
  <p class="fetched muted" data-testid="fetched">{fetchedText(freshness)}</p>
{/if}

<style>
  .fetched {
    margin: 0;
    font-size: var(--text-body-size-small);
  }
</style>
