<script lang="ts">
  import Icon from '../../components/Icon.svelte';

  // The sentence of an empty overview box after a decorative icon, muted
  // (ui-overview-polish AC-02, AC-06).
  type Props = { icon: string; text: string };
  let { icon, text }: Props = $props();
</script>

<p class="box-row muted empty"><Icon svg={icon} />{text}</p>

<style>
  .empty {
    display: flex;
    align-items: center;
    gap: var(--base-size-8);
    margin: 0;
  }
</style>
