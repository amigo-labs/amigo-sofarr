<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Loading from '../../components/Loading.svelte';
  import PageError from '../../components/PageError.svelte';
  import ImdbAttribution from '../../imdb/ImdbAttribution.svelte';
  import { dateText } from '../../library/format';
  import { notify } from '../../library/notices.svelte';
  import type { ApiFailure } from '../../api/http';
  import type { PersonPage } from '../../api/generated/PersonPage';
  import type { TitleCard } from '../../api/generated/TitleCard';
  import { addedText, type AddTarget } from '../add';
  import { getPerson } from '../api';
  import { choiceParams, parseChoices, type CreditChoices } from '../credits';
  import { createTitleStates } from '../states.svelte';
  import { discover } from '../status.svelte';
  import { departmentText } from '../texts';
  import AddDialog from '../components/AddDialog.svelte';
  import CreditSection from '../components/CreditSection.svelte';
  import FreshnessNote from '../components/FreshnessNote.svelte';
  import ImportListDialog from '../components/ImportListDialog.svelte';
  import StatusGate from '../components/StatusGate.svelte';
  import TmdbImage from '../components/TmdbImage.svelte';

  // A person with every movie and series, as cast and per crew department (spec discover
  // AC-05), and the TMDb person import list (AC-12).
  type Props = { id: number };
  let { id }: Props = $props();

  const states = createTitleStates();
  let person: PersonPage | null = $state(null);
  let failure: ApiFailure | null = $state(null);
  let adding: AddTarget | null = $state(null);
  let listOpen = $state(false);

  const ratings = $derived(discover.status?.ratings === true);
  const choices = $derived(parseChoices(page.url.searchParams, ratings));

  async function load(): Promise<void> {
    failure = null;
    const result = await getPerson(id);
    if (!result.ok) {
      if (result.failure.kind !== 'unauthorized') failure = result.failure;
      return;
    }
    person = result.data;
    states.show([
      ...result.data.cast.map((c) => c.title),
      ...result.data.crew.flatMap((d) => d.credits.map((c) => c.title)),
    ]);
  }

  onMount(() => {
    void load();
    return states.start();
  });

  function choose(next: CreditChoices): void {
    const params = choiceParams(next).toString();
    void goto(params === '' ? page.url.pathname : `?${params}`, {
      replace: true,
      reset: false,
    });
  }

  function onadd(card: TitleCard): void {
    adding = { kind: card.kind, tmdbId: card.tmdb_id, title: card.title, seasons: null };
  }
</script>

<svelte:head>
  <title>{person?.name ?? 'Person'} – sofarr</title>
</svelte:head>

<StatusGate title="Person">
  {#if person !== null}
    {@const p = person}
    <section class="head">
      <div class="photo"><TmdbImage src={p.photo} name={p.name} shape="photo" lazy={false} /></div>
      <div class="text">
        <h1>{p.name}</h1>
        <dl class="facts">
          {#if p.department !== null}<div>
              <dt>Bekannt für</dt>
              <dd>{departmentText(p.department)}</dd>
            </div>{/if}
          {#if p.birthday !== null}<div>
              <dt>Geboren</dt>
              <dd>
                {dateText(p.birthday)}{#if p.place_of_birth !== null}, {p.place_of_birth}{/if}
              </dd>
            </div>{/if}
          {#if p.deathday !== null}<div>
              <dt>Gestorben</dt>
              <dd>{dateText(p.deathday)}</dd>
            </div>{/if}
        </dl>
        {#if p.biography !== null}<p class="bio">{p.biography}</p>{/if}
        <div class="actions">
          <button type="button" class="btn" onclick={() => (listOpen = true)}>
            Import-Liste anlegen
          </button>
        </div>
      </div>
    </section>

    <form class="choices" aria-label="Titel ordnen" onsubmit={(e) => e.preventDefault()}>
      <label class="form-field">
        Sortierung
        <select
          value={choices.sort}
          onchange={(e) => {
            const v = e.currentTarget.value;
            choose({
              ...choices,
              sort: v === 'aelteste' || v === 'bewertung' ? v : 'neueste',
            });
          }}
        >
          <option value="neueste">Jahr, neueste zuerst</option>
          <option value="aelteste">Jahr, älteste zuerst</option>
          {#if ratings}<option value="bewertung">IMDb-Bewertung</option>{/if}
        </select>
      </label>
      <label class="form-field">
        Art
        <select
          value={choices.kind ?? ''}
          onchange={(e) => {
            const v = e.currentTarget.value;
            choose({ ...choices, kind: v === 'movie' || v === 'series' ? v : null });
          }}
        >
          <option value="">Filme und Serien</option>
          <option value="movie">Filme</option>
          <option value="series">Serien</option>
        </select>
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={choices.missingOnly}
          onchange={(e) => choose({ ...choices, missingOnly: e.currentTarget.checked })}
        />
        nur fehlende
      </label>
    </form>

    <CreditSection
      id="credits-cast"
      title="Darsteller"
      credits={p.cast}
      {choices}
      stateOf={states.of}
      canAdd={true}
      {onadd}
      empty="{p.name} hat keine Rollen vor der Kamera – nur Arbeit hinter der Kamera."
    />
    {#each p.crew as department, i (department.department)}
      <CreditSection
        id="credits-crew-{i}"
        title={departmentText(department.department)}
        credits={department.credits}
        {choices}
        stateOf={states.of}
        canAdd={true}
        {onadd}
        empty="Keine Titel."
      />
    {/each}
    <FreshnessNote freshness={p.freshness} />
    <ImdbAttribution info={discover.imdb} />

    <ImportListDialog
      open={listOpen}
      personId={p.tmdb_id}
      name={p.name}
      department={p.department}
      onclose={() => (listOpen = false)}
      oncreated={(answer) => {
        listOpen = false;
        notify(
          'info',
          `Import-Liste „${answer.name}“ angelegt; die nächste Synchronisierung fügt die Titel hinzu.`,
        );
      }}
    />
  {:else if failure !== null}
    <PageError
      title={failure.status === 404 ? 'Person nicht gefunden' : 'TMDB ist nicht verfügbar'}
      message={failure.message}
      action="Erneut versuchen"
      onaction={() => void load()}
    />
  {:else}
    <Loading label="Person wird geladen …" />
  {/if}
</StatusGate>

<AddDialog
  target={adding}
  onclose={() => (adding = null)}
  onadded={(target, answer) => {
    states.added(target.kind, target.tmdbId, answer.id);
    notify('info', addedText(answer));
    adding = null;
  }}
/>

<style>
  .head {
    display: grid;
    grid-template-columns: minmax(6rem, 12rem) minmax(0, 1fr);
    gap: var(--base-size-24);
    align-items: start;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
    min-width: 0;
  }

  h1 {
    overflow-wrap: anywhere;
  }

  .bio {
    max-width: 50rem;
    margin: 0;
    white-space: pre-line;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-8);
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--base-size-8) var(--base-size-16);
  }

  @media (max-width: 543.98px) {
    .head {
      grid-template-columns: minmax(0, 1fr);
    }

    .photo {
      max-width: 9rem;
    }
  }
</style>
