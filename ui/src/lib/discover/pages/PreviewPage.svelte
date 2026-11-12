<script lang="ts">
  import plusIcon from '@primer/octicons/build/svg/plus-16.svg?raw';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import Icon from '../../components/Icon.svelte';
  import Loading from '../../components/Loading.svelte';
  import PageError from '../../components/PageError.svelte';
  import ImdbAttribution from '../../imdb/ImdbAttribution.svelte';
  import Label from '../../library/components/Label.svelte';
  import { mediaHref } from '../../library/actions';
  import { dateText } from '../../library/format';
  import { notify } from '../../library/notices.svelte';
  import type { ApiFailure } from '../../api/http';
  import type { TitleKind } from '../../api/generated/TitleKind';
  import type { TitlePreview } from '../../api/generated/TitlePreview';
  import { addedText, type AddTarget } from '../add';
  import { getTitle } from '../api';
  import { createTitleStates } from '../states.svelte';
  import { discover } from '../status.svelte';
  import { KIND_TEXT, jobText, libraryLabel, runtimeText, seasonText, statusText } from '../texts';
  import AddDialog from '../components/AddDialog.svelte';
  import FreshnessNote from '../components/FreshnessNote.svelte';
  import PeopleList from '../components/PeopleList.svelte';
  import RatingText from '../components/RatingText.svelte';
  import StatusGate from '../components/StatusGate.svelte';
  import TmdbImage from '../components/TmdbImage.svelte';

  // The preview of a title that is not in the library (spec discover AC-04); a title in
  // the library opens its library page instead.
  type Props = { kind: TitleKind; id: number };
  let { kind, id }: Props = $props();

  const states = createTitleStates();
  let preview: TitlePreview | null = $state(null);
  let failure: ApiFailure | null = $state(null);
  let adding: AddTarget | null = $state(null);

  async function load(): Promise<void> {
    failure = null;
    const result = await getTitle(kind, id);
    if (!result.ok) {
      if (result.failure.kind !== 'unauthorized') failure = result.failure;
      return;
    }
    const { library } = result.data;
    if (library.status === 'in_library' && library.id !== null) {
      await goto(mediaHref(kind, library.id), { replace: true });
      return;
    }
    preview = result.data;
    states.show([result.data]);
  }

  onMount(() => {
    void load();
    return states.start();
  });

  const current = $derived(preview === null ? null : states.of(preview));
  const label = $derived(current === null ? null : libraryLabel(current.library.status));
</script>

<svelte:head>
  <title>{preview?.title ?? KIND_TEXT[kind]} – sofarr</title>
</svelte:head>

<StatusGate title={KIND_TEXT[kind]}>
  {#if preview !== null && current !== null}
    {@const p = preview}
    <section class="head" data-testid="preview">
      <div class="poster"><TmdbImage src={p.poster} name={p.title} lazy={false} /></div>
      <div class="text">
        <h1>{p.title}<span class="year">{p.year === null ? '' : ` (${p.year})`}</span></h1>
        {#if p.original_title !== p.title}<p class="muted">
            Originaltitel: {p.original_title}
          </p>{/if}
        <div class="labels">
          <Label text={KIND_TEXT[p.kind]} />
          {#if label !== null}<Label text={label.text} tone={label.tone} />{/if}
          <RatingText rating={current.rating} />
        </div>
        {#if p.overview !== null}<p class="overview">{p.overview}</p>{/if}
        <div class="actions">
          {#if current.library.status === 'in_library' && current.library.id !== null}
            <a class="btn" href={mediaHref(p.kind, current.library.id)}>Zur Bibliothek</a>
          {:else}
            <button
              type="button"
              class="btn btn-primary"
              onclick={() =>
                (adding = { kind: p.kind, tmdbId: p.tmdb_id, title: p.title, seasons: p.seasons })}
            >
              <Icon svg={plusIcon} />Hinzufügen
            </button>
          {/if}
          {#if p.trailer !== null}
            <a class="btn" href={p.trailer} rel="noopener noreferrer" target="_blank"
              >Trailer auf YouTube</a
            >
          {/if}
        </div>
        <dl class="facts">
          <div>
            <dt>{p.kind === 'movie' ? 'Erscheinungsdatum' : 'Erstausstrahlung'}</dt>
            <dd>{dateText(p.date)}</dd>
          </div>
          {#if p.genres.length > 0}<div>
              <dt>Genres</dt>
              <dd>{p.genres.join(', ')}</dd>
            </div>{/if}
          {#if p.runtime_minutes !== null && p.runtime_minutes > 0}<div>
              <dt>Laufzeit</dt>
              <dd>{runtimeText(p.runtime_minutes)}</dd>
            </div>{/if}
          {#if p.status !== null}<div>
              <dt>Status</dt>
              <dd>{statusText(p.status)}</dd>
            </div>{/if}
        </dl>
      </div>
    </section>

    {#if p.seasons.length > 0}
      <section aria-labelledby="seasons-title" class="part">
        <h2 id="seasons-title">Staffeln</h2>
        <ul class="seasons">
          {#each p.seasons as season (season.number)}
            <li>
              {seasonText(
                season.name,
                season.number,
                season.episode_count,
              )}{#if season.air_date !== null}
                <span class="muted">· {dateText(season.air_date)}</span>{/if}
            </li>
          {/each}
        </ul>
      </section>
    {/if}
    {#if p.cast.length > 0}
      <section aria-labelledby="cast-title" class="part">
        <h2 id="cast-title">Besetzung</h2>
        <PeopleList
          label="Besetzung"
          people={p.cast.map((c) => ({
            id: c.person_id,
            name: c.name,
            role: c.character,
            photo: c.photo,
          }))}
        />
      </section>
    {/if}
    {#if p.crew.length > 0}
      <section aria-labelledby="crew-title" class="part">
        <h2 id="crew-title">Stab</h2>
        <PeopleList
          label="Stab"
          people={p.crew.map((c) => ({
            id: c.person_id,
            name: c.name,
            role: jobText(c.job),
            photo: c.photo,
          }))}
        />
      </section>
    {/if}
    <FreshnessNote freshness={p.freshness} />
    <ImdbAttribution info={discover.imdb} />
  {:else if failure !== null}
    <PageError
      title={failure.status === 404
        ? `${KIND_TEXT[kind]} nicht gefunden`
        : 'TMDB ist nicht verfügbar'}
      message={failure.message}
      action="Erneut versuchen"
      onaction={() => void load()}
    />
  {:else}
    <Loading label={`${KIND_TEXT[kind]} wird geladen …`} />
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
    grid-template-columns: minmax(6rem, 14rem) minmax(0, 1fr);
    gap: var(--base-size-24);
    align-items: start;
  }

  .text,
  .part {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
    min-width: 0;
  }

  h1 {
    overflow-wrap: anywhere;
  }

  h2,
  p {
    margin: 0;
  }

  .year {
    color: var(--fgColor-muted);
    font-weight: var(--base-text-weight-normal);
  }

  .overview {
    max-width: 50rem;
  }

  .labels,
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .seasons {
    margin: 0;
    padding-left: var(--base-size-20, 1.25rem);
  }

  @media (max-width: 543.98px) {
    .head {
      grid-template-columns: minmax(0, 1fr);
    }

    .poster {
      max-width: 9rem;
    }
  }
</style>
