<script lang="ts">
  import type { CalendarEntry } from '../../api/generated/CalendarEntry';
  import { mediaHref } from '../actions';
  import { ENTRY_STATE, RELEASE_TEXT, episodeCode } from '../format';
  import Label from './Label.svelte';

  // One release date: a movie's cinema, digital or disc date, or an episode's air date (AC-07).
  // `dot` shows the state as a coloured dot named by its text, `cut` keeps title and detail on
  // one line each (spec ui-calendar AC-04).
  type Props = { entry: CalendarEntry; dot?: boolean; cut?: boolean };
  let { entry, dot = false, cut = false }: Props = $props();

  const href = $derived(
    entry.movie_id !== null
      ? mediaHref('movie', entry.movie_id)
      : entry.series_id !== null
        ? mediaHref('series', entry.series_id)
        : null,
  );
  const state = $derived(ENTRY_STATE[entry.state]);
  const what = $derived(
    entry.kind === 'movie'
      ? entry.release === null
        ? 'Film'
        : RELEASE_TEXT[entry.release]
      : [episodeCode(entry.season, entry.episode), entry.episode_title]
          .filter((part) => part !== null && part !== '')
          .join(' · '),
  );
</script>

<span
  class="entry"
  class:stacked={dot}
  class:cut
  data-state={entry.state}
  data-testid="calendar-entry"
>
  {#if dot}
    <span class="dot dot-{state.tone}" role="img" aria-label={state.text} title={state.text}></span>
  {/if}
  {#if href !== null}<a class="ui-link title" {href} title={cut ? entry.title : undefined}
      >{entry.title}</a
    >{:else}<span class="title" title={cut ? entry.title : undefined}>{entry.title}</span>{/if}
  <span class="detail" title={cut ? what : undefined}>{what}</span>
  {#if !dot}<Label text={state.text} tone={state.tone} />{/if}
</span>

<style>
  .entry {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-4) var(--base-size-8);
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .detail {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }

  /* Calendar form: the dot before the title, the detail below it. */
  .stacked {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 0 var(--base-size-8);
    align-items: baseline;
    width: 100%;
  }

  .stacked .detail {
    grid-column: 2;
  }

  .cut .title,
  .cut .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: var(--borderRadius-full);
    background: var(--am-border-strong);
    transform: translateY(-1px);
  }

  .dot-success {
    background: var(--am-success);
  }

  .dot-danger {
    background: var(--am-danger);
  }

  .dot-accent {
    background: var(--am-accent);
  }
</style>
