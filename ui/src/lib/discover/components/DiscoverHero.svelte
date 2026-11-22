<script lang="ts">
  import plusIcon from '@primer/octicons/build/svg/plus-16.svg?raw';
  import { tick } from 'svelte';
  import HeadBackdrop from '../../components/HeadBackdrop.svelte';
  import Icon from '../../components/Icon.svelte';
  import Label from '../../library/components/Label.svelte';
  import { mediaHref } from '../../library/actions';
  import type { Hero } from '../../api/generated/Hero';
  import { previewHref } from '../links';
  import type { State } from '../states.svelte';
  import { KIND_TEXT, ratingCompact } from '../texts';

  // The hero of the discover start page (spec ui-cinema-hero AC-02, AC-05): the featured
  // title on its backdrop under the scrim of the title pages, from 768 px the text on the
  // image, below it the image as a band above the text.
  type Props = {
    hero: Hero;
    state: State;
    onadd: (hero: Hero) => void;
  };
  let { hero, state: current, onadd }: Props = $props();

  const rating = $derived(ratingCompact(current.rating));
  const libraryId = $derived(current.library.status === 'in_library' ? current.library.id : null);

  let libraryLink: HTMLAnchorElement | undefined = $state();
  /** The add dialog was opened from the hero's button. */
  let adding = false;

  function add(): void {
    adding = true;
    onadd(hero);
  }

  // Added from the hero: the button gives way to the library state, and the focus, which
  // the closing dialog cannot return to the button, moves to the library link.
  $effect(() => {
    if (libraryId === null || !adding) return;
    adding = false;
    void tick().then(() =>
      requestAnimationFrame(() => {
        const active = document.activeElement;
        if (active === null || active === document.body) libraryLink?.focus();
      }),
    );
  });
</script>

<section class="hero" aria-labelledby="hero-title" data-testid="hero">
  <HeadBackdrop image={hero.backdrop} band={true} />
  <div class="text">
    <h2 id="hero-title">{hero.title}</h2>
    <p class="meta" data-testid="hero-meta">
      {KIND_TEXT[
        hero.kind
      ]}{#if hero.year !== null}{` · ${hero.year}`}{/if}{#if rating !== null && rating.text !== ''}{' · '}<span
          class="rating"
          role="img"
          aria-label={rating.full}
          title={rating.full}>{rating.text}</span
        >{/if}
    </p>
    {#if hero.overview !== null}<p class="overview">{hero.overview}</p>{/if}
    <div class="actions">
      {#if libraryId !== null}
        <Label text="In der Bibliothek" tone="success" />
        <a class="btn" href={mediaHref(hero.kind, libraryId)} bind:this={libraryLink}
          >Zur Bibliothek</a
        >
      {:else}
        <button type="button" class="btn btn-primary" onclick={add}>
          <Icon svg={plusIcon} />Hinzufügen
        </button>
      {/if}
      <a class="btn" href={previewHref(hero.kind, hero.tmdb_id)}
        >Details<span class="sr-only">{` zu „${hero.title}“`}</span></a
      >
    </div>
  </div>
</section>

<style>
  .hero {
    position: relative;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    gap: var(--base-size-16);
    min-width: 0;
    overflow: hidden;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
    min-width: 0;
  }

  /* The text keeps to the measure the scrim is strong under (ui-cinema-backdrops AC-07). */
  .text > * {
    max-width: 42rem;
  }

  h2 {
    margin: 0;
    font-size: var(--text-title-size-large);
    line-height: var(--text-title-line);
    overflow-wrap: anywhere;
  }

  p {
    margin: 0;
  }

  .meta {
    color: var(--fgColor-muted);
  }

  .overview {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  /* From 768 px the text lies on the image, at its lower edge. */
  @media (min-width: 768px) {
    .hero {
      justify-content: flex-end;
      min-height: 360px;
      padding: var(--base-size-48) var(--base-size-24) var(--base-size-24);
      border-radius: var(--borderRadius-large, 0.75rem);
    }
  }

  @media (min-width: 1024px) {
    .hero {
      min-height: clamp(420px, 36vw, 560px);
    }
  }

  @media (max-width: 767.98px) {
    .hero :global([data-testid='head-backdrop']) {
      border-radius: var(--borderRadius-medium, 0.375rem);
    }
  }
</style>
