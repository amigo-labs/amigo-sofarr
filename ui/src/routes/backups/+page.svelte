<script lang="ts">
  import downloadIcon from '@primer/octicons/build/svg/download-16.svg?raw';
  import trashIcon from '@primer/octicons/build/svg/trash-16.svg?raw';
  import { onMount } from 'svelte';
  import Flash from '#lib/components/Flash.svelte';
  import Icon from '#lib/components/Icon.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import PageError from '#lib/components/PageError.svelte';
  import type { ApiResult } from '#lib/api/http.ts';
  import type { BackupFile } from '#lib/api/generated/BackupFile.ts';
  import type { Kind } from '#lib/api/generated/Kind.ts';
  import {
    backupAddress,
    cancelRestore,
    chooseRestore,
    createBackup,
    deleteBackup,
    getBackups,
  } from '#lib/api/library.ts';
  import Dialog from '#lib/library/components/Dialog.svelte';
  import Label from '#lib/library/components/Label.svelte';
  import { bytesText } from '#lib/library/format.ts';
  import { notify } from '#lib/library/notices.svelte.ts';
  import { createResource } from '#lib/resource.svelte.ts';
  import { dateTimeText } from '#lib/system/format.ts';

  // The backups: create, download, delete, choose one for restore, cancel a pending
  // restore; the last error (spec web-ui-library AC-19).
  const backups = createResource(getBackups);
  onMount(() => backups.start());

  const KIND_TEXT: Readonly<Record<Kind, string>> = {
    scheduled: 'geplant',
    manual: 'manuell',
    'pre-migration': 'vor Migration',
    other: 'andere',
  };

  let deleting = $state<BackupFile | null>(null);
  let restoring = $state<BackupFile | null>(null);
  let busy = $state(false);

  async function act<T>(
    run: () => Promise<ApiResult<T>>,
    done: (data: T) => string,
    label: string,
  ): Promise<void> {
    busy = true;
    const result = await run();
    busy = false;
    deleting = null;
    restoring = null;
    if (result.ok) notify('info', done(result.data));
    else if (result.failure.kind !== 'unauthorized') {
      notify('danger', `${label} fehlgeschlagen: ${result.failure.message}`);
    }
    await backups.reload();
  }
</script>

<svelte:head>
  <title>Backups – sofarr</title>
</svelte:head>

<div class="head">
  <h1>Backups</h1>
  <button
    type="button"
    class="btn btn-primary"
    disabled={busy}
    onclick={() =>
      void act(createBackup, (b) => `Sicherung „${b.name}“ erstellt.`, 'Sicherung erstellen')}
  >
    Sicherung erstellen
  </button>
</div>

{#if backups.data !== null}
  {@const list = backups.data}
  {#if list.last_error !== null}
    <Flash tone="danger">
      <p data-testid="backup-error">
        Letzter Fehler ({dateTimeText(list.last_error.time)}): {list.last_error.message}
      </p>
    </Flash>
  {/if}
  {#if list.pending_restore !== null}
    <Flash tone="warning">
      <p data-testid="pending-restore">
        Beim nächsten Start wird „{list.pending_restore}“ eingespielt.
      </p>
      <button
        type="button"
        class="btn"
        disabled={busy}
        onclick={() =>
          void act(cancelRestore, () => 'Die Wiederherstellung ist abgebrochen.', 'Abbrechen')}
      >
        Wiederherstellung abbrechen
      </button>
    </Flash>
  {/if}
  {#if list.backups.length === 0}
    <p class="muted">Es gibt noch keine Sicherung.</p>
  {:else}
    <ul class="box rows">
      {#each list.backups as backup (backup.name)}
        <li class="box-row row" data-testid="backup">
          <div class="main">
            <span class="name mono">{backup.name}</span>
            <span class="muted small">
              {dateTimeText(backup.time)} · {bytesText(backup.size)} · {KIND_TEXT[backup.kind]}
            </span>
            {#if !backup.valid}<Label text="beschädigt" tone="danger" />{/if}
          </div>
          <div class="buttons">
            <a
              class="btn"
              href={backupAddress(backup.name)}
              download={backup.name}
              aria-label={`„${backup.name}“ herunterladen`}
            >
              <Icon svg={downloadIcon} />
            </a>
            <button
              type="button"
              class="btn"
              disabled={!backup.valid || busy}
              onclick={() => (restoring = backup)}
            >
              Wiederherstellen …
            </button>
            <button
              type="button"
              class="btn btn-danger"
              aria-label={`„${backup.name}“ löschen`}
              disabled={busy}
              onclick={() => (deleting = backup)}
            >
              <Icon svg={trashIcon} />
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
{:else if backups.failure !== null}
  <PageError
    title="Die Backups sind nicht verfügbar"
    message={backups.failure.message}
    action="Erneut versuchen"
    onaction={() => void backups.reload()}
  />
{:else}
  <Loading label="Backups werden geladen …" />
{/if}

<Dialog open={restoring !== null} title="Sicherung einspielen" onclose={() => (restoring = null)}>
  <p>
    Die Sicherung vom <strong>{restoring === null ? '' : dateTimeText(restoring.time)}</strong>
    („{restoring?.name}“) wird beim nächsten Start von sofarr eingespielt. Alles, was danach
    geändert wurde, geht verloren.
  </p>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => (restoring = null)}>Abbrechen</button>
    <button
      type="button"
      class="btn btn-danger"
      disabled={busy}
      onclick={() => {
        const chosen = restoring;
        if (chosen !== null) {
          void act(
            () => chooseRestore(chosen.name),
            (r) => r.message,
            'Wiederherstellung wählen',
          );
        }
      }}
    >
      Beim nächsten Start einspielen
    </button>
  {/snippet}
</Dialog>

<Dialog open={deleting !== null} title="Sicherung löschen" onclose={() => (deleting = null)}>
  <p>„{deleting?.name}“ wird endgültig gelöscht.</p>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => (deleting = null)}>Abbrechen</button>
    <button
      type="button"
      class="btn btn-danger"
      disabled={busy}
      onclick={() => {
        const chosen = deleting;
        if (chosen !== null) {
          void act(
            () => deleteBackup(chosen.name),
            () => `„${chosen.name}“ gelöscht.`,
            'Löschen',
          );
        }
      }}
    >
      Löschen
    </button>
  {/snippet}
</Dialog>

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--base-size-8);
  }

  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--base-size-12);
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .small {
    font-size: var(--text-body-size-small);
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-8);
  }

  p {
    margin: 0;
  }
</style>
