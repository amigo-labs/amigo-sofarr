<script lang="ts">
  import type { ConnectionAnswer } from '../api/generated/ConnectionAnswer';
  import { connectionTitle, contactText, dateTimeText } from './format';
  import StatusLabel from './StatusLabel.svelte';

  type Props = { connections: readonly ConnectionAnswer[] };
  let { connections }: Props = $props();
</script>

<section class="box" aria-labelledby="connections-title">
  <h2 id="connections-title" class="box-header">Verbindungen</h2>
  <ul class="rows">
    {#each connections as connection, i (i)}
      <li class="box-row" data-connection={connection.kind}>
        <div class="row-head">
          <h3>{connectionTitle(connection)}</h3>
          <StatusLabel status={contactText(connection.state)} />
        </div>
        {#if connection.message !== null}
          <p class:error-message={connection.state === 'error'}>{connection.message}</p>
        {/if}
        {#if connection.at !== null}
          <p class="muted">Letzter Kontakt: {dateTimeText(connection.at)}</p>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<style>
  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }

  p {
    margin: var(--base-size-4) 0 0;
  }

  .error-message {
    color: var(--fgColor-danger);
  }
</style>
