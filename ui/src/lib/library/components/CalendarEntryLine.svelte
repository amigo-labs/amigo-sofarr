<script lang="ts">
  import type { CalendarEntry } from '../../api/generated/CalendarEntry';
  import { mediaHref } from '../actions';
  import { ENTRY_STATE, RELEASE_TEXT, episodeCode } from '../format';
  import Label from './Label.svelte';

  // One release date: a movie's cinema, digital or disc date, or an episode's air date (AC-07).
  type Props = { entry: CalendarEntry };
  let { entry }: Props = $props();

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

<span class="entry" data-state={entry.state} data-testid="calendar-entry">
  {#if href !== null}<a {href}>{entry.title}</a>{:else}<span>{entry.title}</span>{/if}
  <span class="detail">{what}</span>
  <Label text={state.text} tone={state.tone} />
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
</style>
