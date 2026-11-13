<script lang="ts">
  import type { FoundSubtitle } from '../api/generated/FoundSubtitle';
  import { dateTimeText } from '../system/format';
  import { itemHref, languageName, matchText, needKindText } from './subtitles';

  // Own subtitles found in the last 7 days, newest first (spec subtitles AC-11).
  type Props = { recent: readonly FoundSubtitle[] };
  let { recent }: Props = $props();
</script>

{#if recent.length === 0}
  <p class="box-row">In den letzten 7 Tagen wurden keine Untertitel gefunden.</p>
{:else}
  <ul class="found">
    {#each recent as found (found.file_name)}
      {@const href = itemHref(found.item)}
      <li class="box-row" data-found={found.file_name}>
        <div class="head">
          {#if href === null}
            <strong>{found.item.title}</strong>
          {:else}
            <a class="title" {href}>{found.item.title}</a>
          {/if}
          <span>
            <strong>{languageName(found.language)}</strong> · {needKindText(
              found.kind,
              found.hearing_impaired,
            )}
          </span>
          <span class="small muted">{dateTimeText(found.created_at)}</span>
        </div>
        <span class="mono">{found.file_name}</span>
        <span class="small muted"
          >Release: {found.release} · Treffer: {matchText(found.match_quality)}</span
        >
      </li>
    {/each}
  </ul>
{/if}

<style>
  .found {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .found li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-wrap: anywhere;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--base-size-12);
  }

  .title {
    font-weight: var(--base-text-weight-semibold);
  }

  .small {
    font-size: var(--text-body-size-small);
  }

  p {
    margin: 0;
  }
</style>
