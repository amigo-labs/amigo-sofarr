<script lang="ts">
  import { page } from '$app/state';
  import PageError from '#lib/components/PageError.svelte';
  import SeriesPage from '#lib/library/detail/SeriesPage.svelte';
  import { idOf } from '#lib/library/detail/detail.svelte.ts';

  const id = $derived(idOf(page.params['id']));
</script>

{#if id === null}
  <PageError
    title="Serie nicht gefunden"
    message="Diese Adresse nennt keine Serie."
    action="Zu den Serien"
    onaction={() => (location.href = '/serien')}
  />
{:else}
  {#key id}<SeriesPage {id} />{/key}
{/if}
