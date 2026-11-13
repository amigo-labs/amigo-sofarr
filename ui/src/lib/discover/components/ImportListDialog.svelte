<script lang="ts">
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import Dialog from '../../library/components/Dialog.svelte';
  import { availabilityText } from '../../library/format';
  import type { AddOptions } from '../../api/generated/AddOptions';
  import type { ImportListAnswer } from '../../api/generated/ImportListAnswer';
  import { formOf, missingChoice, toggled, type AddForm } from '../add';
  import { createImportList, getAddOptions } from '../api';
  import { departmentText } from '../texts';

  // A TMDb person import list for the person of the page (spec discover AC-12): cast and
  // the crew departments the list supports, with the choices of adding a movie.
  type Props = {
    open: boolean;
    personId: number;
    name: string;
    /** What the person is known for, e.g. `Acting` or `Directing`. */
    department: string | null;
    onclose: () => void;
    oncreated: (answer: ImportListAnswer) => void;
  };
  let { open, personId, name, department, onclose, oncreated }: Props = $props();

  /** The crew departments a person list can follow. */
  const DEPARTMENTS = ['Directing', 'Production', 'Sound', 'Writing'] as const;

  let options: AddOptions | null = $state(null);
  let form: AddForm | null = $state(null);
  let cast = $state(true);
  let departments: string[] = $state([]);
  let loadError: string | null = $state(null);
  let error: string | null = $state(null);
  let busy = $state(false);

  async function load(): Promise<void> {
    options = null;
    form = null;
    error = null;
    loadError = null;
    cast = department === null || department === 'Acting';
    departments = DEPARTMENTS.filter((d) => d === department);
    const result = await getAddOptions('movie');
    if (!result.ok) {
      loadError = result.failure.message;
      return;
    }
    options = result.data;
    form = formOf(result.data, []);
  }

  $effect(() => {
    if (open) void load();
  });

  async function submit(): Promise<void> {
    if (form === null) return;
    error =
      !cast && departments.length === 0
        ? 'Bitte Besetzung oder mindestens eine Abteilung wählen.'
        : missingChoice(form);
    if (error !== null || form.profile === null || form.rootFolder === null) return;
    busy = true;
    const result = await createImportList(personId, {
      cast,
      departments: [...departments],
      quality_profile_id: form.profile,
      root_folder: form.rootFolder,
      monitored: form.monitored,
      minimum_availability: form.minimumAvailability,
      tags: [...form.tags],
      search_on_add: form.search,
    });
    busy = false;
    if (result.ok) oncreated(result.data);
    else if (result.failure.kind !== 'unauthorized') error = result.failure.message;
  }
</script>

<Dialog {open} title={`Import-Liste für ${name} anlegen`} {onclose}>
  {#if loadError !== null}
    <Flash tone="danger"><p>Anlegen nicht möglich: {loadError}</p></Flash>
  {:else if options === null || form === null}
    <Loading label="Auswahl wird geladen …" />
  {:else}
    <p class="muted">
      Die nächste Synchronisierung der Importlisten fügt die Filme dieser Person hinzu.
    </p>
    <fieldset>
      <legend>Filme aus</legend>
      <label class="check"><input type="checkbox" bind:checked={cast} /> Besetzung</label>
      {#each DEPARTMENTS as d (d)}
        <label class="check">
          <input
            type="checkbox"
            checked={departments.includes(d)}
            onchange={(e) => {
              departments = e.currentTarget.checked
                ? [...departments, d]
                : departments.filter((x) => x !== d);
            }}
          />
          {departmentText(d)}
        </label>
      {/each}
    </fieldset>
    <label class="form-field">
      Qualitätsprofil
      <select bind:value={form.profile}>
        {#if form.profile === null}<option value={null}>Bitte wählen</option>{/if}
        {#each options.profiles as profile (profile.id)}
          <option value={profile.id}>{profile.name}</option>
        {/each}
      </select>
    </label>
    <label class="form-field">
      Stammordner
      <select bind:value={form.rootFolder}>
        {#if form.rootFolder === null}<option value={null}>Bitte wählen</option>{/if}
        {#each options.root_folders as folder (folder)}
          <option value={folder}>{folder}</option>
        {/each}
      </select>
    </label>
    <label class="form-field">
      Mindestverfügbarkeit
      <select bind:value={form.minimumAvailability}>
        {#each options.minimum_availability as value (value)}
          <option {value}>{availabilityText(value)}</option>
        {/each}
      </select>
    </label>
    <label class="check"><input type="checkbox" bind:checked={form.monitored} /> Überwachen</label>
    {#if options.tags.length > 0}
      <fieldset>
        <legend>Tags</legend>
        {#each options.tags as tag (tag.id)}
          <label class="check">
            <input
              type="checkbox"
              checked={form.tags.includes(tag.id)}
              onchange={(e) => {
                if (form !== null) form.tags = toggled(form.tags, tag.id, e.currentTarget.checked);
              }}
            />
            {tag.name}
          </label>
        {/each}
      </fieldset>
    {/if}
    <label class="check">
      <input type="checkbox" bind:checked={form.search} /> Beim Hinzufügen suchen
    </label>
    {#if error !== null}
      <Flash tone="danger"><p>Nicht angelegt: {error}</p></Flash>
    {/if}
  {/if}
  {#snippet actions()}
    <button type="button" class="btn" onclick={onclose}>Abbrechen</button>
    <button
      type="button"
      class="btn btn-primary"
      disabled={busy || form === null}
      onclick={() => void submit()}>Import-Liste anlegen</button
    >
  {/snippet}
</Dialog>

<style>
  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    margin: 0;
    padding: var(--base-size-8) var(--base-size-12);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-medium);
  }

  legend {
    padding: 0 var(--base-size-4);
    font-weight: var(--base-text-weight-semibold);
  }
</style>
