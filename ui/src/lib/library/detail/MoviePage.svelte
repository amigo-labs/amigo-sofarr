<script lang="ts">
  import { onMount } from 'svelte';
  import Loading from '../../components/Loading.svelte';
  import PageError from '../../components/PageError.svelte';
  import { changeMovie, getMovie, previewMovieChange } from '../../api/library';
  import { ensureOptions, options } from '../options.svelte';
  import { searchBody } from '../actions';
  import Label from '../components/Label.svelte';
  import DetailHead from './DetailHead.svelte';
  import FileBox from './FileBox.svelte';
  import FilesBox from './FilesBox.svelte';
  import HistoryBox from './HistoryBox.svelte';
  import SettingsBox from './SettingsBox.svelte';
  import { createItemActions } from './detail.svelte';
  import { STAGE_TEXT, availabilityText, dateText } from '../format';
  import { notify } from '../notices.svelte';
  import { createResource } from '../../resource.svelte';

  // A movie (spec web-ui-library AC-05, AC-09 to AC-13, AC-15 to AC-17).
  type Props = { id: number };
  let { id }: Props = $props();

  const movie = createResource(() => getMovie(id));
  const actions = createItemActions(() => void movie.reload());

  onMount(() => {
    void ensureOptions();
    return movie.start();
  });
</script>

<svelte:head>
  <title>{movie.data?.title ?? 'Film'} – sofarr</title>
</svelte:head>

{#if movie.data !== null}
  {@const m = movie.data}
  <DetailHead
    kind="movie"
    id={m.id}
    title={m.title}
    year={m.year}
    otherTitles={[m.title_de, m.original_title, ...m.alternative_titles].filter(
      (t): t is string => t !== null && t !== m.title,
    )}
    overview={m.overview}
    hasPoster={m.has_poster}
    monitored={m.monitored}
    busy={actions.busy.has('monitor')}
    searching={actions.busy.has('search')}
    onmonitor={() =>
      void actions.setMonitored('monitor', !m.monitored, [{ kind: 'movie', id: m.id }])}
    onsearch={() => void actions.search('search', m.title, searchBody({ movie_id: m.id }))}
  >
    {#snippet facts()}
      <div class="labels">
        {#if m.missing}<Label text="fehlt" tone="danger" />{/if}
        {#if m.cutoff_unmet}<Label text="Cutoff nicht erreicht" />{/if}
        <Label
          text={m.available ? 'verfügbar' : 'noch nicht verfügbar'}
          tone={m.available ? 'success' : 'neutral'}
        />
      </div>
      <dl class="facts">
        <div>
          <dt>Status</dt>
          <dd data-testid="stage">{STAGE_TEXT[m.stage]}</dd>
        </div>
        <div>
          <dt>Mindestverfügbarkeit</dt>
          <dd>{availabilityText(m.minimum_availability)}</dd>
        </div>
        <div>
          <dt>Im Kino</dt>
          <dd>{dateText(m.release_dates.in_cinemas)}</dd>
        </div>
        <div>
          <dt>Digital</dt>
          <dd>{dateText(m.release_dates.digital)}</dd>
        </div>
        <div>
          <dt>Disc</dt>
          <dd>{dateText(m.release_dates.physical)}</dd>
        </div>
        <div>
          <dt>Qualitätsprofil</dt>
          <dd>{m.quality_profile.name}</dd>
        </div>
        <div>
          <dt>Tags</dt>
          <dd>{m.tags.length === 0 ? 'keine' : m.tags.map((t) => t.name).join(', ')}</dd>
        </div>
        {#if m.runtime_minutes > 0}<div>
            <dt>Laufzeit</dt>
            <dd>{m.runtime_minutes} Min.</dd>
          </div>{/if}
      </dl>
    {/snippet}
  </DetailHead>

  <div class="grid">
    <FileBox file={m.file} cutoffReason={m.cutoff_reason} />
    <SettingsBox
      profiles={options.value?.movie_profiles ?? []}
      profile={m.quality_profile}
      availability={m.minimum_availability}
      preview={(c) =>
        previewMovieChange(m.id, {
          quality_profile_id: c.profile,
          minimum_availability: c.availability,
        })}
      save={async (c) => {
        const r = await changeMovie(m.id, {
          quality_profile_id: c.profile,
          minimum_availability: c.availability,
        });
        if (r.ok) {
          notify('info', 'Einstellungen gespeichert; sie gelten ab der nächsten Entscheidung.');
          await movie.reload();
        } else if (r.failure.kind !== 'unauthorized') {
          notify('danger', `Nicht gespeichert: ${r.failure.message}`);
        }
        return r.ok;
      }}
    />
    <FilesBox
      kind="movie"
      id={m.id}
      title={m.title}
      folder={m.folder}
      fileActions={m.file_actions}
      onchanged={() => void movie.reload()}
    />
    <HistoryBox
      kind="movie"
      grabs={m.grabs}
      blocklist={m.blocklist}
      onchanged={() => void movie.reload()}
    />
  </div>
{:else if movie.failure !== null}
  <PageError
    title={movie.failure.status === 404 ? 'Film nicht gefunden' : 'Der Film ist nicht verfügbar'}
    message={movie.failure.message}
    action="Erneut versuchen"
    onaction={() => void movie.reload()}
  />
{:else}
  <Loading label="Film wird geladen …" />
{/if}

<style>
  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
    gap: var(--base-size-16);
    align-items: start;
  }
</style>
