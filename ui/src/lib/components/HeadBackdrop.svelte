<script lang="ts">
  // The ground of a title page's head (spec ui-cinema-backdrops AC-05, AC-06): the title's
  // backdrop, sharp, under a scrim in the page ground that keeps the head's text readable
  // whatever the image; without one (or when it fails to load) the poster, blurred beyond
  // recognition (spec ui-polish AC-01); without that, nothing. Decorative, and absolutely
  // placed: the head never waits for the image and never changes its size with it (AC-08).
  type Props = { image: string | null; fallback?: string | null };
  let { image, fallback = null }: Props = $props();

  let failed = $state<string | null>(null);
  let fallbackFailed = $state(false);
  const sharp = $derived(image !== null && failed !== image);
</script>

{#if sharp}
  <div class="backdrop" aria-hidden="true" data-testid="head-backdrop" data-kind="sharp">
    <img src={image} alt="" decoding="async" onerror={() => (failed = image)} />
    <div class="scrim"></div>
  </div>
{:else if fallback !== null && !fallbackFailed}
  <div class="backdrop blurred" aria-hidden="true" data-testid="head-backdrop" data-kind="blurred">
    <img src={fallback} alt="" decoding="async" onerror={() => (fallbackFailed = true)} />
    <div class="veil"></div>
  </div>
{/if}

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: -1;
    overflow: hidden;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center top;
  }

  /* The scrim: strong under the head's content (the poster column and a measure of 42rem),
     then quickly lighter, so the image shows beside it, and the full page ground at the
     lower edge. */
  .scrim {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(to bottom, transparent 55%, var(--am-bg) 100%),
      linear-gradient(
        to right,
        color-mix(in srgb, var(--am-bg) 84%, transparent) 0,
        color-mix(in srgb, var(--am-bg) 84%, transparent) calc(14rem + 42rem + 4.5rem),
        color-mix(in srgb, var(--am-bg) 15%, transparent) calc(14rem + 42rem + 4.5rem + 16rem),
        color-mix(in srgb, var(--am-bg) 15%, transparent) 100%
      );
  }

  /* Blurred far beyond any shape of the poster, so only its colours remain as an even wash
     (spec ui-polish AC-01); scaled up, so the blur's soft edge lies outside the head. */
  .blurred img {
    object-position: center;
    filter: blur(clamp(56px, 9vw, 120px)) saturate(1.2);
    transform: scale(1.6);
  }

  .veil {
    position: absolute;
    inset: 0;
    background: var(--am-bg);
    opacity: 0.78;
  }

  .blurred::after {
    content: '';
    position: absolute;
    inset: auto 0 0;
    height: 35%;
    background: linear-gradient(transparent, var(--am-bg));
  }
</style>
