<script lang="ts">
  import { onMount } from 'svelte';
  import sunIcon from '@primer/octicons/build/svg/sun-16.svg?raw';
  import {
    THEME_CHOICES,
    applyChoice,
    parseChoice,
    readChoice,
    storeChoice,
    type ThemeChoice,
  } from '../theme/theme';
  import Icon from './Icon.svelte';

  // The color scheme switch (spec web-ui-shell AC-10).
  const storage = (): Storage => window.localStorage;
  let choice: ThemeChoice = $state('system');

  onMount(() => {
    choice = readChoice(storage);
    applyChoice(document.documentElement, choice);
  });

  function change(event: Event & { currentTarget: HTMLSelectElement }): void {
    choice = parseChoice(event.currentTarget.value);
    applyChoice(document.documentElement, choice);
    storeChoice(storage, choice);
  }
</script>

<label class="theme-switch">
  <Icon svg={sunIcon} />
  <span class="sr-only">Farbschema</span>
  <select value={choice} onchange={change}>
    {#each THEME_CHOICES as option (option.value)}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
</label>

<style>
  .theme-switch {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-4);
    color: var(--header-fgColor-logo);
  }

  select {
    min-height: 2rem;
    padding: 0 var(--base-size-4);
    border: var(--borderWidth-thin) solid var(--header-fgColor-default);
    border-radius: var(--borderRadius-medium);
    background: var(--header-bgColor);
    color: var(--header-fgColor-logo);
    font: inherit;
  }
</style>
