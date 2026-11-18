<script lang="ts">
  import type { Snippet } from 'svelte';

  // A notice: its words in the status' ink on the status' subtle fill (amigo-theme AC-03);
  // an info notice is neutral (ui-consistency AC-02).
  // Errors are announced at once (role alert).
  type Props = { tone: 'danger' | 'warning' | 'info'; children: Snippet };
  let { tone, children }: Props = $props();
</script>

<div class="flash flash-{tone}" role={tone === 'danger' ? 'alert' : 'status'}>
  {@render children()}
</div>

<style>
  .flash {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8) var(--base-size-16);
    padding: var(--base-size-12) var(--base-size-16);
    border: var(--borderWidth-thin) solid;
    border-radius: var(--borderRadius-large);
  }

  .flash :global(p) {
    margin: 0;
    flex: 1 1 16rem;
  }

  .flash-danger {
    background: var(--bgColor-danger-muted);
    color: var(--fgColor-danger);
    border-color: var(--borderColor-danger-muted);
  }

  .flash-warning {
    background: var(--bgColor-attention-muted);
    color: var(--fgColor-attention);
    border-color: var(--borderColor-attention-muted);
  }

  /* Neutral, so a restriction does not look like a success (spec ui-consistency AC-02). */
  .flash-info {
    background: var(--bgColor-muted);
    color: var(--fgColor-default);
    border-color: var(--borderColor-default);
  }
</style>
