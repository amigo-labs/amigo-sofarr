<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLLiAttributes } from 'svelte/elements';
  import type { Mark } from './tile';

  // A movie or series as a poster tile (spec ui-cinema-tiles): at rest only its poster with
  // a state mark in the lower left corner; open (the tile under the pointer, else the one
  // with the keyboard focus) it grows a little and shows its information in an overlay over
  // the poster's lower part. Touch devices show the overlay at rest. The link carries the
  // title, year and rating as its name, so the hidden overlay hides nothing from a screen
  // reader. The overlay's controls and the corner controls stay in the Tab order.
  type Props = Omit<HTMLLiAttributes, 'title' | 'children'> & {
    href: string;
    /** The full title: the link's tooltip. */
    title: string;
    /** The link's accessible name (`tileName`). */
    name: string;
    mark: Mark | null;
    /** The poster is dimmed (an unmonitored library title). */
    dimmed?: boolean;
    /** The poster is outlined (a selected library title). */
    selected?: boolean;
    poster: Snippet;
    overlay: Snippet;
    /** Controls in the poster's top corners (checkbox, monitor toggle). */
    controls?: Snippet;
    link?: HTMLAnchorElement | undefined;
  };
  let {
    class: className = '',
    href,
    title,
    name,
    mark,
    dimmed = false,
    selected = false,
    poster,
    overlay,
    controls,
    link = $bindable(),
    ...rest
  }: Props = $props();
</script>

<li {...rest} class={['poster-tile', className]} class:dimmed class:selected>
  <div class="frame">
    <a class="link" {href} {title} aria-label={name} bind:this={link}>
      <span class="poster-wrap">{@render poster()}</span>
    </a>
    <div class="overlay" class:marked={mark !== null} data-testid="tile-overlay">
      {@render overlay()}
    </div>
    {#if mark !== null}
      <span
        class="mark mark-{mark.tone}"
        role="img"
        aria-label={mark.text}
        title={mark.text}
        data-testid="state-mark"
      ></span>
    {/if}
    {@render controls?.()}
  </div>
</li>

<style>
  .poster-tile {
    position: relative;
    min-width: 0;
  }

  /* Its own stacking context at rest; an open tile lies above its neighbours (AC-03). */
  .frame {
    position: relative;
    z-index: 0;
    transition: transform var(--am-dur-fast) ease-out;
  }

  .link {
    display: block;
    border-radius: var(--borderRadius-medium);
    text-decoration: none;
  }

  .poster-wrap {
    display: block;
    border-radius: var(--borderRadius-medium);
  }

  .dimmed .poster-wrap {
    opacity: 0.55;
  }

  .selected .poster-wrap {
    outline: 2px solid var(--am-accent);
    outline-offset: 2px;
  }

  /* Over the poster's lower part, never over its top corners' controls (AC-07): a gradient
     from transparent to the page ground above the text, the full ground behind it (AC-04).
     Clicks on its text reach the link below. */
  .overlay {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: flex-start;
    max-height: calc(100% - 2.5rem);
    overflow: hidden;
    padding: 1.5rem var(--base-size-8) var(--base-size-8);
    border-radius: 0 0 var(--borderRadius-medium) var(--borderRadius-medium);
    background: linear-gradient(to bottom, transparent 0, var(--am-bg) 1.5rem);
    color: var(--fgColor-default);
    font-size: var(--text-body-size-small);
    pointer-events: none;
  }

  /* Room for the state mark below the text. */
  .overlay.marked {
    padding-bottom: calc(var(--base-size-8) + 0.75rem + 0.375rem);
  }

  .overlay :global(:is(a, button, input, label)) {
    pointer-events: auto;
  }

  .overlay :global(.title) {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    max-width: 100%;
    overflow: hidden;
    font-size: var(--text-body-size-medium);
    font-weight: var(--base-text-weight-semibold);
    overflow-wrap: anywhere;
  }

  .overlay :global(.meta) {
    max-width: 100%;
    color: var(--fgColor-muted);
    overflow-wrap: anywhere;
  }

  .overlay :global(.rating) {
    color: var(--fgColor-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* The state at rest: a dot with a ring of the page ground, readable on any poster. */
  .mark {
    position: absolute;
    bottom: var(--base-size-8);
    left: var(--base-size-8);
    z-index: 2;
    width: 0.75rem;
    height: 0.75rem;
    border-radius: var(--borderRadius-full);
    box-shadow: 0 0 0 2px var(--am-bg);
  }

  .mark-accent {
    background: var(--am-accent);
  }

  .mark-danger {
    background: var(--am-danger);
  }

  /* Hover devices: the overlay only on the open tile, which grows by 7 % (AC-01, AC-03). The
     tile under the pointer is open; while the pointer is over no tile of the list, the one
     with the focus. Touch devices show the overlay at rest (AC-06). */
  @media (hover: hover) {
    .overlay {
      opacity: 0;
      transition: opacity var(--am-dur-fast) ease-out;
    }

    .poster-tile:hover > .frame,
    :global(:not(:has(> .poster-tile:hover))) > .poster-tile:focus-within > .frame {
      z-index: 1;
      transform: scale(1.07);
    }

    .poster-tile:hover .overlay,
    :global(:not(:has(> .poster-tile:hover))) > .poster-tile:focus-within .overlay {
      opacity: 1;
    }
  }

  @media (hover: hover) and (prefers-reduced-motion: reduce) {
    .frame,
    .overlay {
      transition: none;
    }

    .poster-tile:hover > .frame,
    :global(:not(:has(> .poster-tile:hover))) > .poster-tile:focus-within > .frame {
      transform: none;
    }
  }
</style>
