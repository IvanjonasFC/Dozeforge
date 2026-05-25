<script lang="ts">
  import { deviceStore } from '$stores/device.svelte';
  import type { Device } from '$types';

  import { api } from '$lib/tauri/api';
  import PairingModal from './PairingModal.svelte';

  let pairingAddress = $state('');
  let pairingOpen = $state(false);

  async function onSelect(e: Event) {
    const value = (e.target as HTMLSelectElement).value;
    if (!value) return;

    if (value.startsWith('pair_')) {
      pairingAddress = value.replace('pair_', '');
      pairingOpen = true;
      (e.target as HTMLSelectElement).value = deviceStore.selected?.serial ?? '';
      return;
    }
    if (value.startsWith('conn_')) {
      const address = value.replace('conn_', '');
      try {
        await api.adbConnect(address);
        await deviceStore.refresh();
      } catch (err) {
        alert('Failed to connect: ' + err);
      }
      (e.target as HTMLSelectElement).value = deviceStore.selected?.serial ?? '';
      return;
    }
    if (value.startsWith('tcpip_')) {
      const serial = value.replace('tcpip_', '');
      try {
        await api.adbTcpip(serial);
        alert('TCP/IP mode enabled on 5555. You can now disconnect the USB cable and pair via Wi-Fi.');
        await deviceStore.refresh();
      } catch (err) {
        alert('Failed to enable TCP/IP: ' + err);
      }
      (e.target as HTMLSelectElement).value = deviceStore.selected?.serial ?? '';
      return;
    }

    const found = deviceStore.devices.find((d) => d.serial === value);
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
      disabled={deviceStore.loading}
      aria-label="Select device"
    >
      {#if deviceStore.devices.length === 0 && deviceStore.mdnsServices.length === 0}
        <option value="">No devices detected</option>
      {:else}
        <option value="" disabled>Select device…</option>

        {#if deviceStore.devices.length > 0}
          <optgroup label="Connected Devices">
            {#each deviceStore.devices as device (device.serial)}
              <option value={device.serial}>{label(device)}</option>
            {/each}
          </optgroup>
          {#if deviceStore.devices.some(d => d.state === 'device' && !d.serial.includes(':'))}
            <optgroup label="Enable Wireless (USB)">
              {#each deviceStore.devices as device (device.serial)}
                {#if device.state === 'device' && !device.serial.includes(':')}
                  <option value="tcpip_{device.serial}">Enable TCP/IP on {label(device)}</option>
                {/if}
              {/each}
            </optgroup>
          {/if}
        {/if}

        {#if deviceStore.mdnsServices.filter(s => !deviceStore.devices.some(d => d.serial === s.address)).length > 0}
          <optgroup label="Discovered Network Devices">
            {#each deviceStore.mdnsServices.filter(s => !deviceStore.devices.some(d => d.serial === s.address)) as service (service.address + service.service_type)}
              {#if service.service_type.includes('pairing')}
                <option value="pair_{service.address}">Pair: {service.address}</option>
              {:else if service.service_type.includes('connect')}
                <option value="conn_{service.address}">Connect: {service.address}</option>
              {:else}
                <option value="conn_{service.address}">Connect: {service.address}</option>
              {/if}
            {/each}
          </optgroup>
        {:else}
          <optgroup label="Network Devices">
            <option disabled value="fw_warn">None found (Check Windows Firewall)</option>
          </optgroup>
        {/if}
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
<PairingModal address={pairingAddress} bind:open={pairingOpen} />

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
