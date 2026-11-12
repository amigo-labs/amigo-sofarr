<script lang="ts">
  import type { JobAnswer } from '../api/generated/JobAnswer';
  import { intervalText, lastRunText, nextRunText, resultText } from './format';
  import StatusLabel from './StatusLabel.svelte';

  type Props = { jobs: readonly JobAnswer[] };
  let { jobs }: Props = $props();
</script>

<section class="box" aria-labelledby="jobs-title">
  <h2 id="jobs-title" class="box-header">Geplante Aufgaben</h2>
  <ul class="rows">
    {#each jobs as job (job.name)}
      <li class="box-row" data-job={job.name}>
        <div class="row-head">
          <h3>{job.label}</h3>
          <StatusLabel status={resultText(job.result)} />
        </div>
        {#if job.result?.state === 'error'}
          <p class="error-message">{job.result.message}</p>
        {/if}
        <dl class="facts">
          <div>
            <dt>Intervall</dt>
            <dd>{intervalText(job.interval_minutes)}</dd>
          </div>
          <div>
            <dt>Letzter Lauf</dt>
            <dd>{lastRunText(job.last_run)}</dd>
          </div>
          <div>
            <dt>Nächster Lauf</dt>
            <dd>{nextRunText(job.next_run, job.interval_minutes)}</dd>
          </div>
        </dl>
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

  .error-message {
    margin: var(--base-size-4) 0 0;
    color: var(--fgColor-danger);
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
    gap: var(--base-size-4) var(--base-size-16);
    margin: var(--base-size-8) 0 0;
  }

  dt {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }

  dd {
    margin: 0;
  }
</style>
