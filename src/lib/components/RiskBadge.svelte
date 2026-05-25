<script lang="ts">
  import type { RiskTier } from '$types';

  interface Props {
    tier: RiskTier;
    label?: string;
  }

  let { tier, label }: Props = $props();

  const text = $derived(label ?? tier);
</script>

<span class="badge {tier}">{text}</span>
