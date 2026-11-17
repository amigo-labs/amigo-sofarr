<script lang="ts">
  import SubtitlesView from './SubtitlesView.svelte';

  // The subtitles of an episode's file, read when the user opens them (spec subtitles
  // AC-12).
  type Props = { seriesId: number; episodeId: number; title: string };
  let { seriesId, episodeId, title }: Props = $props();
</script>

<div class="episode-subtitles">
  <SubtitlesView target={{ kind: 'episode', seriesId, episodeId }} {title} />
</div>

<style>
  .episode-subtitles {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
  }
</style>
