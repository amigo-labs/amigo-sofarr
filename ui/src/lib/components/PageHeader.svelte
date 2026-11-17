<script lang="ts">
  import type { Snippet } from 'svelte';

  // The header of a page (spec ui-overview-pages AC-01): the heading, then its count and a
  // status where the page has them, and its main actions at the end of the row; the actions
  // wrap below the heading on narrow windows.
  type Props = { title: string; count?: Snippet; status?: Snippet; actions?: Snippet };
  let { title, count, status, actions }: Props = $props();
</script>

<div class="page-header">
  <h1>{title}</h1>
  {#if count}<span class="count">{@render count()}</span>{/if}
  {#if status}{@render status()}{/if}
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</div>

<style>
  .page-header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8) var(--base-size-16);
  }

  .count {
    color: var(--fgColor-muted);
  }

  .count :global(p) {
    margin: 0;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-8);
    margin-left: auto;
  }

  @media (max-width: 767.98px) {
    .actions {
      flex-basis: 100%;
      margin-left: 0;
    }
  }
</style>
