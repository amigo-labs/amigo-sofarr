<script lang="ts">
  import Flash from '../../components/Flash.svelte';
  import { dismiss, notices } from '../notices.svelte';

  // Results of actions, newest first (spec web-ui-library AC-09).
</script>

{#if notices.list.length > 0}
  <div class="notices" aria-live="polite">
    {#each notices.list as notice (notice.id)}
      <Flash tone={notice.tone}>
        <p>{notice.text}</p>
        <button type="button" class="btn" onclick={() => dismiss(notice.id)}>Schließen</button>
      </Flash>
    {/each}
  </div>
{/if}

<style>
  .notices {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
  }
</style>
