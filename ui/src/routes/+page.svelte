<script lang="ts">
  import { onMount } from 'svelte';
  import Flash from '#lib/components/Flash.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import PageError from '#lib/components/PageError.svelte';
  import { getOverview } from '#lib/api/library.ts';
  import { dateText } from '#lib/library/format.ts';
  import CalendarEntryLine from '#lib/library/components/CalendarEntryLine.svelte';
  import ActivityBox from '#lib/library/overview/ActivityBox.svelte';
  import QueueBox from '#lib/library/overview/QueueBox.svelte';
  import { createResource } from '#lib/resource.svelte.ts';

  // The overview (spec web-ui-library AC-01): queue, activity, problems, upcoming dates;
  // refreshes every 15 seconds (spec web-ui-shell).
  const overview = createResource(getOverview);

  onMount(() => overview.start());

  const upcomingDays = $derived.by(() => {
    const days = new Map<string, NonNullable<typeof overview.data>['upcoming']>();
    for (const entry of overview.data?.upcoming ?? []) {
      days.set(entry.date, [...(days.get(entry.date) ?? []), entry]);
    }
    return [...days.entries()];
  });
</script>

<svelte:head>
  <title>Übersicht – sofarr</title>
</svelte:head>

<div class="head">
  <h1>Übersicht</h1>
  {#if overview.slow && overview.data !== null}
    <span class="muted" role="status">Wird aktualisiert …</span>
  {/if}
</div>

{#if overview.data !== null}
  {#if overview.failure !== null}
    <Flash tone="danger">
      <p>
        Aktualisieren fehlgeschlagen: {overview.failure.message} Angezeigt wird der letzte Stand.
      </p>
      <button type="button" class="btn" onclick={() => void overview.reload()}>
        Erneut versuchen
      </button>
    </Flash>
  {/if}
  <div class="columns">
    <QueueBox queue={overview.data.queue} onchanged={() => void overview.reload()} />
    <section class="box" aria-labelledby="upcoming-title">
      <h2 id="upcoming-title" class="box-header">Die nächsten 7 Tage</h2>
      {#if upcomingDays.length === 0}
        <p class="box-row muted">
          Keine Veröffentlichungen überwachter Titel in den nächsten 7 Tagen.
        </p>
      {:else}
        <ul class="days">
          {#each upcomingDays as [day, entries] (day)}
            <li class="box-row">
              <h3>{dateText(day)}</h3>
              <ul class="entries">
                {#each entries as entry, i (i)}
                  <li><CalendarEntryLine {entry} /></li>
                {/each}
              </ul>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
    <ActivityBox
      id="problems-title"
      title="Probleme der letzten 7 Tage"
      entries={overview.data.problems}
      empty="Keine fehlgeschlagenen Grabs oder Importprobleme."
    />
    <ActivityBox
      id="activity-title"
      title="Letzte Aktivität"
      entries={overview.data.activity}
      empty="Noch keine Grabs oder Importe."
    />
  </div>
{:else if overview.failure !== null}
  <PageError
    title="Die Übersicht ist nicht verfügbar"
    message={overview.failure.message}
    hint="sofarr versucht es gleich von selbst erneut."
    action="Erneut versuchen"
    onaction={() => void overview.reload()}
  />
{:else}
  <Loading label="Übersicht wird geladen …" />
{/if}

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--base-size-8) var(--base-size-16);
  }

  .columns {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 28rem), 1fr));
    gap: var(--base-size-16);
    align-items: start;
  }

  .days,
  .entries {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .entries {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    margin-top: var(--base-size-4);
  }
</style>
