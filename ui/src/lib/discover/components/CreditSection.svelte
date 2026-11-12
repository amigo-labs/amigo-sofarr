<script lang="ts">
  import type { CreditCard } from '../../api/generated/CreditCard';
  import type { TitleCard } from '../../api/generated/TitleCard';
  import { arrange, CREDITS_PAGE, type CreditChoices } from '../credits';
  import type { State } from '../states.svelte';
  import { jobText } from '../texts';
  import TitleList from './TitleList.svelte';

  // One section of a person page: the cast or a crew department (spec discover AC-05),
  // sorted and filtered as chosen, long lists shown in pages.
  type Props = {
    id: string;
    title: string;
    credits: readonly CreditCard[];
    choices: CreditChoices;
    stateOf: (title: TitleCard) => State;
    canAdd: boolean;
    onadd: (card: TitleCard) => void;
    /** Shown when the person has no credits here at all. */
    empty: string;
  };
  let { id, title, credits, choices, stateOf, canAdd, onadd, empty }: Props = $props();

  let shown = $state(CREDITS_PAGE);
  const arranged = $derived(arrange(credits, choices, (c) => stateOf(c.title)));
  const notes = $derived(
    new Map(
      credits.map((c) => [
        `${c.title.kind}/${c.title.tmdb_id}`,
        c.job === null ? (c.character ?? '') : jobText(c.job),
      ]),
    ),
  );

  // A new choice starts with the first page again.
  $effect(() => {
    void choices;
    shown = CREDITS_PAGE;
  });
</script>

<section class="section" aria-labelledby={id} data-testid="credits">
  <h2 {id}>{title} <span class="count muted">({arranged.length})</span></h2>
  {#if credits.length === 0}
    <p class="muted">{empty}</p>
  {:else if arranged.length === 0}
    <p class="muted">Keine Titel passen zur Auswahl.</p>
  {:else}
    <TitleList
      cards={arranged.slice(0, shown).map((c) => c.title)}
      {stateOf}
      noteOf={(card) => notes.get(`${card.kind}/${card.tmdb_id}`) ?? null}
      {canAdd}
      {onadd}
      label={title}
    />
    {#if arranged.length > shown}
      <button type="button" class="btn more" onclick={() => (shown += CREDITS_PAGE)}>
        Weitere anzeigen ({arranged.length - shown})<span class="sr-only">{` in „${title}“`}</span>
      </button>
    {/if}
  {/if}
</section>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
  }

  h2,
  p {
    margin: 0;
  }

  .count {
    font-weight: var(--base-text-weight-normal);
  }

  .more {
    align-self: center;
  }
</style>
