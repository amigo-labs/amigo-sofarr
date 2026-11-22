<script lang="ts">
  import { onMount } from 'svelte';
  import type { CastAnswer } from '../api/generated/CastAnswer';
  import type { Freshness } from '../api/generated/Freshness';
  import type { MediaKind } from '../api/generated/MediaKind';
  import { getCast } from './api';
  import FreshnessNote from './components/FreshnessNote.svelte';
  import PeopleList from './components/PeopleList.svelte';
  import { discover, ensureStatus } from './status.svelte';

  // The first 10 billed cast members of a library movie or series with links to their
  // person pages (spec discover AC-06). Nothing without a TMDB key (AC-15) or for a series
  // without TMDB id; a failure is a short German note, the page stays usable (AC-14).
  // The answer's freshness goes to the page, which shows its line at its end (spec
  // ui-polish AC-10); the notice that TMDB cannot be reached stays in the box.
  type Props = { kind: MediaKind; id: number; freshness?: Freshness | null };
  let { kind, id, freshness = $bindable(null) }: Props = $props();

  let answer: CastAnswer | null = $state(null);
  let failure: string | null = $state(null);

  onMount(() => {
    void (async () => {
      await ensureStatus();
      if (discover.status?.available !== true) return;
      const result = await getCast(kind, id);
      if (result.ok) {
        answer = result.data;
        if (result.data.cast.length > 0) freshness = result.data.freshness;
      } else if (result.failure.kind !== 'unauthorized') failure = result.failure.message;
    })();
  });
</script>

{#if answer !== null && answer.cast.length > 0}
  <section class="box cast" aria-labelledby="cast-title">
    <h2 id="cast-title" class="box-header">Besetzung</h2>
    <div class="body">
      <PeopleList
        strip
        label="Besetzung"
        people={answer.cast.map((c) => ({
          id: c.person_id,
          name: c.name,
          role: c.character,
          photo: c.photo,
        }))}
      />
      <FreshnessNote part="notice" freshness={answer.freshness} />
    </div>
  </section>
{:else if failure !== null}
  <p class="muted" data-testid="cast-failure">Besetzung nicht verfügbar: {failure}</p>
{/if}

<style>
  .cast h2 {
    margin: 0;
    font-size: var(--text-title-size-small, 1rem);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-12);
    padding: var(--base-size-12) var(--base-size-16);
  }

  p {
    margin: 0;
  }
</style>
