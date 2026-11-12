<script lang="ts">
  import type { ActivityEntry } from '../../api/generated/ActivityEntry';
  import { dateTimeText } from '../../system/format';
  import { mediaHref } from '../actions';
  import { bytesText, statusText, statusTone } from '../format';
  import Label from '../components/Label.svelte';

  // Grabs and imports, newest first (AC-01): the last activities or the problems.
  type Props = { id: string; title: string; entries: readonly ActivityEntry[]; empty: string };
  let { id, title, entries, empty }: Props = $props();
</script>

<section class="box" aria-labelledby={id}>
  <h2 {id} class="box-header">{title}</h2>
  {#if entries.length === 0}
    <p class="box-row muted">{empty}</p>
  {:else}
    <ul class="rows">
      {#each entries as entry (entry.kind + entry.grab_id)}
        <li class="box-row">
          <div class="line">
            <a href={mediaHref(entry.kind, entry.media_id)} class="strong">{entry.media_title}</a>
            <Label text={statusText(entry.status)} tone={statusTone(entry.status)} />
          </div>
          <span class="muted small">{entry.release_title}</span>
          <span class="muted small">
            {dateTimeText(entry.updated_at)} · {entry.quality} · {bytesText(entry.size)}
          </span>
          {#if entry.message !== null}<span class="message">{entry.message}</span>{/if}
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
    gap: 2px;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .strong {
    font-weight: var(--base-text-weight-semibold);
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
