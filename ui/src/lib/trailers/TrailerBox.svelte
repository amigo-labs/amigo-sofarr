<script lang="ts">
  import { onMount } from 'svelte';
  import downloadIcon from '@primer/octicons/build/svg/download-16.svg?raw';
  import Icon from '../components/Icon.svelte';
  import Loading from '../components/Loading.svelte';
  import type { MediaKind } from '../api/generated/MediaKind';
  import { notify } from '../library/notices.svelte';
  import { createResource } from '../resource.svelte';
  import StatusLabel from '../system/StatusLabel.svelte';
  import { dateTimeText } from '../system/format';
  import { STATUS_TEXT, downloadResultText, downloadTrailer, getTrailer } from './trailers';

  // The trailer of a movie or series: whether there is one, its language and source, or why
  // there is none, and the manual download (spec trailers AC-09, AC-10, AC-15).
  type Props = { kind: MediaKind; id: number; title: string };
  let { kind, id, title }: Props = $props();

  /** At most four attempts of the daily run (AC-05). */
  const MAX_ATTEMPTS = 4;

  const trailer = createResource(() => getTrailer(kind, id));
  let busy = $state(false);

  onMount(() => trailer.start());

  async function download(): Promise<void> {
    busy = true;
    const result = await downloadTrailer(kind, id);
    busy = false;
    if (result.ok) {
      const loaded = result.data.status === 'downloaded';
      notify(loaded ? 'info' : 'warning', downloadResultText(title, result.data));
    } else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `Trailer nicht geladen: ${result.failure.message}`);
    }
    await trailer.reload();
  }
</script>

<section class="box" aria-labelledby="trailer-title" data-testid="trailer">
  <h2 id="trailer-title" class="box-header">Trailer</h2>
  <div class="box-row body">
    {#if trailer.data !== null}
      {@const t = trailer.data}
      <p class="state">
        <StatusLabel status={STATUS_TEXT[t.status]} />
        {#if t.message !== null}<span data-testid="trailer-message">{t.message}</span>{/if}
      </p>
      {#if t.language !== null || t.link !== null || t.files.length > 0 || t.attempts > 0 || t.next_attempt !== null}
        <dl class="facts">
          {#if t.language !== null}
            <div>
              <dt>Sprache</dt>
              <dd data-testid="trailer-language">{t.language}</dd>
            </div>
          {/if}
          {#if t.link !== null}
            <div>
              <dt>Quelle</dt>
              <dd>
                <a href={t.link} rel="noopener noreferrer" target="_blank">Trailer auf YouTube</a>
              </dd>
            </div>
          {/if}
          {#each t.files as file (file)}
            <div>
              <dt>Datei</dt>
              <dd class="mono">{file}</dd>
            </div>
          {/each}
          {#if t.attempts > 0}
            <div>
              <dt>Versuche</dt>
              <dd>{t.attempts} von {MAX_ATTEMPTS}</dd>
            </div>
          {/if}
          {#if t.next_attempt !== null}
            <div>
              <dt>Nächster Versuch</dt>
              <dd>{dateTimeText(t.next_attempt)}</dd>
            </div>
          {/if}
        </dl>
      {/if}
      <div class="actions">
        <button
          type="button"
          class="btn"
          disabled={busy || t.blocked !== null}
          aria-describedby={t.blocked === null ? undefined : 'trailer-blocked'}
          onclick={() => void download()}
        >
          <Icon svg={downloadIcon} />{busy
            ? 'Trailer wird heruntergeladen …'
            : t.status === 'downloaded'
              ? 'Trailer neu laden'
              : 'Trailer laden'}
        </button>
        {#if t.blocked !== null}
          <p id="trailer-blocked" class="muted">{t.blocked}</p>
        {/if}
      </div>
    {:else if trailer.failure !== null}
      <p>Der Trailer-Status ist nicht verfügbar: {trailer.failure.message}</p>
      <button type="button" class="btn" onclick={() => void trailer.reload()}>
        Erneut versuchen
      </button>
    {:else}
      <Loading label="Trailer-Status wird geladen …" />
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

  .state {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8) var(--base-size-12);
  }

  .actions p {
    flex: 1 1 14rem;
    overflow-wrap: anywhere;
  }
</style>
