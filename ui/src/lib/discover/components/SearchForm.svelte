<script lang="ts">
  import searchIcon from '@primer/octicons/build/svg/search-16.svg?raw';
  import { goto } from '$app/navigation';
  import Icon from '../../components/Icon.svelte';
  import { searchHref } from '../links';

  // The search for movies, series and people (spec discover AC-03).
  type Props = { value?: string };
  let { value = '' }: Props = $props();

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
  <label class="sr-only" for="discover-search">Suchbegriff</label>
  <input
    id="discover-search"
    type="search"
    placeholder="Filme, Serien, Personen …"
    maxlength="200"
    bind:value={text}
  />
  <button type="submit" class="btn"><Icon svg={searchIcon} />Suchen</button>
</form>

<style>
  .search {
    display: flex;
    gap: var(--base-size-8);
    max-width: 36rem;
  }

  input {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 2rem;
    padding: 0 var(--base-size-8);
    border: var(--borderWidth-thin) solid var(--control-borderColor-emphasis);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-default);
    color: var(--fgColor-default);
    font: inherit;
  }
</style>
