<script lang="ts">
  import { LOADING_DELAY_MS } from '../resource.svelte';

  // A loading indicator that appears only when loading takes longer than a moment.
  type Props = { label?: string };
  let { label = 'Wird geladen …' }: Props = $props();
  let visible = $state(false);

  $effect(() => {
    const timer = setTimeout(() => (visible = true), LOADING_DELAY_MS);
    return () => clearTimeout(timer);
  });
</script>

<div class="loading" role="status" aria-live="polite">
  {#if visible}
    <span class="spinner" aria-hidden="true"></span>
    <span>{label}</span>
  {/if}
</div>

<style>
  .loading {
    display: flex;
    align-items: center;
    gap: var(--base-size-8);
    min-height: 2rem;
    color: var(--fgColor-muted);
  }

  .spinner {
    width: 1rem;
    height: 1rem;
    border: 2px solid var(--borderColor-default);
    border-top-color: var(--fgColor-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 2.4s;
    }
  }
</style>
