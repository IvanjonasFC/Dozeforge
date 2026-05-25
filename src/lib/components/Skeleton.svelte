<script lang="ts">
  interface Props {
    width?: string;
    height?: string;
    lines?: number;
    variant?: 'block' | 'line' | 'circle';
  }
  let { width = '100%', height = '14px', lines = 1, variant = 'line' }: Props = $props();

  const style = $derived(
    variant === 'circle'
      ? `width: ${width}; height: ${width}; border-radius: 50%;`
      : `width: ${width}; height: ${height};`
  );
</script>

{#if lines > 1}
  <div class="skeleton-stack">
    {#each Array(lines) as _, i (i)}
      <span class="skeleton" style={style + (i === lines - 1 ? '; width: 60%;' : '')}></span>
    {/each}
  </div>
{:else}
  <span class="skeleton" {style}></span>
{/if}

<style>
  .skeleton-stack {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
