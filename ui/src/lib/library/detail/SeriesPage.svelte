<script lang="ts">
  import { onMount } from 'svelte';
  import Loading from '../../components/Loading.svelte';
  import PageError from '../../components/PageError.svelte';
  import { changeSeries, getSeries, previewSeriesChange } from '../../api/library';
  import { ensureOptions, options } from '../options.svelte';
  import { createResource } from '../../resource.svelte';
  import { searchBody } from '../actions';
  import { bytesText, missingText, seriesTypeText } from '../format';
  import { notify } from '../notices.svelte';
  import Label from '../components/Label.svelte';
  import type { Freshness } from '../../api/generated/Freshness';
  import CastBox from '../../discover/CastBox.svelte';
  import FreshnessNote from '../../discover/components/FreshnessNote.svelte';
  import RatingLine from '../../imdb/RatingLine.svelte';
  import ImdbAttribution from '../../imdb/ImdbAttribution.svelte';
  import { imdb } from '../../imdb/info.svelte';
  import TrailerBox from '../../trailers/TrailerBox.svelte';
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

  // When the cast was fetched; its line stands at the page's end (spec ui-polish AC-10).
  let castFreshness: Freshness | null = $state(null);

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
    {#snippet chips()}
      {#if s.missing > 0}<Label text={missingText(s.missing)} tone="danger" />{/if}
      {#if s.cutoff_unmet > 0}<Label text={`${s.cutoff_unmet} unter Cutoff`} />{/if}
      {#if s.complete}<Label text="komplett" tone="success" />{/if}
    {/snippet}
    {#snippet rating()}
      <RatingLine rating={s.imdb_rating} votes={s.imdb_votes} attribution={false} />
    {/snippet}
    {#snippet facts()}
      <dl class="facts facts-compact">
        <div>
          <dt>Episoden</dt>
          <dd class="counts" data-testid="episode-counts">
            <span>{s.files} von {s.episodes} mit Datei</span>
            <span class="muted">{s.monitored_episodes} überwacht</span>
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
          <dd>{seriesTypeText(s.series_type)}</dd>
        </div>
      </dl>
    {/snippet}
  </DetailHead>

  <CastBox kind="series" id={s.id} bind:freshness={castFreshness} />

  <div class="detail-columns">
    <div class="detail-main">
      <section class="seasons" aria-labelledby="seasons-title">
        <h2 id="seasons-title">Staffeln</h2>
        {#each s.seasons as season (season.number)}
          <SeasonBox seriesId={s.id} seriesTitle={s.title} {season} {actions} />
        {/each}
      </section>
      <HistoryBox
        kind="series"
        grabs={s.grabs}
        blocklist={s.blocklist}
        onchanged={() => void series.reload()}
      />
    </div>
    <div class="detail-side">
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
      <TrailerBox kind="series" id={s.id} title={s.title} />
    </div>
  </div>
  <FreshnessNote part="line" freshness={castFreshness} />
  <ImdbAttribution info={imdb.info} />
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
  .counts {
    display: flex;
    flex-direction: column;
  }

  .seasons {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
  }

  .seasons h2 {
    font-size: var(--text-title-size-small);
  }
</style>
