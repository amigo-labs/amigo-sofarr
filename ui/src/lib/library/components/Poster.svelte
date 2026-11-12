<script lang="ts">
  import type { MediaKind } from '../../api/generated/MediaKind';
  import { posterAddress } from '../../api/library';
  import { initial } from '../rows';

  // A poster from sofarr's image cache, or a neutral placeholder with the initial
  // (spec web-ui-library AC-20, AC-21). Images come from sofarr only.
  type Props = {
    kind: MediaKind;
    id: number;
    title: string;
    hasPoster: boolean;
    size: 'list' | 'detail';
  };
  let { kind, id, title, hasPoster, size }: Props = $props();

  let failed = $state(false);
  const show = $derived(hasPoster && !failed);
</script>

<div class="poster poster-{size}">
  {#if show}
    <img
      src={posterAddress(kind, id, size)}
      alt=""
      loading={size === 'list' ? 'lazy' : 'eager'}
      decoding="async"
      onerror={() => (failed = true)}
    />
  {:else}
    <span class="placeholder" data-testid="poster-placeholder" aria-hidden="true"
      >{initial(title)}</span
    >
  {/if}
</div>

<style>
  .poster {
    position: relative;
    flex: none;
    overflow: hidden;
    aspect-ratio: 2 / 3;
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-muted);
  }

  .poster-list {
    width: 100%;
  }

  .poster-detail {
    width: 100%;
    max-width: 14rem;
  }

  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    color: var(--fgColor-muted);
    font-size: 2em;
    font-weight: var(--base-text-weight-semibold);
  }
</style>
