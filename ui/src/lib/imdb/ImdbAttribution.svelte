<!--
  IMDb's attribution and the date of the ratings data (spec imdb-ratings AC-08): shown on
  every page that shows ratings, nothing while ratings are off or not yet known.
-->
<script lang="ts">
  import type { ImdbInfo } from '../api/generated/ImdbInfo';
  import { dataDateText } from './imdb';

  type Props = { info: ImdbInfo | null };
  let { info }: Props = $props();

  const date = $derived(info === null ? null : dataDateText(info));
</script>

{#if info !== null && info.enabled}
  <p class="imdb-attribution muted">
    <span>Bewertungen:</span>
    <a href={info.link} rel="noopener noreferrer" target="_blank">{info.attribution}</a>
    {#if date !== null}<span>· {date}</span>{/if}
  </p>
{/if}

<style>
  .imdb-attribution {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
    margin: var(--base-size-16) 0 0;
    font-size: var(--text-body-size-small);
    overflow-wrap: anywhere;
  }
</style>
