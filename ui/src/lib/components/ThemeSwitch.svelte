<script lang="ts">
  import { onMount } from 'svelte';
  import sunIcon from '@primer/octicons/build/svg/sun-16.svg?raw';
  import {
    DEFAULT_CHOICE,
    THEME_CHOICES,
    applyChoice,
    parseChoice,
    readChoice,
    storeChoice,
    type ThemeChoice,
  } from '../theme/theme';
  import Icon from './Icon.svelte';

  // The theme switch (specs web-ui-shell AC-10, amigo-theme AC-05).
  const storage = (): Storage => window.localStorage;
  let choice: ThemeChoice = $state(DEFAULT_CHOICE);

  onMount(() => {
    choice = readChoice(storage);
    applyChoice(document.documentElement, choice);
  });

  function change(event: Event & { currentTarget: HTMLSelectElement }): void {
    choice = parseChoice(event.currentTarget.value) ?? DEFAULT_CHOICE;
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
    color: var(--fgColor-muted);
  }

  select {
    min-height: var(--control-medium-size);
    padding: 0 var(--base-size-4);
    border: var(--borderWidth-thin) solid var(--control-borderColor-emphasis);
    border-radius: var(--borderRadius-medium);
    background: var(--control-bgColor-rest);
    color: var(--fgColor-default);
    font: inherit;
  }
</style>
