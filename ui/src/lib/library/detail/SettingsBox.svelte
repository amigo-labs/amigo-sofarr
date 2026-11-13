<script lang="ts">
  import type { ApiResult } from '../../api/http';
  import type { ChangePreview } from '../../api/generated/ChangePreview';
  import type { Choice } from '../../api/generated/Choice';
  import type { PreviewItem } from '../../api/generated/PreviewItem';
  import { AVAILABILITY_TEXT, availabilityText } from '../format';

  // Quality profile (and a movie's minimum availability) with a preview of what the change
  // does to the wanted lists before it is saved (spec web-ui-library AC-11).
  type Change = { profile: number; availability: string | null };
  type Props = {
    profiles: readonly Choice[];
    profile: Choice;
    /** `null` for a series (no minimum availability). */
    availability: string | null;
    preview: (change: Change) => Promise<ApiResult<ChangePreview>>;
    save: (change: Change) => Promise<boolean>;
  };
  let { profiles, profile, availability, preview, save }: Props = $props();

  let chosenProfile = $state(0);
  let chosenAvailability: string | null = $state(null);
  let result = $state<ChangePreview | null>(null);
  let failure: string | null = $state(null);
  let busy = $state(false);

  // Primitive copies: a refreshed answer with the same values leaves the fields alone.
  const savedProfile = $derived(profile.id);
  const savedAvailability = $derived(availability);

  // The fields start at the saved values and follow them after a save.
  $effect.pre(() => {
    chosenProfile = savedProfile;
    chosenAvailability = savedAvailability;
    result = null;
  });

  const changed = $derived(
    chosenProfile !== savedProfile || chosenAvailability !== savedAvailability,
  );
  const groups = $derived<readonly [string, readonly PreviewItem[]][]>(
    result === null
      ? []
      : [
          ['Werden fehlend', result.becomes_missing],
          ['Erreichen den Cutoff nicht mehr', result.becomes_cutoff_unmet],
          ['Fehlen nicht mehr', result.no_longer_missing],
          ['Erreichen den Cutoff', result.no_longer_cutoff_unmet],
        ],
  );
  const effects = $derived(groups.filter(([, items]) => items.length > 0));

  async function check(): Promise<void> {
    failure = null;
    result = null;
    if (!changed) return;
    busy = true;
    const answer = await preview({ profile: chosenProfile, availability: chosenAvailability });
    busy = false;
    if (answer.ok) result = answer.data;
    else failure = answer.failure.message;
  }

  async function submit(): Promise<void> {
    busy = true;
    const saved = await save({ profile: chosenProfile, availability: chosenAvailability });
    busy = false;
    if (saved) result = null;
  }
</script>

<section class="box" aria-labelledby="settings-title">
  <h2 id="settings-title" class="box-header">Einstellungen</h2>
  <div class="box-row body">
    <div class="fields">
      <label class="form-field">
        Qualitätsprofil
        <select bind:value={chosenProfile} onchange={() => void check()}>
          {#each profiles.some((p) => p.id === profile.id) ? profiles : [profile, ...profiles] as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
      </label>
      {#if availability !== null}
        <label class="form-field">
          Mindestverfügbarkeit
          <select bind:value={chosenAvailability} onchange={() => void check()}>
            {#each Object.keys(AVAILABILITY_TEXT) as value (value)}
              <option {value}>{availabilityText(value)}</option>
            {/each}
          </select>
        </label>
      {/if}
    </div>
    {#if failure !== null}<p class="error" role="alert">{failure}</p>{/if}
    {#if changed && result !== null}
      <div class="preview" data-testid="change-preview" aria-live="polite">
        <h3>Vorschau der Änderung</h3>
        {#if effects.length === 0}
          <p>Kein Eintrag wird dadurch fehlend oder verfehlt den Cutoff.</p>
        {:else}
          {#each effects as [title, items] (title)}
            <p><strong>{title}:</strong> {items.map((i) => i.label).join(', ')}</p>
          {/each}
        {/if}
      </div>
    {/if}
    <div class="buttons">
      <button
        type="button"
        class="btn btn-primary"
        disabled={!changed || result === null || busy}
        onclick={() => void submit()}
      >
        Speichern
      </button>
      {#if changed}
        <button
          type="button"
          class="btn"
          onclick={() => {
            chosenProfile = savedProfile;
            chosenAvailability = savedAvailability;
            result = null;
            failure = null;
          }}
        >
          Verwerfen
        </button>
      {/if}
    </div>
  </div>
</section>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
  }

  .fields {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
    gap: var(--base-size-12);
  }

  .preview {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    padding: var(--base-size-12);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-muted);
  }

  p {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .error {
    color: var(--fgColor-danger);
  }

  .buttons {
    display: flex;
    gap: var(--base-size-8);
  }
</style>
