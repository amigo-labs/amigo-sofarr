<script lang="ts">
  import { onMount } from 'svelte';
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import ImdbAttribution from '../../imdb/ImdbAttribution.svelte';
  import { notify } from '../../library/notices.svelte';
  import type { ShelfId } from '../../api/generated/ShelfId';
  import type { TitleCard } from '../../api/generated/TitleCard';
  import { addedText, type AddTarget } from '../add';
  import { getShelf } from '../api';
  import { DISCOVER_PATH } from '../links';
  import { createTitlePages } from '../pages.svelte';
  import { createTitleStates } from '../states.svelte';
  import { discover } from '../status.svelte';
  import AddDialog from '../components/AddDialog.svelte';
  import FreshnessNote from '../components/FreshnessNote.svelte';
  import StatusGate from '../components/StatusGate.svelte';
  import TitleList from '../components/TitleList.svelte';

  // All titles of one shelf, loaded while scrolling (spec discover AC-01, "more").
  type Props = { id: ShelfId; title: string };
  let { id, title }: Props = $props();

  const states = createTitleStates();
  const list = createTitlePages();
  let adding: AddTarget | null = $state(null);
  let sentinel: HTMLElement | undefined = $state();

  onMount(() => {
    void list.reset((n) => getShelf(id, n));
    return states.start();
  });

  $effect(() => {
    const shown = [...list.items];
    states.show(shown);
  });

  $effect(() => {
    // Svelte sets a bound element to null once it leaves the page.
    if (!(sentinel instanceof HTMLElement)) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) void list.next();
      },
      { rootMargin: '400px' },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  function onadd(card: TitleCard): void {
    adding = { kind: card.kind, tmdbId: card.tmdb_id, title: card.title, seasons: null };
  }
</script>

<svelte:head>
  <title>{title} – sofarr</title>
</svelte:head>

<StatusGate {title}>
  <p class="back"><a href={DISCOVER_PATH}>Zurück zu Entdecken</a></p>
  <h1>{title}</h1>
  <FreshnessNote freshness={list.freshness} />
  {#if list.failure !== null}
    <Flash tone="danger">
      <p>Die Titel konnten nicht geladen werden: {list.failure.message}</p>
      <button type="button" class="btn" onclick={() => void list.reset((n) => getShelf(id, n))}
        >Erneut versuchen</button
      >
    </Flash>
  {/if}
  {#if list.items.length > 0}
    <TitleList cards={list.items} stateOf={states.of} canAdd={true} {onadd} label={title} />
  {:else if !list.loading && list.freshness !== null}
    <p class="muted">Hier gibt es gerade keine Titel.</p>
  {/if}
  <div bind:this={sentinel} class="sentinel">
    {#if list.loading}
      <Loading label="Titel werden geladen …" />
    {:else if list.more && list.failure === null}
      <button type="button" class="btn" onclick={() => void list.next()}>Weitere laden</button>
    {/if}
  </div>
  <ImdbAttribution info={discover.imdb} />
</StatusGate>

<AddDialog
  target={adding}
  onclose={() => (adding = null)}
  onadded={(target, answer) => {
    states.added(target.kind, target.tmdbId, answer.id);
    notify('info', addedText(answer));
    adding = null;
  }}
/>

<style>
  .back {
    margin: 0;
  }

  .sentinel {
    display: flex;
    justify-content: center;
    min-height: 2rem;
  }
</style>
