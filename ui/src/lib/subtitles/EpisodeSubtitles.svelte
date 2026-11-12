<script lang="ts">
  import SubtitlesView from './SubtitlesView.svelte';

  // The subtitles of an episode's file, read when the user opens them (spec subtitles
  // AC-12).
  type Props = { seriesId: number; episodeId: number; title: string };
  let { seriesId, episodeId, title }: Props = $props();

  let open = $state(false);
</script>

<details class="episode-subtitles" bind:open>
  <summary>Untertitel<span class="sr-only"> von {title}</span></summary>
  {#if open}
    <div class="body">
      <SubtitlesView target={{ kind: 'episode', seriesId, episodeId }} {title} />
    </div>
  {/if}
</details>

<style>
  .episode-subtitles {
    flex: 1 1 100%;
    font-size: var(--text-body-size-small);
  }

  summary {
    width: fit-content;
    color: var(--fgColor-accent);
    cursor: pointer;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    margin-top: var(--base-size-8);
    padding-left: var(--base-size-12);
    border-left: var(--borderWidth-thick, 2px) solid var(--borderColor-muted);
  }
</style>
