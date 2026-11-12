<script lang="ts">
  import onIcon from '@primer/octicons/build/svg/bookmark-fill-16.svg?raw';
  import offIcon from '@primer/octicons/build/svg/bookmark-16.svg?raw';
  import Icon from '../../components/Icon.svelte';

  // Switches the monitoring of one item; saved at once by the caller (AC-10). A toggle:
  // pressed while monitored; its accessible name starts with the visible text.
  type Props = { monitored: boolean; title: string; busy?: boolean; ontoggle: () => void };
  let { monitored, title, busy = false, ontoggle }: Props = $props();
</script>

<button
  type="button"
  class="monitor"
  class:on={monitored}
  aria-pressed={monitored}
  title={monitored
    ? 'Wird überwacht – klicken zum Ausschalten'
    : 'Wird nicht überwacht – klicken zum Einschalten'}
  disabled={busy}
  onclick={ontoggle}
>
  <Icon svg={monitored ? onIcon : offIcon} />
  <span>Überwachen</span><span class="sr-only"> „{title}“</span>
</button>

<style>
  .monitor {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-4);
    min-height: 2rem;
    padding: 0 var(--base-size-8);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-default);
    color: var(--fgColor-muted);
    font: inherit;
    font-size: var(--text-body-size-small);
    cursor: pointer;
    white-space: nowrap;
  }

  .monitor.on {
    color: var(--fgColor-accent);
    border-color: var(--borderColor-accent-emphasis);
  }

  .monitor:disabled {
    cursor: progress;
  }
</style>
