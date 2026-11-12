<script lang="ts">
  import type { SystemAnswer } from '../api/generated/SystemAnswer';
  import { supportText } from '../trailers/trailers';
  import ConfigBox from './ConfigBox.svelte';
  import ConnectionsBox from './ConnectionsBox.svelte';
  import JobsBox from './JobsBox.svelte';
  import { modeText } from './format';

  // The system page's content (spec web-ui-shell AC-12), with whether trailers are
  // downloaded (spec trailers AC-12).
  type Props = { system: SystemAnswer };
  let { system }: Props = $props();

  const mode = $derived(modeText(system.mode));
</script>

<section class="box" aria-labelledby="about-title">
  <h2 id="about-title" class="box-header">Überblick</h2>
  <dl class="box-row facts">
    <div>
      <dt>Version</dt>
      <dd data-testid="version">{system.version}</dd>
    </div>
    <div>
      <dt>Modus</dt>
      <dd data-testid="mode"><strong>{mode.name}</strong> – {mode.hint}</dd>
    </div>
    <div>
      <dt>Trailer</dt>
      <dd data-testid="trailers">{supportText(system.trailers)}</dd>
    </div>
  </dl>
</section>

<JobsBox jobs={system.jobs} />
<ConnectionsBox connections={system.connections} />
<ConfigBox config={system.config} />

<style>
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: var(--base-size-8) var(--base-size-16);
    margin: 0;
  }

  dt {
    color: var(--fgColor-muted);
    font-size: var(--text-body-size-small);
  }

  dd {
    margin: 0;
  }
</style>
