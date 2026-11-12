<script lang="ts">
  import trashIcon from '@primer/octicons/build/svg/trash-16.svg?raw';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Flash from '#lib/components/Flash.svelte';
  import Icon from '#lib/components/Icon.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import type { ApiFailure } from '#lib/api/http.ts';
  import type { BlocklistEntry } from '#lib/api/generated/BlocklistEntry.ts';
  import type { BlocklistPage } from '#lib/api/generated/BlocklistPage.ts';
  import { getBlocklist, unblock } from '#lib/api/library.ts';
  import { mediaHref } from '#lib/library/actions.ts';
  import Dialog from '#lib/library/components/Dialog.svelte';
  import { itemCount } from '#lib/library/format.ts';
  import { notify } from '#lib/library/notices.svelte.ts';
  import { dateTimeText } from '#lib/system/format.ts';

  // The blocklist page by page; an entry is removed after a confirmation (AC-18).
  const pageNumber = $derived(Math.max(1, Number(page.url.searchParams.get('seite')) || 1));

  let answer = $state<BlocklistPage | null>(null);
  let failure = $state<ApiFailure | null>(null);
  let removing = $state<BlocklistEntry | null>(null);
  let busy = $state(false);

  const pages = $derived(
    answer === null ? 1 : Math.max(1, Math.ceil(answer.total / answer.page_size)),
  );

  async function load(): Promise<void> {
    const result = await getBlocklist(pageNumber);
    if (result.ok) {
      answer = result.data;
      failure = null;
    } else if (result.failure.kind !== 'unauthorized') {
      failure = result.failure;
    }
  }

  $effect(() => {
    void pageNumber;
    void load();
  });

  function show(seite: number): void {
    void goto(seite > 1 ? `/blocklist?seite=${seite}` : '/blocklist', { reset: false });
  }

  const href = (entry: BlocklistEntry): string | null =>
    entry.movie_id !== null
      ? mediaHref('movie', entry.movie_id)
      : entry.series_id !== null
        ? mediaHref('series', entry.series_id)
        : null;

  async function confirmRemove(entry: BlocklistEntry): Promise<void> {
    busy = true;
    const result = await unblock(entry.id);
    busy = false;
    removing = null;
    if (result.ok) {
      notify('info', `„${entry.title}“ ist nicht mehr gesperrt.`);
      void load();
    } else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `Nicht entfernt: ${result.failure.message}`);
    }
  }
</script>

<svelte:head>
  <title>Blocklist – sofarr</title>
</svelte:head>

<h1>Blocklist</h1>

{#if failure !== null}
  <Flash tone="danger">
    <p>Die Blocklist konnte nicht geladen werden: {failure.message}</p>
    <button type="button" class="btn" onclick={() => void load()}>Erneut versuchen</button>
  </Flash>
{/if}

{#if answer === null && failure === null}
  <Loading label="Blocklist wird geladen …" />
{:else if answer !== null}
  <p class="muted" data-testid="blocklist-total">{itemCount(answer.total, 'item')}</p>
  {#if answer.entries.length === 0}
    <p class="muted">Die Blocklist ist leer.</p>
  {:else}
    <ul class="box rows">
      {#each answer.entries as entry (entry.id)}
        {@const link = href(entry)}
        <li class="box-row row" data-testid="blocklist-entry">
          <div class="main">
            {#if link !== null}<a href={link}>{entry.media_title}</a>{:else}<span
                >{entry.media_title}</span
              >{/if}
            <span class="release">{entry.title}</span>
            <span class="muted small">
              {dateTimeText(entry.blocklisted_at)} · {entry.indexer} · {entry.reason}
            </span>
          </div>
          <button
            type="button"
            class="btn btn-danger"
            aria-label={`„${entry.title}“ von der Blocklist entfernen`}
            onclick={() => (removing = entry)}
          >
            <Icon svg={trashIcon} />
          </button>
        </li>
      {/each}
    </ul>
    {#if pages > 1}
      <nav class="pager" aria-label="Seiten">
        <button
          type="button"
          class="btn"
          disabled={pageNumber <= 1}
          onclick={() => show(pageNumber - 1)}
        >
          Zurück
        </button>
        <span>Seite {pageNumber} von {pages}</span>
        <button
          type="button"
          class="btn"
          disabled={pageNumber >= pages}
          onclick={() => show(pageNumber + 1)}
        >
          Weiter
        </button>
      </nav>
    {/if}
  {/if}
{/if}

<Dialog
  open={removing !== null}
  title="Von der Blocklist entfernen"
  onclose={() => (removing = null)}
>
  <p>
    „{removing?.title}“ wird nicht mehr gesperrt; sofarr darf das Release wieder herunterladen.
  </p>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => (removing = null)}>Abbrechen</button>
    <button
      type="button"
      class="btn btn-danger"
      disabled={busy}
      onclick={() => removing !== null && void confirmRemove(removing)}
    >
      Entfernen
    </button>
  {/snippet}
</Dialog>

<style>
  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--base-size-12);
  }

  .main {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .release {
    font-weight: var(--base-text-weight-semibold);
  }

  .small {
    font-size: var(--text-body-size-small);
  }

  .pager {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  p {
    margin: 0;
  }
</style>
