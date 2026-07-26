<script lang="ts">
  import { deviceStore } from '$stores/device.svelte';
  import type { Device } from '$types';

  import { api } from '$lib/tauri/api';
  import PairingModal from './PairingModal.svelte';
  import { i18n } from '$stores/i18n.svelte';

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
        alert(i18n.t('TCP/IP mode enabled on 5555. You can now disconnect the USB cable and pair via Wi-Fi.'));
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

  // Deduplicated discovered services (mDNS often lists the same one twice),
  // hiding any that are already connected.
  const discovered = $derived.by(() => {
    const seen = new Set<string>();
    const out: { address: string; service_type: string }[] = [];
    for (const s of deviceStore.mdnsServices) {
      if (deviceStore.devices.some((d) => d.serial === s.address)) continue;
      const key = s.address + s.service_type;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(s);
    }
    return out;
  });

  const batteryColor = $derived.by(() => {
    const l = deviceStore.batteryLevel;
    if (l === null) return 'var(--fg-3)';
    if (l <= 15) return 'var(--bad)';
    if (l <= 40) return 'var(--warn)';
    return 'var(--good)';
  });
</script>

<div class="picker">
  {#if deviceStore.batteryLevel !== null}
    <div class="battery" title={i18n.t('Battery')} style="color: {batteryColor}">
      <svg width="24" height="13" viewBox="0 0 26 14" fill="none" aria-hidden="true">
        <rect x="0.7" y="0.7" width="21.6" height="12.6" rx="2.6" stroke="var(--fg-2)" stroke-width="1.3"/>
        <rect x="23.4" y="4.3" width="2.2" height="5.4" rx="1" fill="var(--fg-2)"/>
        <rect x="2.4" y="2.4" height="9.2" rx="1.2" width={19.2 * (deviceStore.batteryLevel ?? 0) / 100} fill="currentColor"/>
      </svg>
      <span class="battery-pct">{deviceStore.batteryLevel}%</span>
    </div>
  {/if}
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
        <option value="">{i18n.t('No devices detected')}</option>
      {:else}
        <option value="" disabled>{i18n.t('Select device…')}</option>

        {#if deviceStore.devices.length > 0}
          <optgroup label={i18n.t('Connected Devices')}>
            {#each deviceStore.devices as device (device.serial)}
              <option value={device.serial}>{label(device)}</option>
            {/each}
          </optgroup>
          {#if deviceStore.devices.some(d => d.state === 'device' && !d.serial.includes(':'))}
            <optgroup label={i18n.t('Enable Wireless (USB)')}>
              {#each deviceStore.devices as device (device.serial)}
                {#if device.state === 'device' && !device.serial.includes(':')}
                  <option value="tcpip_{device.serial}">{i18n.t('Enable TCP/IP on {{label}}', { label: label(device) })}</option>
                {/if}
              {/each}
            </optgroup>
          {/if}
        {/if}

        {#if discovered.length > 0}
          <optgroup label={i18n.t('Discovered Network Devices')}>
            {#each discovered as service (service.address + service.service_type)}
              {#if service.service_type.includes('pairing')}
                <option value="pair_{service.address}">{i18n.t('Pair:')} {service.address}</option>
              {:else}
                <option value="conn_{service.address}">{i18n.t('Connect:')} {service.address}</option>
              {/if}
            {/each}
          </optgroup>
        {:else}
          <optgroup label={i18n.t('Network Devices')}>
            <option disabled value="fw_warn">{i18n.t('None found — open "Pair device with pairing code" on the phone')}</option>
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
    background: var(--control-bg);
    border: 1px solid var(--border);
    color: var(--fg-0);
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    border-radius: var(--radius);
  }
  select:hover:not(:disabled) {
    background: var(--control-bg-hover);
    border-color: var(--border-strong);
  }
  .battery {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .battery-pct {
    font-size: var(--font-size-xs);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: currentColor;
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
