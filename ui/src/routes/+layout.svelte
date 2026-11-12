<script lang="ts">
  import '../app.css';
  import { onMount, type Snippet } from 'svelte';
  import { afterNavigate, goto } from '$app/navigation';
  import { page } from '$app/state';
  import AppHeader from '#lib/components/AppHeader.svelte';
  import Flash from '#lib/components/Flash.svelte';
  import Loading from '#lib/components/Loading.svelte';
  import PageError from '#lib/components/PageError.svelte';
  import { startedCleanly } from '#lib/asset-recovery.ts';
  import { LOGIN_PATH, canLogOut, loginRequired, routeFor } from '#lib/auth/redirect.ts';
  import { dismissInterrupted, loadSession, logOut, session } from '#lib/auth/session.svelte.ts';

  let { children }: { children: Snippet } = $props();

  let logoutBusy = $state(false);
  let logoutFailure: string | null = $state(null);

  const me = $derived(session.me);
  const redirect = $derived(
    me === null ? null : routeFor(me, page.url.pathname, page.url.search, page.url.hash),
  );
  const onLoginPage = $derived(page.url.pathname === LOGIN_PATH);

  onMount(() => {
    void loadSession();
  });

  afterNavigate(() => startedCleanly());

  $effect(() => {
    if (redirect !== null) void goto(redirect, { replaceState: true });
  });

  async function logout(): Promise<void> {
    logoutBusy = true;
    logoutFailure = null;
    const result = await logOut();
    logoutBusy = false;
    // A 401 is the expired session: the login page explains it (AC-07).
    if (!result.ok && result.failure.kind !== 'unauthorized') {
      logoutFailure = result.failure.message;
    }
  }
</script>

<a class="skip-link" href="#inhalt">Zum Inhalt springen</a>
<AppHeader
  pathname={page.url.pathname}
  showNav={me !== null && !loginRequired(me)}
  showLogout={me !== null && canLogOut(me)}
  {logoutBusy}
  onlogout={() => void logout()}
/>

<main id="inhalt" tabindex="-1">
  {#if logoutFailure !== null}
    <Flash tone="danger">
      <p>Abmelden fehlgeschlagen: {logoutFailure} Die Anmeldung bleibt bestehen.</p>
      <button type="button" class="btn" onclick={() => (logoutFailure = null)}>Schließen</button>
    </Flash>
  {/if}
  {#if session.interrupted !== null && !onLoginPage && me !== null && !loginRequired(me)}
    <Flash tone="warning">
      <p>
        „{session.interrupted}“ wurde nicht ausgeführt, weil die Anmeldung abgelaufen war. Bitte bei
        Bedarf erneut auslösen.
      </p>
      <button type="button" class="btn" onclick={dismissInterrupted}>Schließen</button>
    </Flash>
  {/if}

  {#if me === null}
    {#if session.failure !== null}
      <PageError
        title="sofarr ist nicht erreichbar"
        message={session.failure.message}
        action="Erneut versuchen"
        onaction={() => void loadSession()}
      />
    {:else}
      <Loading label="sofarr wird geladen …" />
    {/if}
  {:else if redirect === null}
    {@render children()}
  {/if}
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-16);
    max-width: 80rem;
    margin: 0 auto;
    padding: var(--base-size-24) var(--base-size-16);
  }

  main:focus {
    outline: none;
  }

  .skip-link {
    position: absolute;
    left: var(--base-size-8);
    top: -4rem;
    z-index: 10;
    padding: var(--base-size-8) var(--base-size-12);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-default);
    color: var(--fgColor-accent);
  }

  .skip-link:focus {
    top: var(--base-size-8);
  }
</style>
