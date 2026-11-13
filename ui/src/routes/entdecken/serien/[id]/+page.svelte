<script lang="ts">
  import { page } from '$app/state';
  import PageError from '#lib/components/PageError.svelte';
  import PreviewPage from '#lib/discover/pages/PreviewPage.svelte';
  import { DISCOVER_PATH } from '#lib/discover/links.ts';
  import { idOf } from '#lib/library/detail/detail.svelte.ts';

  const id = $derived(idOf(page.params['id']));
</script>

{#if id === null}
  <PageError
    title="Serie nicht gefunden"
    message="Diese Adresse nennt keine TMDB-ID."
    action="Zu Entdecken"
    onaction={() => (location.href = DISCOVER_PATH)}
  />
{:else}
  {#key id}<PreviewPage kind="series" {id} />{/key}
{/if}
