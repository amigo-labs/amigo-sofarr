<script lang="ts">
  import type { FileDetail } from '../../api/generated/FileDetail';
  import { valueText } from '../../system/config-view';
  import { bytesText } from '../format';

  // A library file with quality, size, release group and media info (AC-05).
  type Props = { file: FileDetail | null; cutoffReason: string | null; title?: string };
  let { file, cutoffReason, title = 'Datei' }: Props = $props();

  const mediaInfo = $derived(
    file?.media_info === null || file?.media_info === undefined
      ? []
      : Object.entries(file.media_info).map(([key, value]) => ({ key, value: valueText(value) })),
  );
</script>

<section class="box" aria-labelledby="file-title">
  <h2 id="file-title" class="box-header">{title}</h2>
  <div class="box-row body">
    {#if file === null}
      <p><strong>fehlt</strong> – es gibt noch keine Datei.</p>
    {:else}
      <dl class="facts">
        <div>
          <dt>Datei</dt>
          <dd class="mono" data-testid="file-path">{file.relative_path}</dd>
        </div>
        <div>
          <dt>Qualität</dt>
          <dd>{file.quality}</dd>
        </div>
        <div>
          <dt>Größe</dt>
          <dd>{bytesText(file.size)}</dd>
        </div>
        <div>
          <dt>Release-Gruppe</dt>
          <dd>{file.release_group ?? '–'}</dd>
        </div>
        <div>
          <dt>Custom-Format-Score</dt>
          <dd>{file.custom_format_score}</dd>
        </div>
      </dl>
      {#if cutoffReason !== null}
        <p>Cutoff nicht erreicht: {cutoffReason}</p>
      {/if}
      {#if mediaInfo.length > 0}
        <h3>Media-Infos</h3>
        <dl class="facts">
          {#each mediaInfo as row (row.key)}
            <div>
              <dt class="mono">{row.key}</dt>
              <dd>{row.value}</dd>
            </div>
          {/each}
        </dl>
      {:else}
        <p class="muted">Media-Infos wurden noch nicht gelesen.</p>
      {/if}
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
