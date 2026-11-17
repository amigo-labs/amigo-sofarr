<script lang="ts">
  import prevIcon from '@primer/octicons/build/svg/chevron-left-16.svg?raw';
  import nextIcon from '@primer/octicons/build/svg/chevron-right-16.svg?raw';
  import { onMount } from 'svelte';
  import Icon from '../../components/Icon.svelte';

  // Back and forward buttons for one shelf's row of titles (spec ui-discover-polish AC-03):
  // each scrolls by about the visible width; disabled at the row's ends.
  type Props = { name: string };
  let { name }: Props = $props();

  let host: HTMLElement | undefined = $state();
  let strip: HTMLElement | null = null;
  let atStart = $state(true);
  let atEnd = $state(true);

  function measure(): void {
    if (strip === null) return;
    atStart = strip.scrollLeft <= 1;
    atEnd = strip.scrollLeft + strip.clientWidth >= strip.scrollWidth - 1;
  }

  onMount(() => {
    strip = host?.closest('section')?.querySelector<HTMLElement>('ul.tiles') ?? null;
    if (strip === null) return;
    measure();
    const current = strip;
    current.addEventListener('scroll', measure, { passive: true });
    window.addEventListener('resize', measure);
    return () => {
      current.removeEventListener('scroll', measure);
      window.removeEventListener('resize', measure);
    };
  });

  function go(direction: number): void {
    strip?.scrollBy({ left: direction * strip.clientWidth * 0.9, behavior: 'smooth' });
  }
</script>

<span class="arrows" bind:this={host}>
  <button
    type="button"
    class="am-btn icon"
    aria-label={`Zurück in „${name}“`}
    title="Zurück"
    disabled={atStart}
    onclick={() => go(-1)}
  >
    <Icon svg={prevIcon} />
  </button>
  <button
    type="button"
    class="am-btn icon"
    aria-label={`Weiter in „${name}“`}
    title="Weiter"
    disabled={atEnd}
    onclick={() => go(1)}
  >
    <Icon svg={nextIcon} />
  </button>
</span>

<style>
  .arrows {
    display: inline-flex;
    gap: var(--base-size-4);
  }

  .icon {
    width: var(--control-medium-size);
    padding: 0;
  }
</style>
