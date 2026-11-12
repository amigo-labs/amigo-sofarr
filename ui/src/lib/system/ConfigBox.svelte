<script lang="ts">
  import { configGroups } from './config-view';

  type Props = { config: Readonly<Record<string, unknown>> };
  let { config }: Props = $props();

  const groups = $derived(configGroups(config));
</script>

<section class="box" aria-labelledby="config-title" data-config>
  <div class="box-header">
    <h2 id="config-title">Konfiguration</h2>
    <p class="muted">
      Die Einstellungen in Kraft, nur zur Ansicht – geändert werden sie in <code>sofarr.toml</code>.
      Geheimnisse sind als <code>***</code> verborgen.
    </p>
  </div>
  {#each groups as group (group.title)}
    <div class="box-row">
      <h3 class="mono">{group.title}</h3>
      <dl>
        {#each group.rows as row (row.key)}
          <div class="pair">
            <dt class="mono">{row.key}</dt>
            <dd class="mono">{row.value}</dd>
          </div>
        {/each}
      </dl>
    </div>
  {/each}
</section>

<style>
  .box-header p {
    margin: var(--base-size-4) 0 0;
  }

  dl {
    margin: var(--base-size-8) 0 0;
  }

  .pair {
    display: grid;
    grid-template-columns: minmax(8rem, 16rem) minmax(0, 1fr);
    gap: var(--base-size-16);
    padding: var(--base-size-4) 0;
  }

  dt {
    color: var(--fgColor-muted);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  @media (max-width: 543.98px) {
    .pair {
      grid-template-columns: minmax(0, 1fr);
      gap: 0;
    }
  }
</style>
