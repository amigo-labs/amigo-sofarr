<script lang="ts">
  import { page } from '$app/state';
  import PageError from '#lib/components/PageError.svelte';
</script>

<svelte:head>
  <title>{page.status === 404 ? 'Seite nicht gefunden' : 'Fehler'} – sofarr</title>
</svelte:head>

{#if page.error?.asset}
  <PageError
    title="Die Oberfläche konnte nicht geladen werden"
    message={page.error.message}
    hint="Neu laden holt die aktuelle Version der Oberfläche."
    action="Neu laden"
    onaction={() => location.reload()}
  />
{:else if page.status === 404}
  <section class="not-found">
    <h1>Seite nicht gefunden</h1>
    <p>Unter der Adresse <code>{page.url.pathname}</code> gibt es keine Seite.</p>
    <p><a href="/">Zur Startseite</a></p>
  </section>
{:else}
  <PageError
    title="Fehler"
    message={page.error?.message ?? 'Unerwarteter Fehler in der Oberfläche.'}
    action="Neu laden"
    onaction={() => location.reload()}
  />
{/if}

<style>
  .not-found {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
  }

  p {
    margin: 0;
  }
</style>
