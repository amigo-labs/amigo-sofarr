<script lang="ts">
  import downloadIcon from '@primer/octicons/build/svg/download-16.svg?raw';
  import Flash from '../../components/Flash.svelte';
  import type { QueueAnswer } from '../../api/generated/QueueAnswer';
  import EmptyLine from './EmptyLine.svelte';
  import { mediaHref, runMarkFailed } from '../actions';
  import { bytesText, durationText, percentText, statusText } from '../format';

  // The queue with progress, size and remaining time (spec web-ui-library AC-01). Without a
  // download client one muted line instead of a warning; an unreachable client's notice
  // already names the time of the state shown (ui-overview-polish AC-02, AC-03).
  type Props = { queue: QueueAnswer; onchanged: () => void };
  let { queue, onchanged }: Props = $props();

  let busy = $state<number | null>(null);

  async function fail(kind: 'movie' | 'series', grabId: number, title: string): Promise<void> {
    busy = grabId;
    if (await runMarkFailed(kind, grabId, title)) onchanged();
    busy = null;
  }
</script>

<section class="box" aria-labelledby="queue-title">
  <h2 id="queue-title" class="box-header">Warteschlange</h2>
  {#if !queue.client_configured}
    <EmptyLine
      icon={downloadIcon}
      text="Kein Download-Client eingerichtet – sofarr lädt nichts herunter."
    />
  {:else if queue.notice !== null}
    <div class="box-row">
      <Flash tone="warning"><p>{queue.notice}</p></Flash>
    </div>
  {/if}
  {#if queue.items.length === 0}
    {#if queue.client_configured}
      <EmptyLine icon={downloadIcon} text="Nichts in der Warteschlange." />
    {/if}
  {:else}
    <ul class="rows">
      {#each queue.items as item (item.grab_id)}
        <li class="box-row" data-testid="queue-item">
          <div class="main">
            <a href={mediaHref(item.kind, item.media_id)} class="strong ui-link"
              >{item.media_title}</a
            >
            <span class="muted release">{item.release_title}</span>
            <div class="progress">
              <progress
                max="1"
                value={item.progress ?? 0}
                aria-label={`Fortschritt von „${item.media_title}“`}
              ></progress>
              <span>{percentText(item.progress)}</span>
            </div>
            <dl class="facts">
              <div>
                <dt>Status</dt>
                <dd>{statusText(item.download_state ?? item.status)}</dd>
              </div>
              <div>
                <dt>Größe</dt>
                <dd>{bytesText(item.size)}</dd>
              </div>
              <div>
                <dt>Restzeit</dt>
                <dd data-testid="remaining">{durationText(item.remaining_seconds)}</dd>
              </div>
              <div>
                <dt>Qualität</dt>
                <dd>{item.quality}</dd>
              </div>
            </dl>
            {#if item.message !== null}<p class="muted">{item.message}</p>{/if}
          </div>
          <button
            type="button"
            class="btn btn-danger"
            disabled={busy === item.grab_id}
            onclick={() => void fail(item.kind, item.grab_id, item.media_title)}
          >
            Als fehlgeschlagen markieren
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* Title and details on the left, the action at the end of the row (spec
     ui-overview-pages AC-06); stacked on narrow windows. */
  .rows li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--base-size-8) var(--base-size-16);
    align-items: start;
  }

  .main {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--base-size-8);
    min-width: 0;
  }

  @media (max-width: 767.98px) {
    .rows li {
      grid-template-columns: minmax(0, 1fr);
    }

    .rows li > button {
      justify-self: start;
    }
  }

  .strong {
    font-weight: var(--base-text-weight-semibold);
  }

  .release {
    font-size: var(--text-body-size-small);
    overflow-wrap: anywhere;
  }

  .progress {
    display: flex;
    align-items: center;
    gap: var(--base-size-8);
    width: 100%;
  }

  progress {
    flex: 1;
    min-width: 0;
    height: 0.5rem;
    accent-color: var(--fgColor-accent);
  }

  .facts {
    width: 100%;
  }

  p {
    margin: 0;
  }
</style>
