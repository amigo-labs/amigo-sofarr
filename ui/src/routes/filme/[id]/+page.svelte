<script lang="ts">
  import { page } from '$app/state';
  import PageError from '#lib/components/PageError.svelte';
  import MoviePage from '#lib/library/detail/MoviePage.svelte';
  import { idOf } from '#lib/library/detail/detail.svelte.ts';

  const id = $derived(idOf(page.params['id']));
</script>

{#if id === null}
  <PageError
    title="Film nicht gefunden"
    message="Diese Adresse nennt keinen Film."
    action="Zu den Filmen"
    onaction={() => (location.href = '/filme')}
  />
{:else}
  {#key id}<MoviePage {id} />{/key}
{/if}
