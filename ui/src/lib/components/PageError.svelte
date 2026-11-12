<script lang="ts">
  import alertIcon from '@primer/octicons/build/svg/alert-24.svg?raw';
  import Icon from './Icon.svelte';

  // A full-page error with one action, for pages that have no data to show.
  type Props = {
    title: string;
    message: string;
    hint?: string;
    action: string;
    onaction: () => void;
  };
  let { title, message, hint, action, onaction }: Props = $props();
</script>

<section class="page-error" role="alert" aria-labelledby="page-error-title">
  <Icon svg={alertIcon} />
  <h1 id="page-error-title">{title}</h1>
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
