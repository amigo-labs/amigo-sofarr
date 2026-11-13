<script lang="ts">
  import SubtitlesView from './SubtitlesView.svelte';

  // The subtitles of a movie's file on the movie page (spec subtitles AC-12).
  type Props = { movieId: number; title: string; hasFile: boolean };
  let { movieId, title, hasFile }: Props = $props();
</script>

<section class="box" aria-labelledby="subtitles-title">
  <h2 id="subtitles-title" class="box-header">Untertitel</h2>
  <div class="box-row body">
    {#if hasFile}
      {#key movieId}<SubtitlesView target={{ kind: 'movie', movieId }} {title} />{/key}
    {:else}
      <p class="muted">Ohne Datei gibt es keine Untertitel.</p>
    {/if}
  </div>
</section>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
  }

  p {
    margin: 0;
  }
</style>
