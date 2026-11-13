<script lang="ts">
  import { goto } from '$app/navigation';
  import Flash from '../../components/Flash.svelte';
  import type { FileActions } from '../../api/generated/FileActions';
  import type { FolderState } from '../../api/generated/FolderState';
  import type { RenameEntry } from '../../api/generated/RenameEntry';
  import { libraryKind, moveFolder, remove, rename, renamePreview } from '../../api/library';
  import { itemCount, removeText } from '../format';
  import { notify } from '../notices.svelte';
  import Dialog from '../components/Dialog.svelte';

  // Folder and file actions of a movie or series: rename preview, moving the folder to its
  // current name, removing (spec web-ui-library AC-12, AC-13, AC-15 to AC-17, AC-24).
  type Props = {
    kind: 'movie' | 'series';
    id: number;
    title: string;
    folder: FolderState;
    fileActions: FileActions;
    onchanged: () => void;
  };
  let { kind, id, title, folder, fileActions, onchanged }: Props = $props();

  const path = $derived(libraryKind(kind));
  const listHref = $derived(kind === 'movie' ? '/filme' : '/serien');

  let renames: readonly RenameEntry[] | null = $state(null);
  let moving = $state(false);
  let removing = $state(false);
  let deleteFiles = $state(false);
  let addExclusion = $state(false);
  let busy = $state(false);
  let failure: string | null = $state(null);

  function failed(action: string, message: string): void {
    failure = `${action} nicht ausgeführt: ${message}`;
  }

  async function openRename(): Promise<void> {
    failure = null;
    busy = true;
    const answer = await renamePreview(path, id);
    busy = false;
    if (answer.ok) renames = answer.data.files;
    else if (answer.failure.kind !== 'unauthorized') failed('Umbenennen', answer.failure.message);
  }

  async function doRename(): Promise<void> {
    busy = true;
    const answer = await rename(path, id);
    busy = false;
    renames = null;
    if (answer.ok) {
      notify('info', `${itemCount(answer.data.renamed, 'file')} umbenannt.`);
      onchanged();
    } else if (answer.failure.kind !== 'unauthorized') {
      failed('Umbenennen', answer.failure.message);
      onchanged();
    }
  }

  async function doMove(): Promise<void> {
    busy = true;
    const answer = await moveFolder(path, id);
    busy = false;
    moving = false;
    if (answer.ok) {
      notify('info', `Ordner verschoben nach ${answer.data.path}.`);
      onchanged();
    } else if (answer.failure.kind !== 'unauthorized') {
      failed('Ordner verschieben', answer.failure.message);
      onchanged();
    }
  }

  async function doRemove(): Promise<void> {
    busy = true;
    const answer = await remove(path, id, {
      delete_files: deleteFiles && fileActions.available,
      add_exclusion: addExclusion,
    });
    busy = false;
    removing = false;
    if (answer.ok) {
      notify('info', removeText(title, answer.data), true);
      await goto(listHref);
    } else if (answer.failure.kind !== 'unauthorized') {
      failed('Entfernen', answer.failure.message);
      onchanged();
    }
  }
</script>

