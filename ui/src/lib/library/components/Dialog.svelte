<script lang="ts">
  import type { Snippet } from 'svelte';

  // A modal dialog (native <dialog>: focus stays inside, Escape closes it).
  type Props = {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
    actions: Snippet;
  };
  let { open, title, onclose, children, actions }: Props = $props();

  let element: HTMLDialogElement | undefined = $state();
  const id = `dialog-${Math.random().toString(36).slice(2)}`;

  $effect(() => {
    if (element === undefined) return;
    if (open && !element.open) element.showModal();
    if (!open && element.open) element.close();
  });
</script>

<dialog bind:this={element} aria-labelledby={id} {onclose}>
  {#if open}
    <h2 {id}>{title}</h2>
    <div class="body">{@render children()}</div>
    <div class="actions">{@render actions()}</div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(36rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    padding: var(--base-size-16);
    border: var(--borderWidth-thin) solid var(--borderColor-default);
    border-radius: var(--borderRadius-large, 0.75rem);
    background: var(--bgColor-default);
    color: var(--fgColor-default);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.5);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--base-size-8);
    margin: var(--base-size-12) 0;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--base-size-8);
  }
</style>
