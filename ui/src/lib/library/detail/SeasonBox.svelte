<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import Icon from '../../components/Icon.svelte';
  import type { SeasonAnswer } from '../../api/generated/SeasonAnswer';
  import { searchBody } from '../actions';
  import { dateText, episodeCode } from '../format';
  import Label from '../components/Label.svelte';
  import MonitorButton from '../components/MonitorButton.svelte';
  import type { ItemActions } from './detail.svelte';

  // One season with its episode counts and episodes (spec web-ui-library AC-06, AC-10).
  type Props = {
    seriesId: number;
    seriesTitle: string;
    season: SeasonAnswer;
    actions: ItemActions;
  };
  let { seriesId, seriesTitle, season, actions }: Props = $props();

  const name = $derived(season.number === 0 ? 'Specials' : `Staffel ${season.number}`);
  const key = $derived(`season-${season.number}`);
</script>

<details class="box season" open={season.missing > 0 || season.number === 1}>
  <summary class="box-header">
    <span class="name">{name}</span>
    <span class="muted counts" data-testid="season-counts">
      {season.episodes_with_file} mit Datei / {season.episodes_monitored} überwacht / {season.episodes_total}
      gesamt
    </span>
    {#if season.missing > 0}<Label text={`${season.missing} fehlen`} tone="danger" />{/if}
    {#if season.cutoff_unmet > 0}<Label text={`${season.cutoff_unmet} unter Cutoff`} />{/if}
  </summary>
  <div class="box-row season-actions">
    <MonitorButton
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
      class="btn"
      disabled={actions.busy.has(`${key}-search`)}
      onclick={() =>
        void actions.search(
          `${key}-search`,
          `${seriesTitle} – ${name}`,
          searchBody({ series_id: seriesId, season: season.number }),
        )}
    >
      <Icon svg={searchIcon} />{name} suchen
    </button>
  </div>
  <ul class="episodes">
    {#each season.episodes as episode (episode.id)}
      {@const code = episodeCode(episode.season, episode.number)}
      <li class="box-row episode" data-episode={code}>
        <div class="main">
          <span><strong>{code}</strong> {episode.title ?? 'ohne Titel'}</span>
          <span class="muted small">
            {episode.air_date === null
              ? 'Sendetermin unbekannt'
              : `Ausgestrahlt ${dateText(episode.air_date)}`}
            · {episode.has_file ? episode.quality : 'fehlt'}
          </span>
          <span class="labels">
            {#if episode.missing}<Label text="fehlt" tone="danger" />{/if}
            {#if episode.cutoff_unmet}<Label text="Cutoff nicht erreicht" />{/if}
          </span>
        </div>
        <div class="buttons">
          <MonitorButton
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
            class="btn icon-only"
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
        </div>
      </li>
    {/each}
  </ul>
</details>

<style>
  summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-4) var(--base-size-12);
    cursor: pointer;
  }

  .name {
    font-weight: var(--base-text-weight-semibold);
  }

  .counts,
  .small {
    font-size: var(--text-body-size-small);
  }

  .season-actions,
  .buttons,
  .labels {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  .episodes {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .episode {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: var(--base-size-8);
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .icon-only {
    padding: 0 var(--base-size-8);
  }
</style>
