<script lang="ts">
  import type { ApiFailure } from '../../api/http';
  import type { EpisodeDetail } from '../../api/generated/EpisodeDetail';
  import { getEpisode } from '../../api/library';
  import FileBox from './FileBox.svelte';

  // The file of one episode with quality, size, release group and media info, read when
  // the user opens it (spec web-ui-library AC-06: what the movie page shows, per episode).
  type Props = { seriesId: number; episodeId: number; title: string };
  let { seriesId, episodeId, title }: Props = $props();

  let open = $state(false);
  let detail = $state<EpisodeDetail | null>(null);
  let failure = $state<ApiFailure | null>(null);

  async function load(): Promise<void> {
    const result = await getEpisode(seriesId, episodeId);
    if (result.ok) {
      detail = result.data;
      failure = null;
    } else if (result.failure.kind !== 'unauthorized') {
      failure = result.failure;
    }
  }

  $effect(() => {
    if (open && detail === null && failure === null) void load();
  });
</script>

<details class="episode-file" bind:open>
  <summary>Datei<span class="sr-only"> von {title}</span></summary>
  {#if open}
    <div class="body">
      {#if detail !== null}
        <FileBox file={detail.file} cutoffReason={detail.cutoff_reason} />
      {:else if failure !== null}
        <p>Die Datei ist nicht verfügbar: {failure.message}</p>
      {:else}
        <p class="muted">Wird geladen …</p>
      {/if}
    </div>
  {/if}
</details>

<style>
  .episode-file {
    flex: 1 1 100%;
    font-size: var(--text-body-size-small);
  }

  summary {
    width: fit-content;
    color: var(--fgColor-accent);
    cursor: pointer;
  }

  .body {
    margin-top: var(--base-size-8);
    padding-left: var(--base-size-12);
    border-left: var(--borderWidth-thick, 2px) solid var(--borderColor-muted);
  }
</style>
