<script lang="ts">
  import { deviceStore } from '$stores/device.svelte';
  import type { Device } from '$types';

  import { api } from '$lib/tauri/api';
  import PairingModal from './PairingModal.svelte';
  import { i18n } from '$stores/i18n.svelte';

  let pairingAddress = $state('');
  let pairingOpen = $state(false);
  let open = $state(false);
  let rootEl: HTMLDivElement | undefined = $state();

  function label(d: Device): string {
    if (d.model) {
      const maker = d.manufacturer ? `${d.manufacturer} ` : '';
      return `${maker}${d.model}`;
    }
    return d.product ?? d.serial;
  }

  async function choose(d: Device) {
    open = false;
    await deviceStore.select(d);
  }
  async function enableTcpip(serial: string) {
    open = false;
    try {
      await api.adbTcpip(serial);
      alert(i18n.t('TCP/IP mode enabled on 5555. You can now disconnect the USB cable and pair via Wi-Fi.'));
      await deviceStore.refresh();
    } catch (err) {
      alert('Failed to enable TCP/IP: ' + err);
    }
  }
  async function connectService(address: string) {
    open = false;
    try {
      await api.adbConnect(address);
      await deviceStore.refresh();
    } catch (err) {
      alert('Failed to connect: ' + err);
    }
  }
  // Disconnect a device. Wi-Fi (ip:port) endpoints get a real `adb disconnect`;
  // USB can only be unplugged physically, so we explain that instead.
  async function disconnect(d: Device, e: MouseEvent) {
    e.stopPropagation();
    if (d.serial.includes(':')) {
      try { await api.adbDisconnect(d.serial); }
      catch (err) { alert(i18n.t('Failed to disconnect:') + ' ' + err); }
    } else {
      alert(i18n.t('USB devices disconnect by unplugging the cable. Tip: enable Wi-Fi (TCP/IP) to manage this phone without a cable.'));
    }
    await deviceStore.refresh();
  }
  function pairService(address: string) {
    open = false;
    pairingAddress = address;
    pairingOpen = true;
  }

  // Close on outside click / Escape while open.
  $effect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => { if (rootEl && !rootEl.contains(e.target as Node)) open = false; };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') open = false; };
    window.addEventListener('mousedown', onDown);
    window.addEventListener('keydown', onKey);
    return () => { window.removeEventListener('mousedown', onDown); window.removeEventListener('keydown', onKey); };
  });

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

  const usbDevices = $derived(deviceStore.devices.filter(d => d.state === 'device' && !d.serial.includes(':')));

  const batteryColor = $derived.by(() => {
    const l = deviceStore.batteryLevel;
    if (l === null) return 'var(--fg-3)';
    if (l <= 15) return 'var(--bad)';
    if (l <= 40) return 'var(--warn)';
    return 'var(--good)';
  });

  const empty = $derived(deviceStore.devices.length === 0 && deviceStore.mdnsServices.length === 0);
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

  <div class="dd" bind:this={rootEl}>
    <button
      class="dd-trigger"
      class:open
      onclick={() => open = !open}
      disabled={deviceStore.loading}
      aria-haspopup="listbox"
      aria-expanded={open}
      aria-label={i18n.t('Select device…')}
    >
      {#if deviceStore.selected}
        <span class="state-dot" data-state={deviceStore.selected.state} title={deviceStore.selected.state}></span>
      {:else}
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" class="phone-icon">
          <rect x="4" y="2" width="8" height="12" rx="1.5"/>
          <circle cx="8" cy="11.5" r="0.6" fill="currentColor"/>
        </svg>
      {/if}
      <span class="dd-current" class:placeholder={!deviceStore.selected}>
        {deviceStore.selected ? label(deviceStore.selected) : i18n.t('Select device…')}
      </span>
      <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" class="chev">
        <path d="M4 6l4 4 4-4" stroke-linecap="round"/>
      </svg>
    </button>

    {#if open}
      <div class="dd-panel" role="listbox" tabindex="-1">
        {#if empty}
          <div class="dd-empty">{i18n.t('No devices detected')}</div>
        {:else}
          {#if deviceStore.devices.length > 0}
            <div class="dd-group">{i18n.t('Connected Devices')}</div>
            {#each deviceStore.devices as device (device.serial)}
              <div class="dd-row">
                <button
                  class="dd-item grow"
                  class:active={deviceStore.selected?.serial === device.serial}
                  role="option"
                  aria-selected={deviceStore.selected?.serial === device.serial}
                  onclick={() => choose(device)}
                >
                  <span class="state-dot" data-state={device.state}></span>
                  <span class="dd-item-label">{label(device)}</span>
                  {#if device.serial.includes(':')}<span class="dd-tag">Wi-Fi</span>{/if}
                </button>
                {#if device.serial.includes(':')}
                  <!-- Only Wi-Fi (TCP) devices can be disconnected by software.
                       USB devices are unplugged physically — the "Enable TCP/IP"
                       action below is the path to manage them without a cable. -->
                  <button
                    class="dd-x"
                    title={i18n.t('Disconnect (Wi-Fi)')}
                    aria-label={i18n.t('Disconnect')}
                    onclick={(e) => disconnect(device, e)}
                  >
                    <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 4l8 8M12 4l-8 8"/></svg>
                  </button>
                {:else}
                  <span class="dd-usb" title={i18n.t('USB — unplug the cable to disconnect')}>USB</span>
                {/if}
              </div>
            {/each}

            {#if usbDevices.length > 0}
              <div class="dd-group">{i18n.t('Enable Wireless (USB)')}</div>
              {#each usbDevices as device (device.serial)}
                <button class="dd-item sub" role="option" aria-selected="false" onclick={() => enableTcpip(device.serial)}>
                  <span class="dd-item-label">{i18n.t('Enable TCP/IP on {{label}}', { label: label(device) })}</span>
                </button>
              {/each}
            {/if}
          {/if}

          {#if discovered.length > 0}
            <div class="dd-group">{i18n.t('Discovered Network Devices')}</div>
            {#each discovered as service (service.address + service.service_type)}
              {#if service.service_type.includes('pairing')}
                <button class="dd-item sub" role="option" aria-selected="false" onclick={() => pairService(service.address)}>
                  <span class="dd-item-label">{i18n.t('Pair:')} {service.address}</span>
                </button>
              {:else}
                <button class="dd-item sub" role="option" aria-selected="false" onclick={() => connectService(service.address)}>
                  <span class="dd-item-label">{i18n.t('Connect:')} {service.address}</span>
                </button>
              {/if}
            {/each}
          {:else}
            <div class="dd-group">{i18n.t('Network Devices')}</div>
            <div class="dd-empty small">{i18n.t('None found — open "Pair device with pairing code" on the phone')}</div>
          {/if}
        {/if}
      </div>
    {/if}
  </div>

  <button class="ghost reload-btn" onclick={() => deviceStore.refresh()} disabled={deviceStore.loading} title={i18n.t('Reload device list')} aria-label={i18n.t('Reload')}>
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" class={deviceStore.loading ? 'spin' : ''}>
      <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2v3.5h-3.5"/>
    </svg>
  </button>
</div>
<PairingModal address={pairingAddress} bind:open={pairingOpen} />

<style>
  .picker { display: flex; align-items: center; gap: 0.4rem; }

  .dd { position: relative; }

  .dd-trigger {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    min-width: 230px;
    padding: 0.4rem 0.7rem 0.4rem 0.8rem;
    background: var(--control-bg);
    border: 1px solid var(--border);
    color: var(--fg-0);
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    border-radius: var(--radius);
    transition: background var(--t-fast), border-color var(--t-fast);
  }
  .dd-trigger:hover:not(:disabled) { background: var(--control-bg-hover); border-color: var(--border-strong); }
  .dd-trigger.open { border-color: var(--accent); }
  .dd-trigger:disabled { opacity: 0.6; cursor: default; }
  .dd-current { flex: 1; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dd-current.placeholder { color: var(--fg-3); font-weight: 500; }
  .chev { color: var(--fg-3); flex-shrink: 0; transition: transform var(--t-fast); }
  .dd-trigger.open .chev { transform: rotate(180deg); }

  .phone-icon { color: var(--fg-3); flex-shrink: 0; }
  .state-dot {
    width: 7px; height: 7px; border-radius: 50%;
    background: var(--fg-3); flex-shrink: 0;
  }
  .state-dot[data-state="device"]       { background: var(--good); box-shadow: 0 0 6px var(--good); }
  .state-dot[data-state="unauthorized"] { background: var(--warn); }
  .state-dot[data-state="offline"]      { background: var(--bad); }

  .dd-panel {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    min-width: 100%;
    width: max-content;
    max-width: 340px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 0.35rem;
    /* Solid, opaque surface so page content never bleeds through the panel. */
    background: var(--bg-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg, 12px);
    box-shadow: var(--shadow-lg);
    z-index: 1000;
    animation: dd-in 140ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  @keyframes dd-in { from { opacity: 0; transform: translateY(-6px); } to { opacity: 1; transform: translateY(0); } }

  .dd-group {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-3);
    padding: 0.55rem 0.6rem 0.3rem;
  }
  .dd-group:first-child { padding-top: 0.3rem; }

  .dd-item {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    padding: 0.5rem 0.6rem;
    background: transparent;
    border: none;
    border-radius: var(--radius);
    color: var(--fg-1);
    font-size: var(--font-size-sm);
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: background var(--t-fast), color var(--t-fast);
  }
  .dd-item:hover { background: var(--chrome-hover); color: var(--fg-0); }
  .dd-item.active { background: var(--accent-soft); color: var(--accent); }
  .dd-item.sub { color: var(--fg-2); font-weight: 400; font-size: var(--font-size-xs); }
  .dd-item.sub:hover { color: var(--fg-0); }
  .dd-item-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .dd-row { display: flex; align-items: center; gap: 2px; }
  .dd-item.grow { flex: 1; width: auto; min-width: 0; }
  .dd-tag { margin-left: 0.4rem; font-size: 9px; font-weight: 700; letter-spacing: 0.05em; color: var(--fg-3); background: var(--bg-3); padding: 1px 5px; border-radius: 5px; flex-shrink: 0; }
  .dd-x {
    flex-shrink: 0; display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; padding: 0; background: var(--bg-3); border: 1px solid var(--border-strong); border-radius: 8px;
    color: var(--fg-1); cursor: pointer; transition: background var(--t-fast), color var(--t-fast), border-color var(--t-fast);
  }
  .dd-x svg { display: block; width: 13px; height: 13px; }
  .dd-usb {
    flex-shrink: 0; font-size: 9px; font-weight: 700; letter-spacing: 0.05em;
    color: var(--fg-3); background: var(--bg-3); border: 1px solid var(--hairline);
    padding: 3px 6px; border-radius: 6px; cursor: help;
  }
  .dd-x:hover { background: var(--bad); border-color: var(--bad); color: #fff; }

  .dd-empty { padding: 0.6rem; color: var(--fg-3); font-size: var(--font-size-sm); }
  .dd-empty.small { font-size: var(--font-size-xs); }

  .battery { display: flex; align-items: center; gap: 0.3rem; }
  .battery-pct { font-size: var(--font-size-xs); font-weight: 600; font-variant-numeric: tabular-nums; color: currentColor; }
  .reload-btn { padding: 0.4rem 0.55rem; color: var(--fg-2); background: var(--bg-2); }
  .reload-btn:hover:not(:disabled) { color: var(--fg-0); background: var(--bg-3); }
  .spin { animation: spin 800ms linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
