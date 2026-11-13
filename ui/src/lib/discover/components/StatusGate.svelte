<script lang="ts">
  import type { Snippet } from 'svelte';
  import Flash from '../../components/Flash.svelte';
  import Loading from '../../components/Loading.svelte';
  import { DISCOVER_PATH } from '../links';
  import { discover, ensureStatus } from '../status.svelte';

  // Shows a discover page only while TMDB is configured; else the German explanation and
  // nothing to act on (spec discover AC-15).
  type Props = { title: string; home?: boolean; children: Snippet };
  let { title, home = true, children }: Props = $props();

  $effect(() => {
    void ensureStatus();
  });
</script>

{#if discover.status?.available === true}
  {@render children()}
{:else}
  <h1>{title}</h1>
  {#if discover.status !== null}
    <section class="box missing" data-testid="discover-unavailable">
      <p><strong>Entdecken ist noch nicht eingerichtet.</strong></p>
      <p>
        {discover.status.message ?? 'Entdecken braucht einen TMDB-Zugang ([tmdb] in sofarr.toml).'}
      </p>
      {#if home}<p><a href={DISCOVER_PATH}>Zu Entdecken</a></p>{/if}
    </section>
  {:else if discover.failure !== null}
    <Flash tone="danger">
      <p>Entdecken ist nicht verfügbar: {discover.failure.message}</p>
      <button type="button" class="btn" onclick={() => void ensureStatus()}>Erneut versuchen</button
      >
    </Flash>
  {:else}
    <Loading label="Entdecken wird geladen …" />
  {/if}
{/if}

<style>
  .missing {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    padding: var(--base-size-16);
  }

  .missing p {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
