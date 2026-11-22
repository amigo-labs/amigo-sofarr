<script lang="ts">
  import PosterTile from '../../components/PosterTile.svelte';
  import type { HomeTile } from '../home';
  import Poster from './Poster.svelte';

  // A library title in a row of the overview (spec ui-cinema-home AC-01, AC-02): its poster
  // or the library's placeholder, the state mark, and in the overlay title, year and, in
  // „Demnächst“, the day with the episode code.
  type Props = { tile: HomeTile };
  let { tile }: Props = $props();
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
</PosterTile>
