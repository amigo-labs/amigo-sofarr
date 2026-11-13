<script lang="ts">
  import { onMount } from 'svelte';
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import Icon from '../components/Icon.svelte';
  import Loading from '../components/Loading.svelte';
  import type { ApiFailure } from '../api/http';
  import type { FileSubtitles } from '../api/generated/FileSubtitles';
  import Label from '../library/components/Label.svelte';
  import { notify } from '../library/notices.svelte';
  import { dateTimeText } from '../system/format';
  import {
    getFileSubtitles,
    kindText,
    languageName,
    matchText,
    needKindText,
    searchReportText,
    searchSubtitles,
    type SubtitleTarget,
  } from './subtitles';

  // The subtitles of one video file – embedded, as files next to it (sofarr's own marked),
  // still needed – and a search at once (spec subtitles AC-11, AC-12).
  type Props = { target: SubtitleTarget; title: string };
  let { target, title }: Props = $props();

  let data = $state<FileSubtitles | null>(null);
  let failure = $state<ApiFailure | null>(null);
  let busy = $state(false);

  async function load(): Promise<void> {
    const result = await getFileSubtitles(target);
    if (result.ok) {
      data = result.data;
      failure = null;
    } else if (result.failure.kind !== 'unauthorized') {
      failure = result.failure;
    }
  }

  onMount(() => {
    void load();
  });

  async function search(): Promise<void> {
    busy = true;
    const result = await searchSubtitles(target);
    busy = false;
    if (result.ok) notify('info', searchReportText(title, result.data));
    else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `Untertitelsuche für „${title}“: ${result.failure.message}`);
    }
    await load();
  }
</script>

{#if data !== null}
  <dl class="subtitles" data-testid="subtitles">
    <div>
      <dt>Eingebettet</dt>
      <dd data-testid="subtitles-embedded">
        {#if data.embedded === null}
          <span class="muted">unbekannt – die Media-Infos der Datei sind noch nicht gelesen</span>
        {:else if data.embedded.length === 0}
          keine
        {:else}
          <ul>
            {#each data.embedded as track, i (i)}
              <li>
                <strong>{languageName(track.language)}</strong>
                · {kindText(track.forced, track.hearing_impaired)}
                <span class="muted">· {track.format}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </dd>
    </div>
    <div>
      <dt>Als Datei</dt>
      <dd data-testid="subtitles-external">
        {#if data.external.length === 0}
          keine
        {:else}
          <ul>
            {#each data.external as file (file.file_name)}
              <li>
                <strong>{languageName(file.language)}</strong>
                · {kindText(file.forced, file.hearing_impaired)}
                {#if file.own}
                  <Label text="von sofarr" tone="success" />
                  <span class="muted">Treffer: {matchText(file.match_quality)}</span>
                {:else}
                  <Label text="fremd" />
                {/if}
                <span class="mono name">{file.file_name}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </dd>
    </div>
    <div>
      <dt>Benötigt</dt>
      <dd data-testid="subtitles-needed">
        {#if data.needed.length === 0}
          keine – jede gewünschte Sprache ist vorhanden
        {:else}
          <ul>
            {#each data.needed as need (`${need.language}-${need.kind}`)}
              <li>
                <strong>{languageName(need.language)}</strong> · {needKindText(need.kind)}
                <span class="muted">
                  · nächste Suche {need.next_search === null
                    ? 'beim nächsten Lauf'
                    : dateTimeText(need.next_search)}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </dd>
    </div>
  </dl>
  {#if data.enabled}
    <div>
      <button type="button" class="btn" disabled={busy} onclick={() => void search()}>
        <Icon svg={searchIcon} />{busy ? 'Untertitelsuche läuft …' : 'Untertitelsuche starten'}
      </button>
    </div>
  {:else}
    <p class="muted">
      Die Untertitelsuche ist ausgeschaltet; den Grund nennt die Seite
      <a href="/untertitel">Untertitel</a>.
    </p>
  {/if}
{:else if failure !== null}
  <p>Die Untertitel sind nicht verfügbar: {failure.message}</p>
  <div>
    <button type="button" class="btn" onclick={() => void load()}>Erneut versuchen</button>
  </div>
{:else}
  <Loading label="Untertitel werden geladen …" />
{/if}

<style>
  .subtitles {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    margin: 0;
  }

  dt {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .name {
    display: block;
  }

  p {
    margin: 0;
  }
</style>
