<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import Icon from '../components/Icon.svelte';
  import type { NeededSubtitle } from '../api/generated/NeededSubtitle';
  import type { SubtitleItem } from '../api/generated/SubtitleItem';
  import { dateTimeText } from '../system/format';
  import { itemHref, languageName, needKindText, targetKey } from './subtitles';

  // The needed subtitles of the overview, grouped by file, each file with a search at once
  // (spec subtitles AC-11, AC-13).
  type Props = {
    needed: readonly NeededSubtitle[];
    enabled: boolean;
    busy: ReadonlySet<string>;
    onsearch: (item: SubtitleItem) => void;
  };
  let { needed, enabled, busy, onsearch }: Props = $props();

  type Group = {
    readonly key: string;
    readonly item: SubtitleItem;
    readonly needs: NeededSubtitle[];
  };

  const groups = $derived.by(() => {
    const byFile = new Map<string, Group>();
    for (const need of needed) {
      const key = targetKey(need.item);
      const group = byFile.get(key);
      if (group === undefined) byFile.set(key, { key, item: need.item, needs: [need] });
      else group.needs.push(need);
    }
    return [...byFile.values()];
  });
</script>

{#if groups.length === 0}
  <p class="box-row">Keine – jede gewünschte Sprache ist vorhanden.</p>
{:else}
  <ul class="files">
    {#each groups as group (group.key)}
      {@const href = itemHref(group.item)}
      <li class="box-row" data-file={group.key}>
        <div class="head">
          {#if href === null}
            <strong>{group.item.title}</strong>
          {:else}
            <a class="title" {href}>{group.item.title}</a>
          {/if}
          {#if enabled}
            <button
              type="button"
              class="btn"
              disabled={busy.has(group.key)}
              onclick={() => onsearch(group.item)}
            >
              <Icon svg={searchIcon} />{busy.has(group.key) ? 'Suche läuft …' : 'Jetzt suchen'}
              <span class="sr-only">für „{group.item.title}“</span>
            </button>
          {/if}
        </div>
        <ul class="needs">
          {#each group.needs as need (`${need.language}-${need.kind}`)}
            <li data-need={`${need.language}-${need.kind}`}>
              <span><strong>{languageName(need.language)}</strong> · {needKindText(need.kind)}</span
              >
              <span class="small muted">
                zuletzt gesucht: {need.last_search === null
                  ? 'noch nie'
                  : dateTimeText(need.last_search)}
              </span>
              <span class="small muted">
                nächste Suche: {need.next_search === null
                  ? 'beim nächsten Lauf'
                  : dateTimeText(need.next_search)}
              </span>
              {#if need.would_download !== null}
                <span class="small"
                  >würde laden: <span class="mono">{need.would_download}</span></span
                >
              {/if}
            </li>
          {/each}
        </ul>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .files,
  .needs {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--base-size-8);
  }

  .title {
    font-weight: var(--base-text-weight-semibold);
    overflow-wrap: anywhere;
  }

  .needs {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    margin-top: var(--base-size-8);
  }

  .needs li {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--base-size-12);
    overflow-wrap: anywhere;
  }

  .small {
    font-size: var(--text-body-size-small);
  }

  p {
    margin: 0;
  }
</style>
