<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import type { Snippet } from 'svelte';
  import { goto } from '$app/navigation';
  import Icon from '../../components/Icon.svelte';
  import { searchHref } from '../links';

  // The search for movies, series and people (spec discover AC-03), with the magnifier in
  // the field as the library search has it (ui-consistency AC-05). `children` (the filter
  // button of the start page) follows „Suchen“, so both share a line on narrow windows
  // (ui-discover-polish AC-05, ui-consistency AC-06).
  type Props = { value?: string; children?: Snippet };
  let { value = '', children }: Props = $props();

  let text = $state('');
  $effect.pre(() => {
    text = value;
  });

  function submit(event: SubmitEvent): void {
    event.preventDefault();
    if (text.trim() !== '') void goto(searchHref(text));
  }
</script>

<form class="search" role="search" aria-label="Filme, Serien und Personen suchen" onsubmit={submit}>
  <div class="field">
    <label class="sr-only" for="discover-search">Suchbegriff</label>
    <span class="search-icon"><Icon svg={searchIcon} /></span>
    <input
      id="discover-search"
      class="am-input"
      type="search"
      placeholder="Filme, Serien, Personen …"
      maxlength="200"
      bind:value={text}
    />
  </div>
  <button type="submit" class="btn">Suchen</button>
  {@render children?.()}
</form>

<style>
  .search {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
    max-width: 36rem;
  }

  .field {
    position: relative;
    flex: 1 1 16rem;
    min-width: 0;
  }

  input {
    width: 100%;
    padding-left: 2rem;
  }

  .search-icon {
    position: absolute;
    top: 50%;
    left: var(--base-size-8);
    display: inline-flex;
    color: var(--fgColor-caption);
    transform: translateY(-50%);
    pointer-events: none;
  }

  @media (max-width: 767.98px) {
    .search {
      max-width: none;
    }

    .field {
      flex-basis: 100%;
    }
  }
</style>
