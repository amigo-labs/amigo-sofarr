<script lang="ts">
  import alertIcon from '@primer/octicons/build/svg/alert-24.svg?raw';
  import Icon from './Icon.svelte';

  // A full-page error with one action, for pages that have no data to show. Below a page
  // header (spec ui-overview-pages) its title is a second-level heading (`sub`).
  type Props = {
    title: string;
    sub?: boolean;
    message: string;
    hint?: string;
    action: string;
    onaction: () => void;
  };
  let { title, sub = false, message, hint, action, onaction }: Props = $props();
</script>

<section class="page-error" role="alert" aria-labelledby="page-error-title">
  <Icon svg={alertIcon} />
  {#if sub}
    <h2 id="page-error-title">{title}</h2>
  {:else}
    <h1 id="page-error-title">{title}</h1>
  {/if}
  <p>{message}</p>
  {#if hint}<p class="muted">{hint}</p>{/if}
  <button type="button" class="btn" onclick={onaction}>{action}</button>
</section>

<style>
  .page-error {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--base-size-12);
    max-width: 36rem;
    margin: var(--base-size-48) auto;
    text-align: center;
    color: var(--fgColor-default);
  }

  .page-error :global(.icon) {
    color: var(--fgColor-danger);
  }

  p {
    margin: 0;
  }
</style>
