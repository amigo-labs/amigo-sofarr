<script lang="ts">
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import Dialog from '../../library/components/Dialog.svelte';
  import { availabilityText } from '../../library/format';
  import type { AddAnswer } from '../../api/generated/AddAnswer';
  import type { AddOptions } from '../../api/generated/AddOptions';
  import type { SeasonInfo } from '../../api/generated/SeasonInfo';
  import type { ApiResult } from '../../api/http';
  import {
    formOf,
    missingChoice,
    movieBody,
    needsConfirmation,
    seriesBody,
    toggled,
    type AddForm,
    type AddTarget,
  } from '../add';
  import { addMovie, addSeries, getAddOptions, getTitle } from '../api';
  import { seasonText } from '../texts';

  // Adding a movie or series (spec discover AC-07..AC-11): filled with the add defaults of
  // its media kind; an excluded title is added only after the user confirmed it.
  type Props = {
    target: AddTarget | null;
    onclose: () => void;
    onadded: (target: AddTarget, answer: AddAnswer) => void;
  };
  let { target, onclose, onadded }: Props = $props();

  const uid = `add-${Math.random().toString(36).slice(2)}`;
  let options: AddOptions | null = $state(null);
  let seasons: readonly SeasonInfo[] = $state([]);
  let form: AddForm | null = $state(null);
  let loadError: string | null = $state(null);
  let error: string | null = $state(null);
  let confirming = $state(false);
  let busy = $state(false);

  async function open(t: AddTarget): Promise<void> {
    options = null;
    form = null;
    loadError = null;
    error = null;
    confirming = false;
    const [opts, preview] = await Promise.all([
      getAddOptions(t.kind),
      t.kind === 'series' && t.seasons === null ? getTitle(t.kind, t.tmdbId) : null,
    ]);
    if (target !== t) return;
    if (!opts.ok) {
      loadError = opts.failure.message;
      return;
    }
    if (preview !== null && !preview.ok) {
      loadError = preview.failure.message;
      return;
    }
    seasons = t.seasons ?? preview?.data.seasons ?? [];
    options = opts.data;
    form = formOf(opts.data, seasons);
  }

  $effect(() => {
    if (target !== null) void open(target);
  });

  async function submit(confirmExcluded: boolean): Promise<void> {
    if (target === null || form === null) return;
    error = missingChoice(form);
    if (error !== null) return;
    const t = target;
    let result: ApiResult<AddAnswer> | null = null;
    busy = true;
    if (t.kind === 'movie') {
      const body = movieBody(t.tmdbId, form, confirmExcluded);
      if (body !== null) result = await addMovie(body);
    } else {
      const body = seriesBody(t.tmdbId, form, confirmExcluded);
      if (body !== null) result = await addSeries(body);
    }
    busy = false;
    if (result === null) return;
    if (result.ok) {
      onadded(t, result.data);
    } else if (needsConfirmation(result.failure)) {
      confirming = true;
    } else if (result.failure.kind !== 'unauthorized') {
      error = result.failure.message;
    }
  }
</script>

<Dialog
  open={target !== null}
  title={target === null ? '' : `„${target.title}“ hinzufügen`}
  {onclose}
>
  {#if loadError !== null}
    <Flash tone="danger"><p>Hinzufügen nicht möglich: {loadError}</p></Flash>
  {:else if options === null || form === null}
    <Loading label="Auswahl wird geladen …" />
  {:else}
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
    {#if target?.kind === 'movie'}
      <label class="form-field">
        Mindestverfügbarkeit
        <select bind:value={form.minimumAvailability}>
          {#each options.minimum_availability as value (value)}
            <option {value}>{availabilityText(value)}</option>
          {/each}
        </select>
      </label>
    {/if}
    <label class="check"><input type="checkbox" bind:checked={form.monitored} /> Überwachen</label>
    {#if target?.kind === 'series'}
      <fieldset>
        <legend>Überwachte Staffeln</legend>
        {#each seasons as season (season.number)}
          <label class="check">
            <input
              type="checkbox"
              checked={form.seasons.includes(season.number)}
              onchange={(e) => {
                if (form !== null)
                  form.seasons = toggled(form.seasons, season.number, e.currentTarget.checked);
              }}
            />
            {seasonText(season.name, season.number, season.episode_count)}
          </label>
        {:else}
          <p class="muted">TMDB nennt keine Staffeln.</p>
        {/each}
      </fieldset>
      <label class="check">
        <input type="checkbox" bind:checked={form.monitorNewSeasons} /> Neue Staffeln überwachen
      </label>
    {/if}
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
      <input type="checkbox" bind:checked={form.search} />
      {target?.kind === 'movie' ? 'Suchen, sobald verfügbar' : 'Fehlende Folgen gleich suchen'}
    </label>
    {#if confirming}
      <Flash tone="warning">
        <p id="{uid}-excluded">
          „{target?.title}“ steht auf der Ausschlussliste der Importlisten. Beim Hinzufügen wird der
          Titel von dieser Liste genommen.
        </p>
      </Flash>
    {/if}
    {#if error !== null}
      <Flash tone="danger"><p>Nicht hinzugefügt: {error}</p></Flash>
    {/if}
  {/if}
  {#snippet actions()}
    <button type="button" class="btn" onclick={onclose}>Abbrechen</button>
    {#if confirming}
      <button
        type="button"
        class="btn btn-primary"
        disabled={busy}
        aria-describedby="{uid}-excluded"
        onclick={() => void submit(true)}>Trotzdem hinzufügen</button
      >
    {:else}
      <button
        type="button"
        class="btn btn-primary"
        disabled={busy || form === null}
        onclick={() => void submit(false)}>{busy ? 'Wird hinzugefügt …' : 'Hinzufügen'}</button
      >
    {/if}
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
