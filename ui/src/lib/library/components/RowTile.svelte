<script lang="ts">
  import PosterTile from '../../components/PosterTile.svelte';
  import type { HomeTile } from '../home';
  import MonitorButton from './MonitorButton.svelte';
  import Poster from './Poster.svelte';

  // A library title in a row of the overview (spec ui-cinema-home AC-01, AC-02): its poster
  // or the library's placeholder, the state mark, and in the overlay title, year and, in
  // „Demnächst“, the day with the episode code. The rows of the movie and series lists
  // (spec ui-cinema-library-rows AC-07) add the monitor toggle in the poster's top right
  // corner, as the full list's tiles have it.
  type Monitor = { monitored: boolean; busy: boolean; ontoggle: () => void };
  type Props = { tile: HomeTile; monitor?: Monitor };
  let { tile, monitor }: Props = $props();
</script>

<PosterTile
  data-key={tile.key}
  href={tile.href}
  title={tile.title}
  name={tile.name}
  mark={tile.mark}
  dimmed={tile.dimmed}
>
  {#snippet poster()}
    <Poster
      kind={tile.kind}
      id={tile.id}
      title={tile.title}
      hasPoster={tile.hasPoster}
      size="list"
    />
  {/snippet}
  {#snippet overlay()}
    <span class="title" aria-hidden="true">{tile.title}</span>
    {#if tile.year !== null}<span class="meta" aria-hidden="true">{tile.year}</span>{/if}
    {#if tile.when !== null}<span class="meta" aria-hidden="true">{tile.when}</span>{/if}
  {/snippet}
  {#snippet controls()}
    {#if monitor !== undefined}
      <span class="monitor-slot">
        <MonitorButton
          icon
          monitored={monitor.monitored}
          title={tile.title}
          busy={monitor.busy}
          ontoggle={monitor.ontoggle}
        />
      </span>
    {/if}
  {/snippet}
</PosterTile>

<style>
  .monitor-slot {
    position: absolute;
    top: var(--base-size-4);
    right: var(--base-size-4);
    z-index: 2;
    display: inline-flex;
  }
</style>
