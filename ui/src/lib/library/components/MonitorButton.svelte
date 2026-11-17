<script lang="ts">
  import onIcon from '@primer/octicons/build/svg/bookmark-fill-16.svg?raw';
  import offIcon from '@primer/octicons/build/svg/bookmark-16.svg?raw';
  import Icon from '../../components/Icon.svelte';

  // Switches the monitoring of one item; saved at once by the caller (AC-10). A toggle:
  // pressed while monitored; its accessible name starts with the visible text. `icon` shows
  // only the bookmark on its own ground, for the library lists (spec ui-list-items AC-04).
  type Props = {
    monitored: boolean;
    title: string;
    busy?: boolean;
    icon?: boolean;
    ontoggle: () => void;
  };
  let { monitored, title, busy = false, icon = false, ontoggle }: Props = $props();
</script>

<button
  type="button"
  class="monitor"
  class:on={monitored}
  class:icon
  aria-pressed={monitored}
  title={monitored
    ? 'Wird überwacht – klicken zum Ausschalten'
    : 'Wird nicht überwacht – klicken zum Einschalten'}
  disabled={busy}
  onclick={ontoggle}
>
  <Icon svg={monitored ? onIcon : offIcon} />
  <span class:sr-only={icon}>Überwachen</span><span class="sr-only"> „{title}“</span>
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
    background: var(--button-default-bgColor-rest);
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

  .monitor.icon {
    justify-content: center;
    width: 2rem;
    height: 2rem;
    padding: 0;
    border-color: transparent;
    border-radius: var(--borderRadius-full);
    background: color-mix(in srgb, var(--am-surface) 85%, transparent);
    color: var(--fgColor-default);
  }

  .monitor.icon.on {
    color: var(--fgColor-accent);
    border-color: transparent;
  }
</style>
