<script lang="ts">
  // An Octicon from `@primer/octicons/build/svg/<name>.svg?raw`; decorative, so hidden from
  // screen readers. A control with only an icon carries its own German accessible name.
  type Props = { svg: string };
  let { svg }: Props = $props();
</script>

<span class="icon" aria-hidden="true">{@html svg}</span>

<style>
  .icon {
    display: inline-flex;
    flex: none;
    line-height: 0;
  }

  .icon :global(svg) {
    fill: currentColor;
  }
</style>
