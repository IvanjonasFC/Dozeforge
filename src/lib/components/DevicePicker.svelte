<script lang="ts">
  import { deviceStore } from '$stores/device.svelte';
  import type { Device } from '$types';

  async function onSelect(e: Event) {
    const serial = (e.target as HTMLSelectElement).value;
    const found = deviceStore.devices.find((d) => d.serial === serial);
    if (found) await deviceStore.select(found);
  }

  function label(d: Device): string {
    if (d.model) {
      const maker = d.manufacturer ? `${d.manufacturer} ` : '';
      return `${maker}${d.model}`;
    }
    return d.product ?? d.serial;
  }
</script>

<div class="picker">
  <div class="select-wrapper">
    {#if deviceStore.selected}
      <span class="state-dot" data-state={deviceStore.selected.state} title={deviceStore.selected.state}></span>
    {:else}
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" class="phone-icon">
        <rect x="4" y="2" width="8" height="12" rx="1.5"/>
        <circle cx="8" cy="11.5" r="0.6" fill="currentColor"/>
      </svg>
    {/if}
    <select
      value={deviceStore.selected?.serial ?? ''}
      onchange={onSelect}
      disabled={deviceStore.loading || deviceStore.devices.length === 0}
      aria-label="Select device"
    >
      {#if deviceStore.devices.length === 0}
        <option value="">No devices detected</option>
      {:else}
        <option value="" disabled>Select device…</option>
        {#each deviceStore.devices as device (device.serial)}
          <option value={device.serial}>{label(device)}</option>
        {/each}
      {/if}
    </select>
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" class="chev">
      <path d="M4 6l4 4 4-4" stroke-linecap="round"/>
    </svg>
  </div>
  <button class="ghost reload-btn" onclick={() => deviceStore.refresh()} disabled={deviceStore.loading} title="Reload device list" aria-label="Reload">
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" class={deviceStore.loading ? 'spin' : ''}>
      <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2v3.5h-3.5"/>
    </svg>
  </button>
</div>

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .select-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }
  .phone-icon, .state-dot {
    position: absolute;
    left: 0.85rem;
    pointer-events: none;
    z-index: 1;
  }
  .phone-icon { color: var(--fg-3); }
  .state-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--fg-3);
  }
  .state-dot[data-state="device"]       { background: var(--good); box-shadow: 0 0 6px var(--good); }
  .state-dot[data-state="unauthorized"] { background: var(--warn); }
  .state-dot[data-state="offline"]      { background: var(--bad); }
  .chev {
    position: absolute;
    right: 0.65rem;
    color: var(--fg-3);
    pointer-events: none;
  }
  select {
    appearance: none;
    -webkit-appearance: none;
    padding: 0.4rem 1.85rem 0.4rem 2.1rem;
    min-width: 220px;
    background: var(--bg-2);
    color: var(--fg-0);
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    border-radius: var(--radius);
  }
  select:hover:not(:disabled) {
    background: var(--bg-3);
    border-color: var(--border-strong);
  }
  .reload-btn {
    padding: 0.4rem 0.55rem;
    color: var(--fg-2);
    background: var(--bg-2);
  }
  .reload-btn:hover:not(:disabled) { color: var(--fg-0); background: var(--bg-3); }
  .spin { animation: spin 800ms linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
