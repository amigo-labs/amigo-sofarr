<script module lang="ts">
  export type Person = {
    readonly id: number;
    readonly name: string;
    readonly role: string | null;
    readonly photo: string | null;
  };
</script>

<script lang="ts">
  import { personHref } from '../links';
  import TmdbImage from './TmdbImage.svelte';

  // People with photo and role, each linking to their person page (spec discover AC-04,
  // AC-06); without TMDB no links are offered (AC-15).

  // `strip` shows them as one scrolling row of round portraits (spec ui-detail-head AC-05).
  type Props = { people: readonly Person[]; links?: boolean; label: string; strip?: boolean };
  let { people, links = true, label, strip = false }: Props = $props();
</script>

<ul class="people" class:strip aria-label={label}>
  {#each people as person, i (`${person.id}-${i}`)}
    <li class="person">
      <TmdbImage src={person.photo} name={person.name} shape="photo" />
      {#if links}
        <a class="ui-link" href={personHref(person.id)}>{person.name}</a>
      {:else}
        <span class="name">{person.name}</span>
      {/if}
      {#if person.role !== null && person.role !== ''}
        <span class="role">{person.role}</span>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .people {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(7rem, 1fr));
    gap: var(--base-size-12);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .person {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .person a,
  .name {
    font-weight: var(--base-text-weight-semibold);
  }

  .role {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }

  .strip {
    display: flex;
    gap: var(--base-size-16);
    overflow-x: auto;
    padding-bottom: var(--base-size-8);
    scroll-padding-inline: var(--base-size-8);
  }

  .strip .person {
    flex: none;
    width: 6.5rem;
    align-items: center;
    text-align: center;
  }

  .strip .person :global(.image) {
    width: 80px;
    aspect-ratio: 1;
    border-radius: var(--borderRadius-full);
  }

  @media (max-width: 543.98px) {
    .people:not(.strip) {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
</style>
