<script lang="ts">
  import PageHeader from '#lib/components/PageHeader.svelte';
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
  <PageHeader title="System">
    {#snippet status()}
      {#if system.slow}
        <span class="muted" role="status">Wird aktualisiert …</span>
      {/if}
    {/snippet}
  </PageHeader>
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
  <PageHeader title="System" />
  <PageError
    sub
    title="Der Systemstatus ist nicht verfügbar"
    message={system.failure.message}
    hint="sofarr versucht es gleich von selbst erneut."
    action="Erneut versuchen"
    onaction={() => void system.reload()}
  />
{:else}
  <PageHeader title="System" />
  <Loading label="Systemstatus wird geladen …" />
{/if}
