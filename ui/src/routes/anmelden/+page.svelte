<script lang="ts">
  import Flash from '#lib/components/Flash.svelte';
  import { logIn, session } from '#lib/auth/session.svelte.ts';

  // The login page (spec web-ui-shell AC-04, AC-05). After a successful login the layout
  // leads to the requested page (`?weiter=`).
  let username = $state('');
  let password = $state('');
  let busy = $state(false);
  let failure: string | null = $state(null);
  /** When the next attempt is possible after the API throttled (epoch milliseconds). */
  let waitUntil: number | null = $state(null);
  let now = $state(Date.now());

  const waiting = $derived(waitUntil !== null && now < waitUntil);
  const secondsLeft = $derived(
    waitUntil === null ? 0 : Math.max(0, Math.ceil((waitUntil - now) / 1000)),
  );
  const timeFormat = new Intl.DateTimeFormat('de-DE', { timeStyle: 'medium' });

  $effect(() => {
    if (waitUntil === null) return;
    const ticker = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(ticker);
  });

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (busy || waiting) return;
    busy = true;
    failure = null;
    const result = await logIn({ username, password });
    busy = false;
    if (result.ok) return;
    failure = result.failure.message;
    if (result.failure.kind === 'throttled') {
      now = Date.now();
      waitUntil = now + result.failure.retryAfterSeconds * 1000;
    } else {
      waitUntil = null;
    }
  }
</script>

<svelte:head>
  <title>Anmelden – sofarr</title>
</svelte:head>

<section class="login">
  <h1>Bei sofarr anmelden</h1>

  {#if session.expired}
    <Flash tone="info"><p>Die Anmeldung ist abgelaufen. Bitte erneut anmelden.</p></Flash>
  {/if}
  {#if session.interrupted !== null}
    <Flash tone="warning">
      <p>
        „{session.interrupted}“ wurde nicht ausgeführt. Nach der Anmeldung bitte erneut auslösen.
      </p>
    </Flash>
  {/if}
  {#if failure !== null}
    <Flash tone="danger">
      <p>
        {failure}
        {#if waitUntil !== null}
          {#if waiting}
            Nächster Versuch ab {timeFormat.format(waitUntil)} Uhr möglich (in {secondsLeft}
            {secondsLeft === 1 ? 'Sekunde' : 'Sekunden'}).
          {:else}
            Ein neuer Versuch ist jetzt möglich.
          {/if}
        {/if}
      </p>
    </Flash>
  {/if}

  <form class="box" onsubmit={(event) => void submit(event)}>
    <div class="field">
      <label for="username">Benutzername</label>
      <input id="username" name="username" autocomplete="username" required bind:value={username} />
    </div>
    <div class="field">
      <label for="password">Passwort</label>
      <input
        id="password"
        name="password"
        type="password"
        autocomplete="current-password"
        required
        bind:value={password}
      />
    </div>
    <button type="submit" class="btn btn-primary" disabled={busy || waiting}>
      {busy ? 'Anmeldung läuft …' : 'Anmelden'}
    </button>
  </form>
</section>

<style>
  .login {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-16);
    width: 100%;
    max-width: 22rem;
    margin: var(--base-size-24) auto 0;
  }

  h1 {
    text-align: center;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-16);
    padding: var(--base-size-16);
    background: var(--bgColor-muted);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-4);
  }

  label {
    font-weight: var(--base-text-weight-semibold);
  }

  input {
    min-height: 2rem;
    padding: 0 var(--base-size-8);
    border: var(--borderWidth-thin) solid var(--control-borderColor-emphasis);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-default);
    color: var(--fgColor-default);
    font: inherit;
  }

  button {
    justify-content: center;
  }
</style>
