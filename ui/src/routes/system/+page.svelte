<script lang="ts">
  import { onMount } from 'svelte';
  import Flash from '#lib/components/Flash.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import PageError from '#lib/components/PageError.svelte';
  import type { SystemAnswer } from '#lib/api/generated/SystemAnswer.ts';
  import { api } from '#lib/auth/session.svelte.ts';
  import { createResource } from '#lib/resource.svelte.ts';
  import SystemView from '#lib/system/SystemView.svelte';

  // The system page (spec web-ui-shell AC-12, AC-13): refreshes while it is open.
  const system = createResource(() => api.read<SystemAnswer>('/api/system'));

  onMount(() => system.start());
</script>

<svelte:head>
  <title>System – sofarr</title>
</svelte:head>

{#if system.data !== null}
  <div class="page-head">
    <h1>System</h1>
    {#if system.slow}
      <span class="muted" role="status">Wird aktualisiert …</span>
    {/if}
  </div>
  {#if system.failure !== null}
    <Flash tone="danger">
      <p>
        Aktualisieren fehlgeschlagen: {system.failure.message} Angezeigt wird der letzte Stand; sofarr
        versucht es gleich von selbst erneut.
      </p>
      <button type="button" class="btn" onclick={() => void system.reload()}
        >Erneut versuchen</button
      >
    </Flash>
  {/if}
  <SystemView system={system.data} />
{:else if system.failure !== null}
  <PageError
    title="Der Systemstatus ist nicht verfügbar"
    message={system.failure.message}
    hint="sofarr versucht es gleich von selbst erneut."
    action="Erneut versuchen"
    onaction={() => void system.reload()}
  />
{:else}
  <h1>System</h1>
  <Loading label="Systemstatus wird geladen …" />
{/if}

<style>
  .page-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--base-size-8) var(--base-size-16);
  }
</style>
