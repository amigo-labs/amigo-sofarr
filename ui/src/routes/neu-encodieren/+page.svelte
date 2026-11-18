<script lang="ts">
  import PageHeader from '#lib/components/PageHeader.svelte';
  import { onMount } from 'svelte';
  import Flash from '#lib/components/Flash.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import PageError from '#lib/components/PageError.svelte';
  import type { PageCandidate } from '#lib/api/generated/PageCandidate.ts';
  import { notify } from '#lib/library/notices.svelte.ts';
  import { createResource } from '#lib/resource.svelte.ts';
  import {
    candidateHref,
    encodeBlocked,
    exclusionTexts,
    gbText,
    getReencodePage,
    jobText,
    mbText,
    outcomeText,
    runnerText,
    savedText,
    setExcluded,
    sizesText,
    startEncode,
    stateText,
  } from '#lib/reencode/reencode.ts';
  import { dateTimeText } from '#lib/system/format.ts';

  // The page "Neu encodieren" (spec ui-reencode): the job, the files that are too large
  // with their job state, encode at once, keep a file out of the job, and the history.
  // The resource reloads every 15 seconds (AC-09).
  const page = createResource(getReencodePage);
  let busy = $state(new Set<string>());
  let loadedAt = $state<string | null>(null);

  onMount(() => page.start());

  $effect(() => {
    if (page.data !== null && page.failure === null) loadedAt = new Date().toISOString();
  });

  const key = (c: PageCandidate): string => `${c.kind}-${c.movie_id ?? c.episode_ids[0]}`;

  async function act(c: PageCandidate, action: () => Promise<{ ok: boolean }>, done: string) {
    const k = key(c);
    busy = new Set([...busy, k]);
    const result = await action();
    busy = new Set([...busy].filter((b) => b !== k));
    if (result.ok) notify('info', done);
    else if ('failure' in result) {
      const failure = (result as { failure: { kind: string; message: string } }).failure;
      if (failure.kind !== 'unauthorized') notify('danger', failure.message);
    }
    await page.reload();
  }
</script>

<svelte:head>
  <title>Neu encodieren – sofarr</title>
</svelte:head>

<PageHeader title="Neu encodieren" />

{#if page.data !== null}
  {@const p = page.data}
  {@const blocked = encodeBlocked(p)}
  {#if page.failure !== null && loadedAt !== null}
    <Flash tone="danger">
      <p data-testid="reencode-stale">
        Stand von {dateTimeText(loadedAt)} – sofarr ist nicht erreichbar
      </p>
    </Flash>
  {/if}

  <section class="box" aria-labelledby="reencode-state-title">
    <h2 id="reencode-state-title" class="box-header">Stand</h2>
    <dl class="box-row facts">
      <div>
        <dt>Job</dt>
        <dd data-testid="reencode-job">{jobText(p.job)}</dd>
      </div>
      <div>
        <dt>Läuft gerade</dt>
        <dd data-testid="reencode-running">
          {#if p.running === null}
            nichts
          {:else}
            {p.running.title} – seit {dateTimeText(p.running.started_at)} ({runnerText(
              p.running.runner,
            )})
          {/if}
        </dd>
      </div>
      <div>
        <dt>Gespart</dt>
        <dd data-testid="reencode-saved">{savedText(p.saved_files, p.saved_bytes)}</dd>
      </div>
    </dl>
  </section>

  <section class="box" aria-labelledby="reencode-candidates-title" data-testid="candidates">
    <h2 id="reencode-candidates-title" class="box-header">Zu große Dateien</h2>
    {#if p.candidates.length === 0}
      <p class="box-row" data-testid="no-candidates">Keine Datei ist zu groß.</p>
    {:else}
      <p class="box-row small" data-testid="candidates-summary">
        {p.count}
        {p.count === 1 ? 'Datei' : 'Dateien'}, {gbText(p.total_size)}
      </p>
    {/if}
    {#each exclusionTexts(p.excluded) as text (text)}
      <p class="box-row small muted" data-testid="left-out">{text}</p>
    {/each}
    {#if p.candidates.length > 0}
      <ul class="list">
        {#each p.candidates as c (key(c))}
          {@const href = candidateHref(c)}
          {@const k = key(c)}
          <li class="box-row item" data-candidate={c.title}>
            <div class="what">
              {#if href === null}
                <strong>{c.title}</strong>
              {:else}
                <a class="title ui-link" {href}>{c.title}</a>
              {/if}
              <span class="small muted">
                {gbText(c.size)} · {mbText(c.mb_per_minute)} &gt; {mbText(c.threshold)} MB/min ·
                {c.resolution} · {c.video_codec}
              </span>
              <span class="small" data-testid="state">{stateText(c)}</span>
            </div>
            <div class="actions">
              <button
                type="button"
                class="btn"
                disabled={blocked !== null || busy.has(k)}
                title={blocked ?? undefined}
                onclick={() => void act(c, () => startEncode(c), `Encode gestartet: ${c.title}`)}
              >
                Jetzt encodieren<span class="sr-only"> „{c.title}“</span>
              </button>
              <button
                type="button"
                class="btn"
                disabled={p.shadow || busy.has(k)}
                title={p.shadow ? 'im Shadow-Mode' : undefined}
                onclick={() =>
                  void act(
                    c,
                    () => setExcluded(c, c.state !== 'excluded'),
                    c.state === 'excluded'
                      ? `Wieder aufgenommen: ${c.title}`
                      : `Ausgeschlossen: ${c.title}`,
                  )}
              >
                {c.state === 'excluded' ? 'Wieder aufnehmen' : 'Ausschließen'}<span class="sr-only">
                  „{c.title}“</span
                >
              </button>
              {#if blocked !== null}
                <span class="small muted" data-testid="blocked">{blocked}</span>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="box" aria-labelledby="reencode-history-title" data-testid="history">
    <h2 id="reencode-history-title" class="box-header">Verlauf</h2>
    {#if p.history.length === 0}
      <p class="box-row">Noch nichts encodiert.</p>
    {:else}
      <ul class="list">
        {#each p.history as h, i (i)}
          <li class="box-row item" data-attempt={i}>
            <div class="what">
              <strong>{h.title}</strong>
              <span class="small muted"
                >{dateTimeText(h.started_at)} · {runnerText(h.runner)} · {sizesText(h)}</span
              >
            </div>
            <div class="small">
              <strong>{outcomeText(h.outcome)}</strong>{#if h.reason !== null}: {h.reason}{/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{:else if page.failure !== null}
  <PageError
    sub
    title="Neu encodieren ist nicht verfügbar"
    message={page.failure.message}
    action="Erneut versuchen"
    onaction={() => void page.reload()}
  />
{:else}
  <Loading label="Wird geladen …" />
{/if}

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

  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .item {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--base-size-8);
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
    min-width: 0;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8);
  }
</style>
