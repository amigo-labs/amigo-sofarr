<script lang="ts">
  import { page } from '$app/state';
  import PageError from '#lib/components/PageError.svelte';
  import ShelfPage from '#lib/discover/pages/ShelfPage.svelte';
  import { DISCOVER_PATH, shelfOf } from '#lib/discover/links.ts';

  const shelf = $derived(shelfOf(page.params['slug']));
</script>

{#if shelf === null}
  <PageError
    title="Regal nicht gefunden"
    message="Diese Adresse nennt kein Regal."
    action="Zu Entdecken"
    onaction={() => (location.href = DISCOVER_PATH)}
  />
{:else}
  {#key shelf.id}<ShelfPage id={shelf.id} title={shelf.title} />{/key}
{/if}
