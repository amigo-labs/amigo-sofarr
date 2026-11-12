<script lang="ts">
  import type { BlockedRelease } from '../../api/generated/BlockedRelease';
  import type { GrabEntry } from '../../api/generated/GrabEntry';
  import type { MediaKind } from '../../api/generated/MediaKind';
  import { dateTimeText } from '../../system/format';
  import { runMarkFailed } from '../actions';
  import { bytesText, statusText, statusTone } from '../format';
  import Label from '../components/Label.svelte';

  // The grabs of an item with state and message, and its blocklist entries (AC-05, AC-09).
  type Props = {
    kind: MediaKind;
    grabs: readonly GrabEntry[];
    blocklist: readonly BlockedRelease[];
    onchanged: () => void;
  };
  let { kind, grabs, blocklist, onchanged }: Props = $props();

  let busy: number | null = $state(null);

  async function fail(grab: GrabEntry): Promise<void> {
    busy = grab.id;
    if (await runMarkFailed(kind, grab.id, grab.title)) onchanged();
    busy = null;
  }
</script>

<section class="box" aria-labelledby="grabs-title">
  <h2 id="grabs-title" class="box-header">Grabs</h2>
  {#if grabs.length === 0}
    <p class="box-row muted">Noch keine Grabs.</p>
  {:else}
    <ul class="rows">
      {#each grabs as grab (grab.id)}
        <li class="box-row" data-testid="grab">
          <div class="line">
            <span class="title">{grab.title}</span>
            <Label text={statusText(grab.status)} tone={statusTone(grab.status)} />
          </div>
          <span class="muted small">
            {dateTimeText(grab.grabbed_at)} · {grab.indexer} · {grab.quality} · {bytesText(
              grab.size,
            )}
          </span>
          {#if grab.message !== null}<span class="message">{grab.message}</span>{/if}
          {#if grab.active}
            <button
              type="button"
              class="btn btn-danger"
              disabled={busy === grab.id}
              onclick={() => void fail(grab)}
            >
              Als fehlgeschlagen markieren
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<section class="box" aria-labelledby="blocked-title">
  <h2 id="blocked-title" class="box-header">Blocklist</h2>
  {#if blocklist.length === 0}
    <p class="box-row muted">Keine gesperrten Releases.</p>
  {:else}
    <ul class="rows">
      {#each blocklist as entry (entry.id)}
        <li class="box-row">
          <span class="title">{entry.title}</span>
          <span class="muted small">
            {dateTimeText(entry.blocklisted_at)} · {entry.indexer} · {entry.reason}
          </span>
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

  .rows li {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--base-size-4);
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .title {
    font-weight: var(--base-text-weight-semibold);
    overflow-wrap: anywhere;
  }

  .small {
    font-size: var(--text-body-size-small);
    overflow-wrap: anywhere;
  }

  .message {
    color: var(--fgColor-danger);
    overflow-wrap: anywhere;
  }
</style>
