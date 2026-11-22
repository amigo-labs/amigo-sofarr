<script lang="ts">
  import { onMount } from 'svelte';
  import Flash from '#lib/components/Flash.svelte';
  import PageHeader from '#lib/components/PageHeader.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import PageError from '#lib/components/PageError.svelte';
  import type { OverviewAnswer } from '#lib/api/generated/OverviewAnswer.ts';
  import { getOverview } from '#lib/api/library.ts';
  import alertIcon from '@primer/octicons/build/svg/alert-16.svg?raw';
  import calendarIcon from '@primer/octicons/build/svg/calendar-16.svg?raw';
  import checkIcon from '@primer/octicons/build/svg/check-16.svg?raw';
  import downloadIcon from '@primer/octicons/build/svg/download-16.svg?raw';
  import Icon from '#lib/components/Icon.svelte';
  import { dateText, dayHeading, localToday } from '#lib/library/format.ts';
  import EmptyLine from '#lib/library/overview/EmptyLine.svelte';
  import CalendarEntryLine from '#lib/library/components/CalendarEntryLine.svelte';
  import ActivityBox from '#lib/library/overview/ActivityBox.svelte';
  import QueueBox from '#lib/library/overview/QueueBox.svelte';
  import { createResource } from '#lib/resource.svelte.ts';

  // The overview (spec web-ui-library AC-01): queue, activity, problems, upcoming dates;
  // refreshes every 15 seconds (spec web-ui-shell). Key figures on top and two columns from
  // 1200 px (spec ui-overview-pages AC-02 .. AC-04). Empty boxes are hidden (spec ui-polish
  // AC-08).
  const overview = createResource(getOverview);

  onMount(() => overview.start());

  // Recomputed with every refresh, so „Heute“ moves on after midnight (AC-05).
  const today = $derived.by(() => {
    void overview.data;
    return localToday();
  });

  // A box shows only while it has something to say; its key figure says „0“ otherwise (spec
  // ui-polish AC-08). The queue's notices (no client, client unreachable) count as content.
  function shownBoxes(data: OverviewAnswer) {
    return {
      queue:
        !data.queue.client_configured || data.queue.notice !== null || data.queue.items.length > 0,
      upcoming: data.upcoming.length > 0,
      problems: data.problems.length > 0,
      activity: data.activity.length > 0,
    };
  }

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

{#snippet figure(
  linked: boolean,
  target: string,
  count: number,
  icon: string,
  label: string,
  danger: boolean,
)}
  {#snippet content()}
    <strong>{count}</strong><span class="figure-label"><Icon svg={icon} />{label}</span>
  {/snippet}
  {#if linked}
    <a class="figure ui-link" class:danger href="#{target}">{@render content()}</a>
  {:else}
    <span class="figure" class:danger>{@render content()}</span>
  {/if}
{/snippet}

<PageHeader title="Übersicht">
  {#snippet status()}
    {#if overview.slow && overview.data !== null}
      <span class="muted" role="status">Wird aktualisiert …</span>
    {/if}
  {/snippet}
</PageHeader>

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
  {@const data = overview.data}
  {@const problems = data.problems.length}
  {@const shown = shownBoxes(data)}
  {@const left = shown.queue || shown.upcoming}
  {@const right = shown.problems || shown.activity}
  <ul class="figures" aria-label="Kennzahlen">
    <li>
      {@render figure(
        shown.queue,
        'queue-title',
        data.queue.items.length,
        downloadIcon,
        'Warteschlange',
        false,
      )}
    </li>
    <li>
      {@render figure(
        shown.problems,
        'problems-title',
        problems,
        alertIcon,
        'Probleme (7 Tage)',
        problems > 0,
      )}
    </li>
    <li>
      {@render figure(
        shown.upcoming,
        'upcoming-title',
        data.upcoming.length,
        calendarIcon,
        'Nächste 7 Tage',
        false,
      )}
    </li>
  </ul>
  {#if !left && !right}
    <EmptyLine icon={checkIcon} text="Alles ruhig – keine Downloads, Probleme oder Termine." />
  {/if}
  <div class="columns" class:single={!left || !right}>
    {#if left}
      <div class="column">
        {#if shown.queue}
          <QueueBox queue={data.queue} onchanged={() => void overview.reload()} />
        {/if}
        {#if shown.upcoming}
          <section class="box" aria-labelledby="upcoming-title">
            <h2 id="upcoming-title" class="box-header">Die nächsten 7 Tage</h2>
            <ul class="days">
              {#each upcomingDays as [day, entries] (day)}
                <li class="box-row day-group">
                  <h3>
                    <time datetime={day} title={dateText(day)}>{dayHeading(day, today)}</time>
                  </h3>
                  <ul class="entries">
                    {#each entries as entry, i (i)}
                      <li><CalendarEntryLine {entry} quietUpcoming /></li>
                    {/each}
                  </ul>
                </li>
              {/each}
            </ul>
          </section>
        {/if}
      </div>
    {/if}
    {#if right}
      <div class="column">
        <ActivityBox
          id="problems-title"
          title="Probleme der letzten 7 Tage"
          entries={data.problems}
        />
        <ActivityBox id="activity-title" title="Letzte Aktivität" entries={data.activity} />
      </div>
    {/if}
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
  .figures {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--base-size-12);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .figure {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--base-size-12) var(--base-size-16);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-large);
    background: var(--bgColor-surface);
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }

  .figure .figure-label {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-4);
  }

  .figure strong {
    color: var(--fgColor-default);
    font-variant-numeric: tabular-nums;
    font-size: var(--text-title-size-large);
    line-height: 1.2;
  }

  .figure.danger {
    border-color: var(--borderColor-danger-muted);
    background: var(--bgColor-danger-muted);
    color: var(--fgColor-danger);
  }

  .figure.danger strong {
    color: var(--fgColor-danger);
  }

  .columns {
    display: grid;
    gap: var(--base-size-16);
    align-items: start;
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-16);
    min-width: 0;
  }

  @media (min-width: 1200px) {
    .columns:not(.single) {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 543.98px) {
    .figures {
      grid-template-columns: minmax(0, 1fr);
    }
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
