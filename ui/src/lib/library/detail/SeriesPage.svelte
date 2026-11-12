<script lang="ts">
  import { onMount } from 'svelte';
  import Loading from '../../components/Loading.svelte';
  import PageError from '../../components/PageError.svelte';
  import { changeSeries, getSeries, previewSeriesChange } from '../../api/library';
  import { ensureOptions, options } from '../options.svelte';
  import { createResource } from '../../resource.svelte';
  import { searchBody } from '../actions';
  import { bytesText } from '../format';
  import { notify } from '../notices.svelte';
  import Label from '../components/Label.svelte';
  import RatingLine from '../../imdb/RatingLine.svelte';
  import DetailHead from './DetailHead.svelte';
  import FilesBox from './FilesBox.svelte';
  import HistoryBox from './HistoryBox.svelte';
  import SeasonBox from './SeasonBox.svelte';
  import SettingsBox from './SettingsBox.svelte';
  import { createItemActions } from './detail.svelte';

  // A series with its seasons and episodes (spec web-ui-library AC-06, AC-09 to AC-13,
  // AC-15 to AC-17).
  type Props = { id: number };
  let { id }: Props = $props();

  const series = createResource(() => getSeries(id));
  const actions = createItemActions(() => void series.reload());

  onMount(() => {
    void ensureOptions();
    return series.start();
  });
</script>

<svelte:head>
  <title>{series.data?.title ?? 'Serie'} – sofarr</title>
</svelte:head>

{#if series.data !== null}
  {@const s = series.data}
  <DetailHead
    kind="series"
    id={s.id}
    title={s.title}
    year={s.year}
    otherTitles={[s.title_de, ...s.alternative_titles].filter(
      (t): t is string => t !== null && t !== s.title,
    )}
    overview={s.overview}
    hasPoster={s.has_poster}
    monitored={s.monitored}
    busy={actions.busy.has('monitor')}
    searching={actions.busy.has('search')}
    onmonitor={() =>
      void actions.setMonitored('monitor', !s.monitored, [{ kind: 'series', id: s.id }])}
    onsearch={() => void actions.search('search', s.title, searchBody({ series_id: s.id }))}
  >
    {#snippet facts()}
      <RatingLine rating={s.imdb_rating} votes={s.imdb_votes} />
      <div class="labels">
        {#if s.missing > 0}<Label text={`${s.missing} fehlen`} tone="danger" />{/if}
        {#if s.cutoff_unmet > 0}<Label text={`${s.cutoff_unmet} unter Cutoff`} />{/if}
        {#if s.complete}<Label text="komplett" tone="success" />{/if}
      </div>
      <dl class="facts">
        <div>
          <dt>Episoden</dt>
          <dd data-testid="episode-counts">
            {s.files} mit Datei / {s.monitored_episodes} überwacht / {s.episodes} gesamt
          </dd>
        </div>
        <div>
          <dt>Größe</dt>
          <dd>{bytesText(s.size)}</dd>
        </div>
        <div>
          <dt>Qualitätsprofil</dt>
          <dd>{s.quality_profile.name}</dd>
        </div>
        <div>
          <dt>Tags</dt>
          <dd>{s.tags.length === 0 ? 'keine' : s.tags.map((t) => t.name).join(', ')}</dd>
        </div>
        <div>
          <dt>Serientyp</dt>
          <dd>{s.series_type}</dd>
        </div>
      </dl>
    {/snippet}
  </DetailHead>

  <section class="seasons" aria-labelledby="seasons-title">
    <h2 id="seasons-title">Staffeln</h2>
    {#each s.seasons as season (season.number)}
      <SeasonBox seriesId={s.id} seriesTitle={s.title} {season} {actions} />
    {/each}
  </section>

  <div class="grid">
    <SettingsBox
      profiles={options.value?.series_profiles ?? []}
      profile={s.quality_profile}
      availability={null}
      preview={(c) => previewSeriesChange(s.id, { quality_profile_id: c.profile })}
      save={async (c) => {
        const r = await changeSeries(s.id, { quality_profile_id: c.profile });
        if (r.ok) {
          notify('info', 'Einstellungen gespeichert; sie gelten ab der nächsten Entscheidung.');
          await series.reload();
        } else if (r.failure.kind !== 'unauthorized') {
          notify('danger', `Nicht gespeichert: ${r.failure.message}`);
        }
        return r.ok;
      }}
    />
    <FilesBox
      kind="series"
      id={s.id}
      title={s.title}
      folder={s.folder}
      fileActions={s.file_actions}
      onchanged={() => void series.reload()}
    />
    <HistoryBox
      kind="series"
      grabs={s.grabs}
      blocklist={s.blocklist}
      onchanged={() => void series.reload()}
    />
  </div>
{:else if series.failure !== null}
  <PageError
    title={series.failure.status === 404 ? 'Serie nicht gefunden' : 'Die Serie ist nicht verfügbar'}
    message={series.failure.message}
    action="Erneut versuchen"
    onaction={() => void series.reload()}
  />
{:else}
  <Loading label="Serie wird geladen …" />
{/if}

<style>
  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
  }

  .seasons {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
    gap: var(--base-size-16);
    align-items: start;
  }
</style>