<section class="box" aria-labelledby="files-title">
  <h2 id="files-title" class="box-header">Ordner und Dateiaktionen</h2>
  <div class="box-row body">
    {#if failure !== null}
      <Flash tone="danger">
        <p>{failure}</p>
        <button type="button" class="btn" onclick={() => (failure = null)}>Schließen</button>
      </Flash>
    {/if}
    <dl class="facts">
      <div>
        <dt>Ordner</dt>
        <dd class="mono" data-testid="folder">{folder.current}</dd>
      </div>
    </dl>
    {#if folder.differs && folder.expected !== null}
      <div class="differs" data-testid="folder-differs">
        <p><strong>Der Ordner weicht vom aktuellen Ordnernamen ab.</strong></p>
        <p>Bisher: <span class="mono">{folder.current}</span></p>
        <p>Neu: <span class="mono">{folder.expected}</span></p>
      </div>
    {/if}
    {#if !fileActions.available}
      <Flash tone="info">
        <p>
          {fileActions.reason ?? 'Dateiaktionen sind gerade nicht möglich.'} Überwachung, Profil und Entfernen
          ohne Dateien funktionieren trotzdem.
        </p>
      </Flash>
    {/if}
    <div class="buttons">
      {#if fileActions.available}
        <button type="button" class="btn" disabled={busy} onclick={() => void openRename()}>
          Umbenennen …
        </button>
        {#if folder.differs && folder.expected !== null}
          <button type="button" class="btn" disabled={busy} onclick={() => (moving = true)}>
            Ordner verschieben …
          </button>
        {/if}
      {/if}
      <button
        type="button"
        class="btn btn-danger"
        disabled={busy}
        onclick={() => {
          deleteFiles = false;
          addExclusion = false;
          removing = true;
        }}
      >
        Entfernen …
      </button>
    </div>
  </div>
</section>

<Dialog open={renames !== null} title="Dateien umbenennen" onclose={() => (renames = null)}>
  {#if renames !== null && renames.length === 0}
    <p>Alle Dateinamen entsprechen dem Benennungsschema; es gibt nichts umzubenennen.</p>
  {:else if renames !== null}
    <p>Diese {renames.length === 1 ? 'Datei bekommt' : 'Dateien bekommen'} einen neuen Namen:</p>
    <ul class="renames" data-testid="rename-preview">
      {#each renames as entry (entry.old_path)}
        <li>
          <span class="old mono">{entry.old_path}</span>
          <span class="new mono">→ {entry.new_path}</span>
        </li>
      {/each}
    </ul>
  {/if}
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => (renames = null)}>
      {renames !== null && renames.length === 0 ? 'Schließen' : 'Abbrechen'}
    </button>
    {#if renames !== null && renames.length > 0}
      <button type="button" class="btn btn-primary" disabled={busy} onclick={() => void doRename()}>
        Umbenennen
      </button>
    {/if}
  {/snippet}
</Dialog>

<Dialog open={moving} title="Ordner verschieben" onclose={() => (moving = false)}>
  <p>Der Ordner und alles darin werden umbenannt:</p>
  <p class="mono">{folder.current}</p>
  <p class="mono">→ {folder.expected}</p>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => (moving = false)}>Abbrechen</button>
    <button type="button" class="btn btn-primary" disabled={busy} onclick={() => void doMove()}>
      Verschieben
    </button>
  {/snippet}
</Dialog>

<Dialog open={removing} title={`„${title}“ entfernen`} onclose={() => (removing = false)}>
  <p>
    {kind === 'movie' ? 'Der Film' : 'Die Serie'} wird aus der Bibliothek entfernt. Ohne „Dateien löschen“
    bleiben die Dateien liegen; laufende Downloads werden beim Download-Client entfernt.
  </p>
  <label class="check">
    <input type="checkbox" bind:checked={deleteFiles} disabled={!fileActions.available} />
    <span>
      Dateien löschen
      <span class="muted hint">
        {fileActions.available
          ? 'In den Papierkorb, falls einer eingerichtet ist, sonst endgültig.'
          : (fileActions.reason ?? 'Gerade nicht möglich.')}
      </span>
    </span>
  </label>
  <label class="check">
    <input type="checkbox" bind:checked={addExclusion} />
    <span>
      auf die Ausschlussliste
      <span class="muted hint">Import-Listen fügen den Titel nie wieder hinzu.</span>
    </span>
  </label>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => (removing = false)}>Abbrechen</button>
    <button type="button" class="btn btn-danger" disabled={busy} onclick={() => void doRemove()}>
      Entfernen
    </button>
  {/snippet}
</Dialog>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
  }

  .differs {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    padding: var(--base-size-12);
    border: var(--borderWidth-thin) solid var(--borderColor-attention-emphasis);
    border-radius: var(--borderRadius-medium);
  }

  p {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-8);
  }

  .renames {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .renames li {
    display: flex;
    flex-direction: column;
  }

  .old {
    color: var(--fgColor-muted);
  }

  .hint {
    display: block;
    font-size: var(--text-body-size-small);
  }
</style>
