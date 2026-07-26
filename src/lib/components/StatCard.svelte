<script lang="ts">
  import { i18n } from '$stores/i18n.svelte';

  interface Props {
    label: string;
    value: string | number;
    unit?: string;
    /** Inner SVG markup (paths/shapes) for the top-right icon. */
    icon?: string;
    /** Show the green "live" badge with an up-arrow. */
    live?: boolean;
    /** Small line under the value. */
    sub?: string;
    /** Tint the value with the brand accent. */
    accent?: boolean;
  }
  let { label, value, unit = '', icon = '', live = false, sub = '', accent = false }: Props = $props();
</script>

<div class="card stat-card">
  <div class="sc-head">
    <span class="sc-label">{label}</span>
    {#if icon}
      <svg class="sc-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        {@html icon}
      </svg>
    {/if}
  </div>
  <div class="sc-value" class:accent>
    {value}{#if unit}<span class="sc-unit">{unit}</span>{/if}
    {#if live}
      <span class="sc-live">
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M7 17L17 7M17 7H8M17 7v9"/></svg>
        {i18n.t('live')}
      </span>
    {/if}
  </div>
  {#if sub}<div class="sc-sub">{sub}</div>{/if}
</div>

<style>
  .stat-card { display: flex; flex-direction: column; gap: 0.5rem; }
  .sc-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 0.5rem; }
  .sc-label {
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .sc-icon { width: 18px; height: 18px; color: var(--fg-3); flex-shrink: 0; transition: color var(--t-fast); }
  .stat-card:hover .sc-icon { color: var(--accent); }
  .sc-value {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--fg-0);
    line-height: 1.1;
  }
  .sc-value.accent {
    background: var(--accent-gradient);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  .sc-unit { font-family: var(--font-mono); font-size: 13px; font-weight: 500; color: var(--fg-3); -webkit-text-fill-color: var(--fg-3); }
  .sc-live {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--good);
    -webkit-text-fill-color: var(--good);
    text-transform: lowercase;
  }
  .sc-sub { color: var(--fg-2); font-size: 12.5px; }
</style>
