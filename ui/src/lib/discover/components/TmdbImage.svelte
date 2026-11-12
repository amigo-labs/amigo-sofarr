<script lang="ts">
  import { initial } from '../../library/rows';

  // A poster or photo from sofarr's image cache (spec discover AC-17, D24), or a neutral
  // placeholder with the initial. The address comes from the API answer.
  type Props = { src: string | null; name: string; shape?: 'poster' | 'photo'; lazy?: boolean };
  let { src, name, shape = 'poster', lazy = true }: Props = $props();

  let failed = $state(false);
</script>

<div class="image {shape}">
  {#if src !== null && !failed}
    <img
      {src}
      alt=""
      loading={lazy ? 'lazy' : 'eager'}
      decoding="async"
      onerror={() => (failed = true)}
    />
  {:else}
    <span class="placeholder" aria-hidden="true">{initial(name)}</span>
  {/if}
</div>

<style>
  .image {
    position: relative;
    width: 100%;
    overflow: hidden;
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-muted);
  }

  .poster {
    aspect-ratio: 2 / 3;
  }

  .photo {
    aspect-ratio: 2 / 3;
    border-radius: var(--borderRadius-medium);
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
    font-size: 1.75em;
    font-weight: var(--base-text-weight-semibold);
  }
</style>
