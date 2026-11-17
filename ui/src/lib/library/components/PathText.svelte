<script lang="ts">
  import { pathPieces } from '../format';

  // A folder or file path in the monospace font that breaks after its separators first and
  // inside a name only when the name alone is wider than its box; the full path is its
  // tooltip (ui-consistency AC-12, ui-detail-boxes AC-03). Each piece is an inline block:
  // lines break between pieces, and inside one only when it does not fit a line by itself.
  type Props = { path: string; testid?: string; prefix?: string };
  let { path, testid, prefix = '' }: Props = $props();
</script>

<span class="mono path" data-testid={testid} title={path}
  >{prefix}{#each pathPieces(path) as piece, i (i)}<span class="piece">{piece}</span>{/each}</span
>

<style>
  .piece {
    display: inline-block;
    max-width: 100%;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
</style>
