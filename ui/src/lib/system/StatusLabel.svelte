<script lang="ts">
  import type { StatusText } from './format';

  type Props = { status: StatusText };
  let { status }: Props = $props();
</script>

<span
  class="label"
  class:label-success={status.tone === 'success'}
  class:label-danger={status.tone === 'danger'}>{status.text}</span
>
