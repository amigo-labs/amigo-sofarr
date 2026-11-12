<script lang="ts">
  import { onMount } from 'svelte';
  import Flash from '#lib/components/Flash.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import PageError from '#lib/components/PageError.svelte';
  import type { SubtitleItem } from '#lib/api/generated/SubtitleItem.ts';
  import { notify } from '#lib/library/notices.svelte.ts';
  import { createResource } from '#lib/resource.svelte.ts';
  import NeededList from '#lib/subtitles/NeededList.svelte';
  import RecentList from '#lib/subtitles/RecentList.svelte';
  import {
    getSubtitleOverview,
    searchReportText,
    searchSubtitles,
    targetKey,
    targetOf,
  } from '#lib/subtitles/subtitles.ts';
  import { dateTimeText } from '#lib/system/format.ts';

  // The subtitle overview: needed subtitles with their schedule, the ones found in the last
  // 7 days, and a search per file at once (spec subtitles AC-11, AC-13, AC-15).
  const overview = createResource(getSubtitleOverview);
  let busy = $state(new Set<string>());

  onMount(() => overview.start());

  async function search(item: SubtitleItem): Promise<void> {
    const target = targetOf(item);
    if (target === null) return;
    const key = targetKey(item);
    busy = new Set([...busy, key]);
    const result = await searchSubtitles(target);
    busy = new Set([...busy].filter((k) => k !== key));
    if (result.ok) notify('info', searchReportText(item.title, result.data));
    else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `Untertitelsuche für „${item.title}“: ${result.failure.message}`);
    }
    await overview.reload();
  }
</script>

<svelte:head>
  <title>Untertitel – sofarr</title>
</svelte:head>

{#if overview.data !== null}
  {@const o = overview.data}
  <h1>Untertitel</h1>
  {#if o.reason !== null}
    <Flash tone="warning">
      <p data-testid="subtitles-off">Keine Untertitelsuche: {o.reason}</p>
    </Flash>
  {/if}
  {#if o.shadow}
    <Flash tone="info">
      <p>
        Schattenmodus: sofarr sucht Untertitel, lädt aber keine; „würde laden“ nennt, was es geladen
        hätte.
      </p>
    </Flash>
  {/if}
  {#if o.quota_until !== null}
    <Flash tone="warning">
      <p data-testid="subtitles-quota">
        Das Download-Kontingent von OpenSubtitles ist aufgebraucht; sofarr lädt ab
        {dateTimeText(o.quota_until)} weiter.
      </p>
    </Flash>
  {/if}
  {#if overview.failure !== null}
    <Flash tone="danger">
      <p>
        Aktualisieren fehlgeschlagen: {overview.failure.message} Angezeigt wird der letzte Stand.
      </p>
    </Flash>
  {/if}

  <section class="box" aria-labelledby="needed-title" data-testid="needed">
    <h2 id="needed-title" class="box-header">
      Benötigte Untertitel <span class="counter">{o.needed.length}</span>
    </h2>
    <NeededList needed={o.needed} enabled={o.enabled} {busy} onsearch={(i) => void search(i)} />
  </section>

  <section class="box" aria-labelledby="recent-title" data-testid="recent">
    <h2 id="recent-title" class="box-header">
      Gefunden in den letzten 7 Tagen <span class="counter">{o.recent.length}</span>
    </h2>
    <RecentList recent={o.recent} />
  </section>
{:else if overview.failure !== null}
  <PageError
    title="Die Untertitel sind nicht verfügbar"
    message={overview.failure.message}
    action="Erneut versuchen"
    onaction={() => void overview.reload()}
  />
{:else}
  <h1>Untertitel</h1>
  <Loading label="Untertitel werden geladen …" />
{/if}

<style>
  .counter {
    margin-left: var(--base-size-4);
    padding: 0 var(--base-size-8);
    border-radius: 2em;
    background: var(--bgColor-neutral-muted);
    font-size: var(--text-body-size-small);
    font-weight: var(--base-text-weight-normal);
  }
</style>
