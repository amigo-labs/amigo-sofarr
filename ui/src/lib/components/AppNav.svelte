<script lang="ts">
  import { asset } from '$app/paths';
  import { onMount } from 'svelte';
  import menuIcon from '@primer/octicons/build/svg/three-bars-16.svg?raw';
  import closeIcon from '@primer/octicons/build/svg/x-16.svg?raw';
  import signOutIcon from '@primer/octicons/build/svg/sign-out-16.svg?raw';
  import collapseIcon from '@primer/octicons/build/svg/sidebar-expand-16.svg?raw';
  import expandIcon from '@primer/octicons/build/svg/sidebar-collapse-16.svg?raw';
  import { isCurrent, navGroups } from '../nav';
  import { applyCollapsed, readCollapsed, storeCollapsed } from '../sidebar';
  import Icon from './Icon.svelte';
  import ThemeSwitch from './ThemeSwitch.svelte';

  // The navigation (specs web-ui-shell AC-08, AC-09, ui-shell-redesign AC-01 .. AC-08): a
  // sidebar on wide windows, collapsible to icons; on narrow ones a top bar whose menu button
  // opens the same navigation as a drawer (a modal dialog, so focus stays inside). Without a
  // navigation (login) only a top bar with the brand and the theme switch shows.
  type Props = {
    pathname: string;
    showNav: boolean;
    showLogout: boolean;
    logoutBusy: boolean;
    onlogout: () => void;
  };
  let { pathname, showNav, showLogout, logoutBusy, onlogout }: Props = $props();

  const WIDE = '(min-width: 1024px)';
  const groups = navGroups();
  const storage = (): Storage => window.localStorage;

  let collapsed = $state(false);
  let drawerOpen = $state(false);
  let drawer: HTMLDialogElement | null = $state(null);
  let tip: { label: string; top: number } | null = $state(null);

  onMount(() => {
    collapsed = readCollapsed(storage);
    applyCollapsed(document.documentElement, collapsed);
    const wide = window.matchMedia(WIDE);
    const resized = (): void => {
      if (wide.matches) closeDrawer();
    };
    wide.addEventListener('change', resized);
    return () => wide.removeEventListener('change', resized);
  });

  // A new page closes the drawer.
  $effect(() => {
    void pathname;
    closeDrawer();
  });

  // A click on the dimmed page lands on the dialog itself, outside its content.
  $effect(() => {
    const dialog = drawer;
    if (dialog === null) return;
    const clicked = (event: MouseEvent): void => {
      if (event.target === dialog) closeDrawer();
    };
    // Tab wraps inside the drawer: past the last control to the first and back.
    const keydown = (event: KeyboardEvent): void => {
      if (event.key !== 'Tab') return;
      const controls = [...dialog.querySelectorAll<HTMLElement>('a[href], button:not(:disabled)')];
      const first = controls[0];
      const last = controls.at(-1);
      if (first === undefined || last === undefined) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    dialog.addEventListener('click', clicked);
    dialog.addEventListener('close', drawerClosed);
    dialog.addEventListener('keydown', keydown);
    return () => {
      dialog.removeEventListener('click', clicked);
      dialog.removeEventListener('close', drawerClosed);
      dialog.removeEventListener('keydown', keydown);
    };
  });

  function openDrawer(): void {
    if (drawer === null || drawer.open) return;
    drawer.showModal();
    drawerOpen = true;
    drawer.querySelector<HTMLElement>('nav a')?.focus();
  }

  function drawerClosed(): void {
    drawerOpen = false;
  }

  function closeDrawer(): void {
    if (drawer?.open) drawer.close();
    drawerOpen = false;
  }

  function toggleCollapsed(): void {
    collapsed = !collapsed;
    tip = null;
    applyCollapsed(document.documentElement, collapsed);
    storeCollapsed(storage, collapsed);
  }

  function showTip(label: string, event: Event): void {
    if (!collapsed || !(event.currentTarget instanceof HTMLElement)) return;
    const rect = event.currentTarget.getBoundingClientRect();
    tip = { label, top: rect.top + rect.height / 2 };
  }
</script>

{#snippet brand()}
  <a class="brand" href="/">
    <!-- sofarr's monogram from the amigo brand (ui-consistency AC-13); decorative, the
         link is named by its text. -->
    <img class="monogram" src={asset('favicon.svg')} alt="" width="24" height="24" />
    <span class="brand-name">sofarr</span>
  </a>
{/snippet}

{#snippet entries(inSidebar: boolean)}
  <nav aria-label="Hauptnavigation">
    {#each groups as group (group.label)}
      <div class="group">
        <span class="group-heading" aria-hidden="true">{group.label}</span>
        <ul aria-label={group.label}>
          {#each group.pages as item (item.href)}
            <li>
              <a
                href={item.href}
                aria-current={isCurrent(item.href, pathname) ? 'page' : undefined}
                onmouseenter={inSidebar ? (e) => showTip(item.label, e) : undefined}
                onmouseleave={inSidebar ? () => (tip = null) : undefined}
                onfocus={inSidebar ? (e) => showTip(item.label, e) : undefined}
                onblur={inSidebar ? () => (tip = null) : undefined}
              >
                <Icon svg={item.icon} />
                <span class="entry-label">{item.label}</span>
              </a>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </nav>
{/snippet}

{#snippet logout()}
  {#if showLogout}
    <button
      type="button"
      class="nav-action"
      title="Abmelden"
      disabled={logoutBusy}
      onclick={onlogout}
    >
      <Icon svg={signOutIcon} />
      <span class="entry-label">Abmelden</span>
    </button>
  {/if}
{/snippet}

{#if showNav}
  <aside class="sidebar">
    <div class="sidebar-top">{@render brand()}</div>
    <div class="sidebar-entries">{@render entries(true)}</div>
    <div class="sidebar-footer">
      <ThemeSwitch compact={collapsed} />
      {@render logout()}
      <button
        type="button"
        class="nav-action"
        aria-label={collapsed ? 'Seitenleiste ausklappen' : 'Seitenleiste einklappen'}
        title={collapsed ? 'Seitenleiste ausklappen' : 'Seitenleiste einklappen'}
        onclick={toggleCollapsed}
      >
        <Icon svg={collapsed ? expandIcon : collapseIcon} />
        <span class="entry-label" aria-hidden="true">Einklappen</span>
      </button>
    </div>
  </aside>
  {#if tip !== null}
    <div class="nav-tooltip" style:top="{tip.top}px" aria-hidden="true">{tip.label}</div>
  {/if}

  <header class="topbar narrow-only">
    <button
      id="menu-button"
      type="button"
      class="icon-button"
      aria-expanded={drawerOpen}
      aria-controls="hauptnavigation"
      aria-label={drawerOpen ? 'Menü schließen' : 'Menü öffnen'}
      onclick={openDrawer}
    >
      <Icon svg={drawerOpen ? closeIcon : menuIcon} />
    </button>
    {@render brand()}
  </header>

  <dialog id="hauptnavigation" class="drawer" bind:this={drawer} aria-label="Menü">
    <div class="drawer-body">
      <div class="sidebar-top">{@render brand()}</div>
      <div class="sidebar-entries">{@render entries(false)}</div>
      <div class="sidebar-footer">
        <ThemeSwitch />
        {@render logout()}
      </div>
    </div>
  </dialog>
{:else}
  <header class="topbar">
    {@render brand()}
    <div class="topbar-tools"><ThemeSwitch /></div>
  </header>
{/if}

<style>
  /* Sidebar: wide windows only. */
  .sidebar {
    display: none;
  }

  @media (min-width: 1024px) {
    .sidebar {
      position: sticky;
      top: 0;
      display: flex;
      flex-direction: column;
      width: 240px;
      height: 100vh;
      height: 100dvh;
      border-right: var(--borderWidth-thin) solid var(--borderColor-default);
      background: var(--bgColor-surface);
    }

    .topbar.narrow-only {
      display: none;
    }
  }

  .sidebar-top {
    display: flex;
    align-items: center;
    min-height: 3.5rem;
    padding: var(--base-size-8) var(--base-size-12);
  }

  .sidebar-entries {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding: 0 var(--base-size-8);
  }

  .sidebar-footer {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    padding: var(--base-size-12) var(--base-size-8);
    border-top: var(--borderWidth-thin) solid var(--borderColor-default);
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: var(--base-size-8);
    padding: var(--base-size-4);
    border-radius: var(--borderRadius-medium);
    color: var(--fgColor-default);
    font-size: var(--text-title-size-small);
    font-weight: var(--base-text-weight-semibold);
    text-decoration: none;
  }

  .monogram {
    flex: none;
    width: 1.5rem;
    height: 1.5rem;
    image-rendering: pixelated;
  }

  .group + .group {
    margin-top: var(--base-size-12);
  }

  .group-heading {
    display: block;
    padding: var(--base-size-4) var(--base-size-8);
    color: var(--fgColor-caption);
    font-size: 11px;
    font-weight: var(--base-text-weight-semibold);
    letter-spacing: 0.06em;
    line-height: 16px;
    text-transform: uppercase;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  nav a,
  .nav-action {
    display: flex;
    align-items: center;
    gap: var(--base-size-12);
    width: 100%;
    min-height: var(--control-medium-size);
    padding: 0 var(--base-size-8);
    border: 0;
    border-radius: var(--borderRadius-medium);
    background: transparent;
    color: var(--fgColor-muted);
    font: inherit;
    font-weight: var(--base-text-weight-medium);
    text-align: left;
    text-decoration: none;
    cursor: pointer;
  }

  nav a:hover,
  .nav-action:hover:not(:disabled) {
    background: var(--button-default-bgColor-hover);
    color: var(--fgColor-default);
  }

  nav a[aria-current='page'] {
    background: var(--bgColor-accent-muted);
    color: var(--fgColor-accent);
  }

  .nav-action:disabled {
    cursor: progress;
    opacity: 0.7;
  }

  /* Collapsed: icons only; names stay for screen readers, the label shows as a tooltip. */
  :global(:root[data-sidebar='collapsed']) .sidebar {
    width: 56px;
  }

  :global(:root[data-sidebar='collapsed']) .sidebar .group-heading {
    display: none;
  }

  :global(:root[data-sidebar='collapsed']) .sidebar .group + .group {
    margin-top: var(--base-size-8);
    padding-top: var(--base-size-8);
    border-top: var(--borderWidth-thin) solid var(--borderColor-default);
  }

  :global(:root[data-sidebar='collapsed']) .sidebar .entry-label,
  :global(:root[data-sidebar='collapsed']) .sidebar .brand-name {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
  }

  :global(:root[data-sidebar='collapsed']) .sidebar :is(nav a, .nav-action) {
    justify-content: center;
    padding: 0;
  }

  :global(:root[data-sidebar='collapsed']) .sidebar .sidebar-top {
    justify-content: center;
    padding: var(--base-size-8) 0;
  }

  :global(:root[data-sidebar='collapsed']) .sidebar .sidebar-footer {
    align-items: center;
  }

  .nav-tooltip {
    position: fixed;
    left: calc(56px + var(--base-size-8));
    z-index: 20;
    padding: var(--base-size-4) var(--base-size-8);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-medium);
    background: var(--bgColor-surface);
    color: var(--fgColor-default);
    font-size: var(--text-body-size-small);
    white-space: nowrap;
    transform: translateY(-50%);
    pointer-events: none;
  }

  /* Top bar: narrow windows, and the login page at any width. */
  .topbar {
    display: flex;
    align-items: center;
    gap: var(--base-size-8);
    padding: var(--base-size-8) var(--base-size-16);
    border-bottom: var(--borderWidth-thin) solid var(--borderColor-default);
    background: var(--bgColor-surface);
  }

  .topbar-tools {
    margin-left: auto;
  }

  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-medium-size);
    height: var(--control-medium-size);
    padding: 0;
    border: var(--borderWidth-thin) solid var(--control-borderColor-emphasis);
    border-radius: var(--borderRadius-medium);
    background: transparent;
    color: var(--fgColor-default);
    cursor: pointer;
  }

  /* Drawer: the navigation over the dimmed page, from the left. */
  .drawer {
    width: min(280px, 85vw);
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    padding: 0;
    border: 0;
    border-right: var(--borderWidth-thin) solid var(--borderColor-default);
    background: var(--bgColor-surface);
    color: var(--fgColor-default);
  }

  .drawer[open] {
    animation: slide-in 160ms ease-out;
  }

  .drawer::backdrop {
    background: var(--bgColor-backdrop);
  }

  .drawer-body {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  @keyframes slide-in {
    from {
      transform: translateX(-100%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .drawer[open] {
      animation: none;
    }
  }
</style>
