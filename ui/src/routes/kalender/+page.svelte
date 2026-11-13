<script lang="ts">
  import prevIcon from '@primer/octicons/build/svg/chevron-left-16.svg?raw';
  import nextIcon from '@primer/octicons/build/svg/chevron-right-16.svg?raw';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Flash from '#lib/components/Flash.svelte';
  import Icon from '#lib/components/Icon.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import type { CalendarAnswer } from '#lib/api/generated/CalendarAnswer.ts';
  import type { ApiFailure } from '#lib/api/http.ts';
  import { getCalendar } from '#lib/api/library.ts';
  import {
    dayTitle,
    daysOf,
    isDay,
    rangeOf,
    rangeTitle,
    step,
    today,
    type Span,
  } from '#lib/library/calendar.ts';
  import CalendarEntryLine from '#lib/library/components/CalendarEntryLine.svelte';
  import { REFRESH_MS } from '#lib/resource.svelte.ts';

  // The calendar (spec web-ui-library AC-07): a week or a month of release and air dates,
  // starting at the current week; span and date are part of the page address.
  const span: Span = $derived(page.url.searchParams.get('ansicht') === 'monat' ? 'monat' : 'woche');
  const day = $derived.by(() => {
    const value = page.url.searchParams.get('datum');
    return isDay(value) ? value : today();
  });
  const range = $derived(rangeOf(span, day));

  let answer = $state<CalendarAnswer | null>(null);
  let failure = $state<ApiFailure | null>(null);

  $effect(() => {
    const { from, to } = range;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async (): Promise<void> => {
      const result = await getCalendar(from, to);
      if (stopped) return;
      if (result.ok) {
        answer = result.data;
        failure = null;
      } else if (result.failure.kind !== 'unauthorized') {
        failure = result.failure;
      }
      timer = setTimeout(() => void load(), REFRESH_MS);
    };
    answer = null;
    void load();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });

  const days = $derived(
    daysOf(range).map((d) => ({
      day: d,
      entries: (answer?.entries ?? []).filter((e) => e.date === d),
    })),
  );
  const shown = $derived(span === 'woche' ? days : days.filter((d) => d.entries.length > 0));

  function show(nextSpan: Span, nextDay: string): void {
    const params = new URLSearchParams();
    if (nextSpan === 'monat') params.set('ansicht', 'monat');
    if (nextDay !== today()) params.set('datum', nextDay);
    const query = params.toString();
    void goto(query === '' ? '/kalender' : `/kalender?${query}`, { reset: false });
  }
</script>

<svelte:head>
  <title>Kalender – sofarr</title>
</svelte:head>

<h1>Kalender</h1>

<div class="toolbar">
  <div class="group" role="group" aria-label="Zeitraum">
    <button
      type="button"
      class="btn"
      aria-pressed={span === 'woche'}
      onclick={() => show('woche', day)}
    >
      Woche
    </button>
    <button
      type="button"
      class="btn"
      aria-pressed={span === 'monat'}
      onclick={() => show('monat', day)}
    >
      Monat
    </button>
  </div>
  <div class="group">
    <button
      type="button"
      class="btn"
      aria-label={span === 'woche' ? 'Vorherige Woche' : 'Vorheriger Monat'}
      onclick={() => show(span, step(span, day, -1))}
    >
      <Icon svg={prevIcon} />
    </button>
    <button type="button" class="btn" onclick={() => show(span, today())}>Heute</button>
    <button
      type="button"
      class="btn"
      aria-label={span === 'woche' ? 'Nächste Woche' : 'Nächster Monat'}
      onclick={() => show(span, step(span, day, 1))}
    >
      <Icon svg={nextIcon} />
    </button>
  </div>
  <h2 class="range" aria-live="polite" data-testid="range">{rangeTitle(span, day)}</h2>
</div>

{#if failure !== null}
  <Flash tone="danger">
    <p>Der Kalender konnte nicht geladen werden: {failure.message}</p>
  </Flash>
{/if}

{#if answer === null && failure === null}
  <Loading label="Kalender wird geladen …" />
{:else if answer !== null}
  {#if answer.note !== null || answer.entries.length === 0}
    <p class="note" data-testid="calendar-note">
      {answer.note ?? 'In diesem Zeitraum erscheint nichts.'}
    </p>
  {/if}
  {#if shown.length > 0 && (span === 'woche' || answer.entries.length > 0)}
    <ol class="days">
      {#each shown as d (d.day)}
        <li class="box day" class:today={d.day === today()} data-day={d.day}>
          <h3 class="box-header">
            {dayTitle(d.day)}{#if d.day === today()}
              · heute{/if}
          </h3>
          {#if d.entries.length === 0}
            <p class="box-row muted">Nichts.</p>
          {:else}
            <ul class="entries">
              {#each d.entries as entry, i (i)}
                <li class="box-row"><CalendarEntryLine {entry} /></li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
{/if}

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8) var(--base-size-16);
  }

  .group {
    display: flex;
    gap: var(--base-size-4);
  }

  .group button[aria-pressed='true'] {
    border-color: var(--borderColor-accent-emphasis);
    box-shadow: inset 0 0 0 1px var(--borderColor-accent-emphasis);
  }

  .range {
    font-size: var(--text-title-size-small);
  }

  .note {
    margin: 0;
    color: var(--fgColor-muted);
  }

  .days {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 16rem), 1fr));
    gap: var(--base-size-12);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .day.today {
    border-color: var(--borderColor-accent-emphasis);
  }

  .entries {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  p {
    margin: 0;
  }
</style>
