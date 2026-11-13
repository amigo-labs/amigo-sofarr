<script lang="ts">
  import type { Tone } from '../../system/format';

  type Props = { text: string; tone?: Tone };
  let { text, tone = 'neutral' }: Props = $props();
</script>

<span class="label" class:label-success={tone === 'success'} class:label-danger={tone === 'danger'}
  >{text}</span
>
