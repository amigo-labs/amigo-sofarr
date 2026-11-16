<script lang="ts">
  import { onMount } from 'svelte';
  import moonIcon from '@primer/octicons/build/svg/moon-16.svg?raw';
  import sunIcon from '@primer/octicons/build/svg/sun-16.svg?raw';
  import systemIcon from '@primer/octicons/build/svg/device-desktop-16.svg?raw';
  import {
    DEFAULT_CHOICE,
    THEME_CHOICES,
    applyChoice,
    readChoice,
    storeChoice,
    type ThemeChoice,
  } from '../theme/theme';
  import Icon from './Icon.svelte';

  // The theme switch (specs web-ui-shell AC-10, amigo-theme AC-05, ui-shell-redesign AC-09):
  // three buttons, the pressed one is the active choice. `compact` stacks them as icons, for
  // the collapsed sidebar; their names stay.
  type Props = { compact?: boolean };
  let { compact = false }: Props = $props();

  const ICONS: Record<ThemeChoice, string> = { dark: moonIcon, light: sunIcon, system: systemIcon };
  const storage = (): Storage => window.localStorage;
  let choice: ThemeChoice = $state(DEFAULT_CHOICE);

  onMount(() => {
    choice = readChoice(storage);
    applyChoice(document.documentElement, choice);
  });

  function pick(value: ThemeChoice): void {
    choice = value;
    applyChoice(document.documentElement, choice);
    storeChoice(storage, choice);
  }
</script>

<div class="am-seg theme-switch" class:compact role="group" aria-label="Farbschema">
  {#each THEME_CHOICES as option (option.value)}
    <button
      type="button"
      aria-pressed={choice === option.value}
      title={compact ? option.label : undefined}
      onclick={() => pick(option.value)}
    >
      <Icon svg={ICONS[option.value]} />
      <span class:sr-only={compact}>{option.label}</span>
    </button>
  {/each}
</div>

<style>
  .theme-switch button {
    display: inline-flex;
    flex: 1 1 auto;
    align-items: center;
    justify-content: center;
    gap: var(--base-size-4);
    min-width: 0;
    padding: 0 6px;
    white-space: nowrap;
    overflow-wrap: normal;
  }

  .theme-switch:not(.compact) {
    display: flex;
  }

  .compact {
    flex-direction: column;
  }

  .compact button {
    padding: 0;
    width: 28px;
  }
</style>
