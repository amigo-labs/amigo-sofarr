<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import chevronIcon from '@primer/octicons/build/svg/chevron-right-16.svg?raw';
  import Icon from '../../components/Icon.svelte';
  import type { SeasonAnswer } from '../../api/generated/SeasonAnswer';
  import { searchBody } from '../actions';
  import { dateText, episodeCode, missingText } from '../format';
  import Label from '../components/Label.svelte';
  import MonitorButton from '../components/MonitorButton.svelte';
  import EpisodeSubtitles from '../../subtitles/EpisodeSubtitles.svelte';
  import EpisodeFile from './EpisodeFile.svelte';
  import type { ItemActions } from './detail.svelte';

  // One season with its episode counts and episodes (spec web-ui-library AC-06, AC-10), the
  // file of each episode (AC-06) and its subtitles (spec subtitles AC-12). A compact header
  // with progress and icon actions, one line per episode (spec ui-detail-boxes AC-04 ..
  // AC-06); the actions sit beside the summary, so they never open or close the season.
  type Props = {
    seriesId: number;
    seriesTitle: string;
    season: SeasonAnswer;
    actions: ItemActions;
  };
  let { seriesId, seriesTitle, season, actions }: Props = $props();

  const name = $derived(season.number === 0 ? 'Specials' : `Staffel ${season.number}`);
  const key = $derived(`season-${season.number}`);
  const share = $derived(
    season.episodes_total === 0 ? 0 : (100 * season.episodes_with_file) / season.episodes_total,
  );
</script>

<details class="box season" open={season.missing > 0 || season.number === 1}>
  <summary class="box-header">
    <span class="chevron"><Icon svg={chevronIcon} /></span>
    <span class="season-name">{name}</span>
    <span
      class="progress"
      role="progressbar"
      aria-label={`${name}: Episoden mit Datei`}
      aria-valuemin={0}
      aria-valuemax={season.episodes_total}
      aria-valuenow={season.episodes_with_file}><i style:width="{share}%"></i></span
    >
    <span class="progress-text">{season.episodes_with_file} von {season.episodes_total}</span>
    <span class="muted counts" data-testid="season-counts">
      {season.episodes_with_file} mit Datei / {season.episodes_monitored} überwacht / {season.episodes_total}
      gesamt
    </span>
    {#if season.missing > 0}<Label text={missingText(season.missing)} tone="danger" />{/if}
    {#if season.cutoff_unmet > 0}<Label text={`${season.cutoff_unmet} unter Cutoff`} />{/if}
  </summary>
  <div class="season-actions">
    <MonitorButton
      icon
      monitored={season.monitored}
      title={`${seriesTitle} – ${name}`}
      busy={actions.busy.has(key)}
      ontoggle={() =>
        void actions.setMonitored(key, !season.monitored, [
          { kind: 'season', series_id: seriesId, season: season.number },
        ])}
    />
    <button
      type="button"
      class="icon-button"
      aria-label={`${name} suchen`}
      title={`${name} suchen`}
      disabled={actions.busy.has(`${key}-search`)}
      onclick={() =>
        void actions.search(
          `${key}-search`,
          `${seriesTitle} – ${name}`,
          searchBody({ series_id: seriesId, season: season.number }),
        )}
    >
      <Icon svg={searchIcon} />
    </button>
  </div>
  <ul class="episodes">
    {#each season.episodes as episode (episode.id)}
      {@const code = episodeCode(episode.season, episode.number)}
      <li class="box-row episode" data-episode={code}>
        <div class="line">
          <span class="code">{code}</span>
          <span class="episode-title">{episode.title ?? 'ohne Titel'}</span>
          <span class="air muted">
            {episode.air_date === null
              ? 'Sendetermin unbekannt'
              : `Ausgestrahlt ${dateText(episode.air_date)}`}
          </span>
          <span class="labels">
            {#if episode.has_file}
              <Label text={episode.quality ?? 'Datei'} tone="accent" />
            {:else if episode.missing}
              <Label text="fehlt" tone="danger" />
            {:else}
              <Label text="keine Datei" />
            {/if}
            {#if episode.cutoff_unmet}<Label text="Cutoff nicht erreicht" />{/if}
          </span>
          <span class="buttons">
            <MonitorButton
              icon
              monitored={episode.monitored}
              title={`${seriesTitle} ${code}`}
              busy={actions.busy.has(`episode-${episode.id}`)}
              ontoggle={() =>
                void actions.setMonitored(`episode-${episode.id}`, !episode.monitored, [
                  { kind: 'episode', id: episode.id },
                ])}
            />
            <button
              type="button"
              class="icon-button"
              aria-label={`${seriesTitle} ${code} suchen`}
              title={`${code} suchen`}
              disabled={actions.busy.has(`episode-${episode.id}-search`)}
              onclick={() =>
                void actions.search(
                  `episode-${episode.id}-search`,
                  `${seriesTitle} ${code}`,
                  searchBody({
                    series_id: seriesId,
                    season: episode.season,
                    episode: episode.number,
                  }),
                )}
            >
              <Icon svg={searchIcon} />
            </button>
          </span>
        </div>
        {#if episode.has_file}
          <EpisodeFile {seriesId} episodeId={episode.id} title={`${seriesTitle} ${code}`} />
          <EpisodeSubtitles {seriesId} episodeId={episode.id} title={`${seriesTitle} ${code}`} />
        {/if}
      </li>
    {/each}
  </ul>
</details>

<style>
  .season {
    position: relative;
  }

  summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-4) var(--base-size-12);
    padding-right: 6rem;
    list-style: none;
    cursor: pointer;
  }

  summary::-webkit-details-marker {
    display: none;
  }

  .chevron {
    display: inline-flex;
    color: var(--fgColor-muted);
    transition: transform var(--am-dur-fast);
  }

  .season[open] > summary .chevron {
    transform: rotate(90deg);
  }

  .season-name {
    font-weight: var(--base-text-weight-semibold);
  }

  .progress {
    display: block;
    flex: 0 1 8rem;
    height: 6px;
    min-width: 3rem;
    overflow: hidden;
    border-radius: var(--borderRadius-full);
    background: var(--am-active);
  }

  .progress i {
    display: block;
    height: 100%;
    border-radius: var(--borderRadius-full);
    background: var(--am-accent);
  }

  .progress-text,
  .counts,
  .air {
    font-size: var(--text-body-size-small);
    font-weight: var(--base-text-weight-normal);
  }

  .counts {
    color: var(--fgColor-caption);
  }

  .season-actions {
    position: absolute;
    top: 7px;
    right: var(--base-size-12);
    display: flex;
    gap: var(--base-size-4);
  }

  .episodes {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .episode {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    padding-block: var(--base-size-8);
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-4) var(--base-size-12);
    min-width: 0;
  }

  .code {
    font-family: var(--fontStack-monospace);
    font-size: var(--text-body-size-small);
    font-weight: var(--base-text-weight-semibold);
  }

  .episode-title {
    flex: 1 1 10rem;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
  }

  .buttons {
    display: flex;
    gap: var(--base-size-4);
    margin-left: auto;
  }

  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    padding: 0;
    border: 0;
    border-radius: var(--borderRadius-full);
    background: transparent;
    color: var(--fgColor-muted);
    cursor: pointer;
  }

  .icon-button:hover:not(:disabled) {
    background: var(--button-default-bgColor-hover);
    color: var(--fgColor-default);
  }

  .icon-button:disabled {
    cursor: progress;
    opacity: 0.6;
  }
</style>
