<script lang="ts">
  import { onMount } from 'svelte';
  import type { ApiFailure } from '../../api/http';
  import type { EpisodeDetail } from '../../api/generated/EpisodeDetail';
  import { getEpisode } from '../../api/library';
  import FileBox from './FileBox.svelte';

  // The file of one episode with quality, size, release group and media info, read when
  // the user opens it (spec web-ui-library AC-06: what the movie page shows, per episode).
  type Props = { seriesId: number; episodeId: number };
  let { seriesId, episodeId }: Props = $props();

  let detail = $state<EpisodeDetail | null>(null);
  let failure = $state<ApiFailure | null>(null);

  onMount(() => {
    void (async () => {
      const result = await getEpisode(seriesId, episodeId);
      if (result.ok) detail = result.data;
      else if (result.failure.kind !== 'unauthorized') failure = result.failure;
    })();
  });
</script>

{#if detail !== null}
  <FileBox file={detail.file} cutoffReason={detail.cutoff_reason} />
{:else if failure !== null}
  <p>Die Datei ist nicht verfügbar: {failure.message}</p>
{:else}
  <p class="muted">Wird geladen …</p>
{/if}
