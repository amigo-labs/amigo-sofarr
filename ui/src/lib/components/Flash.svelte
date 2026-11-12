<script lang="ts">
  import type { Snippet } from 'svelte';

  // A notice in Primer's "flash" look. Errors are announced at once (role alert).
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
    border-radius: var(--borderRadius-medium);
    color: var(--fgColor-default);
  }

  .flash :global(p) {
    margin: 0;
    flex: 1 1 16rem;
  }

  .flash-danger {
    background: var(--bgColor-danger-muted);
    border-color: var(--borderColor-danger-muted);
  }

  .flash-warning {
    background: var(--bgColor-attention-muted);
    border-color: var(--borderColor-attention-muted);
  }

  .flash-info {
    background: var(--bgColor-accent-muted);
    border-color: var(--borderColor-accent-muted);
  }
</style>
