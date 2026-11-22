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
    WEEKDAYS,
    dayTitle,
    daysOf,
    isDay,
    monthGrid,
    rangeOf,
    rangeTitle,
    step,
    today,
    type Span,
  } from '#lib/library/calendar.ts';
  import CalendarEntryLine from '#lib/library/components/CalendarEntryLine.svelte';
  import { REFRESH_MS } from '#lib/resource.svelte.ts';

  // The calendar (spec web-ui-library AC-07): a week or a month of release and air dates,
  // starting at the current week; span and date are part of the page address. The month is a
  // grid of weeks, the week seven columns, both an agenda on narrow windows (spec ui-calendar).
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

  const entriesOf = (d: string) => (answer?.entries ?? []).filter((e) => e.date === d);
  const weekDays = $derived(daysOf(range).map((d) => ({ day: d, entries: entriesOf(d) })));
  const monthDays = $derived(
    monthGrid(day)
      .flat()
      .map((cell) => ({
        day: cell.day,
        outside: cell.outside,
        entries: cell.outside ? [] : entriesOf(cell.day),
      })),
  );
  const now = today();

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
  <div class="am-seg" role="group" aria-label="Zeitraum">
    <button type="button" aria-pressed={span === 'woche'} onclick={() => show('woche', day)}>
      Woche
    </button>
    <button type="button" aria-pressed={span === 'monat'} onclick={() => show('monat', day)}>
      Monat
    </button>
  </div>
  <div class="group">
    <button
      type="button"
      class="btn icon-only"
      aria-label={span === 'woche' ? 'Vorherige Woche' : 'Vorheriger Monat'}
      onclick={() => show(span, step(span, day, -1))}
    >
      <Icon svg={prevIcon} />
    </button>
    <button type="button" class="btn" onclick={() => show(span, today())}>Heute</button>
    <button
      type="button"
      class="btn icon-only"
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
  {#if span === 'monat'}
    <div class="month">
      <div class="weekdays" aria-hidden="true">
        {#each WEEKDAYS as weekday (weekday)}<span>{weekday}</span>{/each}
      </div>
      <ol class="days">
        {#each monthDays as d (d.day)}
          <li
            class="cell"
            class:today={d.day === now}
            class:empty={d.entries.length === 0}
            data-day={d.day}
            data-outside={d.outside ? '' : undefined}
          >
            <h3 aria-label={`${dayTitle(d.day)}${d.day === now ? ' · heute' : ''}`}>
              <span class="number" aria-hidden="true">{Number(d.day.slice(8))}</span>
              <span class="full" aria-hidden="true"
                >{dayTitle(d.day)}{#if d.day === now}&nbsp;· heute{/if}</span
              >
            </h3>
            {#if d.entries.length > 0}
              <ul class="entries">
                {#each d.entries as entry, i (i)}
                  <li><CalendarEntryLine {entry} dot cut /></li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ol>
    </div>
  {:else}
    <ol class="days week">
      {#each weekDays as d (d.day)}
        <li class="column" class:today={d.day === now} data-day={d.day}>
          <h3>
            {dayTitle(d.day)}{#if d.day === now}&nbsp;· heute{/if}
          </h3>
          {#if d.entries.length === 0}
            <p class="muted nothing">Nichts.</p>
          {:else}
            <ul class="entries">
              {#each d.entries as entry, i (i)}
                <li><CalendarEntryLine {entry} dot /></li>
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

  .icon-only {
    width: var(--control-medium-size);
    padding: 0;
  }

  .range {
    font-size: var(--text-title-size-small);
  }

  .note {
    margin: 0;
    color: var(--fgColor-muted);
  }

  .days,
  .entries {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  h3 {
    margin: 0;
    font-size: var(--text-body-size-small);
    font-weight: var(--base-text-weight-semibold);
  }

  .entries {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    min-width: 0;
  }

  /* Month grid. */
  .month {
    overflow: hidden;
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-large);
    background: var(--bgColor-surface);
  }

  .weekdays,
  .month .days {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
  }

  .weekdays span {
    padding: var(--base-size-8) var(--base-size-12);
    border-bottom: var(--borderWidth-thin) solid var(--borderColor-default);
    color: var(--fgColor-caption);
    font-size: 12px;
    font-weight: var(--base-text-weight-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .cell {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    min-width: 0;
    /* A week without entries stays low; one with entries grows with its fullest day (spec
       ui-polish AC-12). */
    min-height: 3.5rem;
    padding: var(--base-size-8);
    border-right: var(--borderWidth-thin) solid var(--borderColor-default);
    border-bottom: var(--borderWidth-thin) solid var(--borderColor-default);
  }

  .cell:nth-child(7n) {
    border-right: 0;
  }

  /* Outside days: their numbers in the muted text colour, which keeps AA contrast. */
  .cell[data-outside] {
    background: var(--bgColor-default);
    color: var(--fgColor-caption);
  }

  .cell[data-outside] h3 {
    font-weight: var(--base-text-weight-normal);
  }

  .cell .full {
    display: none;
  }

  .cell.today .number {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 1.6rem;
    height: 1.6rem;
    border-radius: var(--borderRadius-full);
    background: var(--am-accent-solid);
    color: var(--am-on-accent);
  }

  /* Week columns. */
  .week {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: var(--base-size-8);
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    min-width: 0;
    padding: var(--base-size-12);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-large);
    background: var(--bgColor-surface);
  }

  .column.today {
    border-color: var(--borderColor-accent-emphasis);
  }

  .column.today h3 {
    color: var(--fgColor-accent);
  }

  .nothing {
    margin: 0;
    font-size: var(--text-body-size-small);
  }

  /* Agenda on narrow windows. */
  @media (max-width: 767.98px) {
    .month {
      border: 0;
      background: none;
    }

    .weekdays {
      display: none;
    }

    .month .days,
    .week {
      grid-template-columns: minmax(0, 1fr);
      gap: var(--base-size-8);
    }

    .cell[data-outside],
    .cell.empty {
      display: none;
    }

    .cell {
      min-height: 0;
      padding: var(--base-size-12);
      border: var(--borderWidth-thin) solid var(--borderColor-default);
      border-radius: var(--borderRadius-large);
      background: var(--bgColor-surface);
    }

    .cell.today {
      border-color: var(--borderColor-accent-emphasis);
    }

    .cell .number {
      display: none;
    }

    .cell .full {
      display: inline;
    }

    .cell.today .full {
      color: var(--fgColor-accent);
    }
  }
</style>
