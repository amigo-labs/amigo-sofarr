<script lang="ts">
  import type { Snippet } from 'svelte';
  import chevronIcon from '@primer/octicons/build/svg/chevron-right-16.svg?raw';
  import Icon from '../../components/Icon.svelte';
  import EpisodeSubtitles from '../../subtitles/EpisodeSubtitles.svelte';
  import EpisodeFile from './EpisodeFile.svelte';

  // The file and subtitles of an episode with a file: two quiet toggles, each opening its
  // content below in the row's full width; both can be open (spec ui-consistency AC-11).
  // From 768 px the toggles sit in the episode's line before its `actions`, below that on a
  // line of their own (spec ui-polish AC-06). All parts are items of the line's flex box.
  type Props = {
    seriesId: number;
    episodeId: number;
    title: string;
    /** The episode's buttons, shown after the toggles. */
    actions: Snippet;
  };
  let { seriesId, episodeId, title, actions }: Props = $props();

  let fileOpen = $state(false);
  let subtitlesOpen = $state(false);
  const id = $derived(`episode-${episodeId}`);
</script>

<div class="toggles">
  <button
    type="button"
    class="toggle"
    aria-expanded={fileOpen}
    aria-controls="{id}-file"
    onclick={() => (fileOpen = !fileOpen)}
  >
    <span class="chevron" class:open={fileOpen}><Icon svg={chevronIcon} /></span>Datei<span
      class="sr-only"
    >
      von {title}</span
    >
  </button>
  <button
    type="button"
    class="toggle"
    aria-expanded={subtitlesOpen}
    aria-controls="{id}-subtitles"
    onclick={() => (subtitlesOpen = !subtitlesOpen)}
  >
    <span class="chevron" class:open={subtitlesOpen}><Icon svg={chevronIcon} /></span
    >Untertitel<span class="sr-only"> von {title}</span>
  </button>
</div>
{@render actions()}
{#if fileOpen}
  <div class="disclosure-body" id="{id}-file">
    <EpisodeFile {seriesId} {episodeId} />
  </div>
{/if}
{#if subtitlesOpen}
  <div class="disclosure-body" id="{id}-subtitles">
    <EpisodeSubtitles {seriesId} {episodeId} {title} />
  </div>
{/if}

<style>
  .toggles {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    gap: var(--base-size-4) var(--base-size-12);
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-4);
    min-height: 1.5rem;
    padding: 0;
    border: 0;
    background: none;
    color: var(--fgColor-muted);
    font: inherit;
    font-size: var(--text-body-size-small);
    cursor: pointer;
  }

  .toggle:hover,
  .toggle:focus-visible {
    color: var(--fgColor-accent);
  }

  .chevron {
    display: inline-flex;
    transition: transform 0.15s ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .disclosure-body {
    flex: 1 1 100%;
    order: 2;
    margin-top: var(--base-size-4);
    padding-left: var(--base-size-12);
    border-left: var(--borderWidth-thick) solid var(--borderColor-muted);
    font-size: var(--text-body-size-small);
  }

  /* Touch targets of at least 32 px on narrow windows (ui-detail-boxes AC-07). */
  @media (max-width: 767.98px) {
    .toggles {
      flex: 1 1 100%;
      order: 1;
      gap: var(--base-size-4) var(--base-size-16);
    }

    .toggle {
      min-width: 2rem;
      min-height: 2rem;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .chevron {
      transition: none;
    }
  }
</style>
