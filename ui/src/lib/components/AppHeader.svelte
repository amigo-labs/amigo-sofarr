<script lang="ts">
  import serverIcon from '@primer/octicons/build/svg/server-24.svg?raw';
  import menuIcon from '@primer/octicons/build/svg/three-bars-16.svg?raw';
  import closeIcon from '@primer/octicons/build/svg/x-16.svg?raw';
  import signOutIcon from '@primer/octicons/build/svg/sign-out-16.svg?raw';
  import { isCurrent, navPages } from '../nav';
  import Icon from './Icon.svelte';
  import ThemeSwitch from './ThemeSwitch.svelte';

  // Header with the navigation (spec web-ui-shell AC-08, AC-09): the pages in a row on
  // wide screens, behind a menu button on narrow ones.
  type Props = {
    pathname: string;
    showNav: boolean;
    showLogout: boolean;
    logoutBusy: boolean;
    onlogout: () => void;
  };
  let { pathname, showNav, showLogout, logoutBusy, onlogout }: Props = $props();

  let menuOpen = $state(false);
  const pages = navPages();

  // A new page closes the menu.
  $effect(() => {
    void pathname;
    menuOpen = false;
  });

  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && menuOpen) {
      menuOpen = false;
      document.getElementById('menu-button')?.focus();
    }
  }
</script>

<svelte:window onkeydown={keydown} />

<header class="app-header">
  <div class="bar">
    {#if showNav}
      <button
        id="menu-button"
        type="button"
        class="icon-button menu-button"
        aria-expanded={menuOpen}
        aria-controls="hauptnavigation"
        aria-label={menuOpen ? 'Menü schließen' : 'Menü öffnen'}
        onclick={() => (menuOpen = !menuOpen)}
      >
        <Icon svg={menuOpen ? closeIcon : menuIcon} />
      </button>
    {/if}
    <a class="brand" href="/">
      <Icon svg={serverIcon} />
      <span>sofarr</span>
    </a>
    {#if showNav}
      <nav id="hauptnavigation" class="nav" class:open={menuOpen} aria-label="Hauptnavigation">
        <ul>
          {#each pages as item (item.href)}
            <li>
              <a
                href={item.href}
                aria-current={isCurrent(item.href, pathname) ? 'page' : undefined}
              >
                {item.label}
              </a>
            </li>
          {/each}
        </ul>
      </nav>
    {/if}
    <div class="tools">
      <ThemeSwitch />
      {#if showLogout}
        <button
          type="button"
          class="icon-button"
          aria-label="Abmelden"
          title="Abmelden"
          disabled={logoutBusy}
          onclick={onlogout}
        >
          <Icon svg={signOutIcon} />
        </button>
      {/if}
    </div>
  </div>
</header>

<style>
  .app-header {
    background: var(--header-bgColor);
    color: var(--header-fgColor-logo);
  }

  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--base-size-8) var(--base-size-16);
    max-width: 80rem;
    margin: 0 auto;
    padding: var(--base-size-12) var(--base-size-16);
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-8);
    color: var(--header-fgColor-logo);
    font-size: var(--text-title-size-small);
    font-weight: var(--base-text-weight-semibold);
    text-decoration: none;
  }

  .nav ul {
    display: flex;
    flex-wrap: wrap;
    gap: var(--base-size-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nav a {
    display: block;
    padding: var(--base-size-4) var(--base-size-8);
    border-radius: var(--borderRadius-medium);
    color: var(--header-fgColor-logo);
    font-weight: var(--base-text-weight-semibold);
    text-decoration: none;
  }

  .nav a:hover {
    background: color-mix(in srgb, var(--header-fgColor-logo) 15%, transparent);
  }

  .nav a[aria-current='page'] {
    box-shadow: inset 0 -2px 0 var(--underlineNav-borderColor-active);
  }

  .tools {
    display: flex;
    align-items: center;
    gap: var(--base-size-8);
    margin-left: auto;
  }

  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    padding: 0;
    border: var(--borderWidth-thin) solid var(--header-fgColor-default);
    border-radius: var(--borderRadius-medium);
    background: transparent;
    color: var(--header-fgColor-logo);
    cursor: pointer;
  }

  .icon-button:disabled {
    cursor: progress;
    opacity: 0.7;
  }

  .app-header :global(:focus-visible) {
    outline-color: var(--header-fgColor-logo);
  }

  .menu-button {
    display: none;
  }

  @media (max-width: 767.98px) {
    .menu-button {
      display: inline-flex;
    }

    .nav {
      display: none;
      order: 10;
      flex: 1 0 100%;
    }

    .nav.open {
      display: block;
    }

    .nav ul {
      flex-direction: column;
    }
  }
</style>
