<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import type {
    DnsPreset,
    PrivacyState,
    PrivateDnsMode,
    SystemTweaks,
    PerformanceSettings
  } from '$types';

  // Seed from cache so tab revisits render instantly (stale-while-revalidate).
  const _seedSerial = deviceStore.selected?.serial ?? '';
  let privacyState: PrivacyState | null = $state(cache.peek<PrivacyState>('privacy:' + _seedSerial));
  let presets: DnsPreset[] = $state(cache.peek<DnsPreset[]>('dns-presets') ?? []);
  let tweaks: SystemTweaks | null = $state(cache.peek<SystemTweaks>('tweaks:' + _seedSerial));
  let perfSettings: PerformanceSettings | null = $state(cache.peek<PerformanceSettings>('perf:' + _seedSerial));
  
  let captiveBusy = $state(false);
  let dataSaverBusy = $state(false);
  let loading = $state(false);
  let busy = $state(false);
  let error: string | null = $state(null);
  let success: string | null = $state(null);
  let tcpAlgo = $state('cubic');
  let tcpBusy = $state(false);
  let networkMode = $state(11);
  let networkModeBusy = $state(false);

  // ---- DNS controls ----
  let dnsMode: PrivateDnsMode = $state('opportunistic');
  let dnsHostname = $state('');

  async function refresh() {
    if (!deviceStore.selected) return;
    loading = cache.peek('privacy:' + deviceStore.selected.serial) === null; // skeleton only if nothing cached
    error = null;
    try {
      const [s, ps, tw, perf] = await Promise.all([
        cache.getOrFetch('privacy:' + deviceStore.selected.serial, TTL.medium, () => api.getPrivacyState(deviceStore.selected!.serial)),
        cache.getOrFetch('dns-presets', TTL.long, () => api.listDnsPresets()),
        cache.getOrFetch('tweaks:' + deviceStore.selected.serial, TTL.medium, () => api.getSystemTweaks(deviceStore.selected!.serial)),
        cache.getOrFetch('perf:' + deviceStore.selected.serial, TTL.medium, () => api.getPerformanceSettings(deviceStore.selected!.serial))
      ]);
      privacyState = s;
      presets = ps;
      tweaks = tw;
      perfSettings = perf;
      dnsMode = s.dns.mode;
      dnsHostname = s.dns.hostname ?? '';
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') refresh();
  });

  async function applyDns() {
    if (!deviceStore.selected) return;
    busy = true; error = null; success = null;
    try {
      await api.setPrivateDns(
        deviceStore.selected.serial,
        dnsMode,
        dnsMode === 'hostname' ? dnsHostname.trim() : null
      );
      success = `Private DNS set to ${dnsMode}${dnsMode === 'hostname' ? ` → ${dnsHostname}` : ''}.`;
      cache.invalidatePrefix('privacy:');
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }

  function pickPreset(p: DnsPreset) {
    dnsMode = 'hostname';
    dnsHostname = p.hostname;
  }

  async function toggleCaptivePortal() {
    if (!deviceStore.selected || !tweaks) return;
    captiveBusy = true; error = null; success = null;
    try {
      const currentlySuppressed = tweaks.captive_portal_mode === 0;
      const nextDisabled = !currentlySuppressed;
      await api.setCaptivePortalMode(deviceStore.selected.serial, nextDisabled);
      cache.invalidatePrefix('tweaks:');
      success = nextDisabled
        ? 'Captive portal pings blocked. Wi-Fi networks now connect without phoning Google.'
        : 'Captive portal pings restored to Android default.';
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { captiveBusy = false; }
  }

  async function toggleDataSaver() {
    if (!deviceStore.selected || !perfSettings) return;
    dataSaverBusy = true; error = null; success = null;
    try {
      const target = !perfSettings.restrict_background_data;
      await api.setDataSaver(deviceStore.selected.serial, target);
      cache.invalidatePrefix('perf:');
      perfSettings.restrict_background_data = target;
      success = target ? 'Data Saver ENABLED.' : 'Data Saver DISABLED.';
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { dataSaverBusy = false; }
  }

  async function applyTcpCongestion() {
    if (!deviceStore.selected) return;
    tcpBusy = true; error = null; success = null;
    try {
      await api.setTcpCongestion(deviceStore.selected.serial, tcpAlgo);
      success = `TCP congestion algorithm set to ${tcpAlgo.toUpperCase()}.`;
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { tcpBusy = false; }
  }

  async function applyNetworkMode() {
    if (!deviceStore.selected) return;
    networkModeBusy = true; error = null; success = null;
    try {
      await api.forceNetworkMode(deviceStore.selected.serial, networkMode);
      const labels: Record<number, string> = { 1: '2G only', 2: '3G only', 9: 'LTE only', 11: 'LTE/3G/2G (default)', 20: '5G/LTE/3G/2G', 22: '5G only' };
      success = `Network mode forced to ${labels[networkMode] ?? String(networkMode)}.`;
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { networkModeBusy = false; }
  }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Network & DNS')}</h1>
    <p class="muted">
      {i18n.t('System-wide encrypted DNS, Captive Portal blocking, and global Data Saver.')}
    </p>
  </div>
  <button class="primary" onclick={refresh} disabled={loading || !deviceStore.selected}>
    {loading ? i18n.t('Refreshing...') : i18n.t('Refresh')}
  </button>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else}
  {#if error}<div class="error" style="margin-bottom: 1rem;">{error}</div>{/if}
  {#if success}<div class="success" style="margin-bottom: 1rem;">{success}</div>{/if}

  <div class="grid two-grid">
    <div class="card">
      <h3>{i18n.t('Current Private DNS')}</h3>
      {#if !privacyState}
        <Skeleton lines={3} />
      {:else}
        <div class="dns-current">
          <div>
            <span class="muted small">{i18n.t('Mode:')}</span>
            <code class="mono pill" data-mode={privacyState.dns.mode}>{privacyState.dns.mode}</code>
          </div>
          {#if privacyState.dns.hostname}
            <div>
              <span class="muted small">{i18n.t('Hostname:')}</span>
              <code class="mono">{privacyState.dns.hostname}</code>
            </div>
          {/if}
        </div>
      {/if}

      <h3 style="margin-top: 1.5rem;">{i18n.t('Set DNS')}</h3>
      <div class="form-grid">
        <label>
          {i18n.t('Mode')}
          <select bind:value={dnsMode}>
            <option value="off">{i18n.t('Off — no DoT, plain DNS')}</option>
            <option value="opportunistic">{i18n.t('Opportunistic (Android default)')}</option>
            <option value="hostname">{i18n.t('Hostname — force a specific DoT server')}</option>
          </select>
        </label>
        {#if dnsMode === 'hostname'}
          <label>
            {i18n.t('Hostname (DNS-over-TLS endpoint)')}
            <input type="text" bind:value={dnsHostname} placeholder="dns.adguard-dns.com" spellcheck="false" autocomplete="off" />
          </label>
        {/if}
      </div>
      <button class="primary" onclick={applyDns} disabled={busy} style="margin-top: 0.85rem;">
        {busy ? i18n.t('Applying...') : i18n.t('Apply DNS')}
      </button>

      {#if presets.length > 0}
        <h4 style="margin-top: 1.5rem;">{i18n.t('Presets')}</h4>
        <div class="preset-grid">
          {#each presets as p (p.hostname)}
            <button class="preset-card" onclick={() => pickPreset(p)}>
              <div class="preset-label">{p.label}</div>
              <code class="mono preset-host">{p.hostname}</code>
              <div class="preset-flags">
                {#if p.blocks_ads}<span class="badge ok">{i18n.t('ads')}</span>{/if}
                {#if p.blocks_trackers}<span class="badge ok">{i18n.t('trackers')}</span>{/if}
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="card-column">
      <div class="card">
        <h3 style="margin: 0 0 0.4rem 0;">{i18n.t('Captive portal pings')}</h3>
        <p class="muted small" style="margin: 0 0 1rem 0;">
          {i18n.t('Android pings connectivitycheck.gstatic.com on every Wi-Fi connect.')}
          {#if tweaks}
            {i18n.t('Currently:')}
            <code class="mono pill" data-state={tweaks.captive_portal_mode === 0 ? 'off' : 'on'}>
              {tweaks.captive_portal_mode === 0 ? i18n.t('BLOCKED') : i18n.t('DEFAULT')}
            </code>
          {/if}
        </p>
        <button class={tweaks?.captive_portal_mode === 0 ? 'primary' : 'danger outline'} onclick={toggleCaptivePortal} disabled={captiveBusy || !tweaks}>
          {captiveBusy ? '...' : (tweaks?.captive_portal_mode === 0 ? i18n.t('Re-enable pings') : i18n.t('Block pings'))}
        </button>
      </div>

      <div class="card" style="margin-top: 1rem;">
        <h3 style="margin: 0 0 0.4rem 0;">{i18n.t('Global Data Saver')}</h3>
        <p class="muted small" style="margin: 0 0 1rem 0;">
          {i18n.t('Restricts background data usage to save battery and traffic.')}
          {#if perfSettings}
            {i18n.t('Currently:')} 
            <code class="mono pill" data-state={perfSettings.restrict_background_data ? 'on' : 'off'}>
              {perfSettings.restrict_background_data ? i18n.t('ENABLED') : i18n.t('DISABLED')}
            </code>
          {/if}
        </p>
        {#if perfSettings}
          <button class={perfSettings.restrict_background_data ? 'outline' : 'primary'} onclick={toggleDataSaver} disabled={dataSaverBusy}>
            {dataSaverBusy ? '...' : (perfSettings.restrict_background_data ? i18n.t('Disable Data Saver') : i18n.t('Enable Data Saver'))}
          </button>
        {/if}
      </div>

      <div class="card" style="margin-top: 1rem;">
        <h3 style="margin: 0 0 0.4rem 0;">{i18n.t('Per-App Firewall')}</h3>
        <p class="muted small" style="margin: 0 0 1rem 0;">
          {i18n.t('Block background data and CPU usage for specific applications.')}
        </p>
        <button class="outline" onclick={() => goto('/apps')}>
          {i18n.t('Manage App Firewalls')}
        </button>
      </div>
    </div>
  </div>

  <div class="grid two-grid" style="margin-top: 1rem;">
    <div class="card" style="padding: 1.25rem;">
      <h3 style="margin: 0 0 0.5rem;">{i18n.t('TCP Congestion Algorithm')}</h3>
      <p class="muted small" style="margin: 0 0 0.85rem;">
        {i18n.t('Controls how the kernel manages network congestion. BBR provides better throughput on unstable connections. Requires root access.')}
      </p>
      <div style="display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 0.85rem;">
        <button class="outline" class:active={tcpAlgo === 'cubic'} onclick={() => tcpAlgo = 'cubic'}>{i18n.t('Cubic (default)')}</button>
        <button class="outline" class:active={tcpAlgo === 'bbr'} onclick={() => tcpAlgo = 'bbr'}>BBR</button>
        <button class="outline" class:active={tcpAlgo === 'reno'} onclick={() => tcpAlgo = 'reno'}>Reno</button>
      </div>
      <button class="primary" onclick={applyTcpCongestion} disabled={tcpBusy}>
        {tcpBusy ? i18n.t('Applying...') : i18n.t('Apply Algorithm')}
      </button>
    </div>
    <div class="card" style="padding: 1.25rem;">
      <h3 style="margin: 0 0 0.5rem;">{i18n.t('Force Network Mode')}</h3>
      <p class="muted small" style="margin: 0 0 0.85rem;">
        {i18n.t('Override the preferred network type. Force LTE to save battery from 5G radio drain, or lock to a specific generation for signal stability.')}
      </p>
      <div class="form-grid">
        <label>
          {i18n.t('Network Type')}
          <select bind:value={networkMode}>
            <option value={11}>{i18n.t('LTE / 3G / 2G (default)')}</option>
            <option value={9}>{i18n.t('LTE only')}</option>
            <option value={20}>{i18n.t('5G / LTE / 3G / 2G')}</option>
            <option value={22}>{i18n.t('5G only')}</option>
            <option value={2}>{i18n.t('3G only')}</option>
            <option value={1}>{i18n.t('2G only')}</option>
          </select>
        </label>
      </div>
      <button class="primary" onclick={applyNetworkMode} disabled={networkModeBusy} style="margin-top: 0.85rem;">
        {networkModeBusy ? i18n.t('Applying...') : i18n.t('Force Network Mode')}
      </button>
    </div>
  </div>
{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 540px; }
  .two-grid { display: grid; grid-template-columns: 1.5fr 1fr; gap: 1rem; }
  .card-column { display: flex; flex-direction: column; gap: 1rem; }
  .success { padding: 0.65rem 1rem; background: rgba(16, 185, 129, 0.1); border-left: 3px solid var(--good); border-radius: var(--radius); color: var(--good); }
  .dns-current { display: flex; gap: 1.5rem; flex-wrap: wrap; margin-top: 0.5rem; }
  .pill { display: inline-block; padding: 2px 8px; border-radius: 99px; background: var(--bg-3); font-size: var(--font-size-xs); }
  .pill[data-mode="off"], .pill[data-state="on"] { color: var(--bad); }
  .pill[data-mode="opportunistic"] { color: var(--fg-2); }
  .pill[data-mode="hostname"], .pill[data-state="off"] { color: var(--good); background: rgba(16, 185, 129, 0.1); }
  .form-grid { display: grid; gap: 1rem; }
  .form-grid label { display: flex; flex-direction: column; gap: 4px; font-size: var(--font-size-sm); color: var(--fg-2); }
  .preset-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.65rem; margin-top: 0.85rem; }
  .preset-card { background: var(--bg-3); border: 1px solid var(--border); border-radius: var(--radius); padding: 0.7rem; text-align: left; cursor: pointer; transition: border-color var(--t-fast); }
  .preset-card:hover { border-color: var(--accent); }
  .preset-label { font-weight: 600; color: var(--fg-0); font-size: var(--font-size-sm); }
  .preset-host { font-size: var(--font-size-xs); color: var(--fg-2); word-break: break-all; }
  .preset-flags { display: flex; gap: 4px; margin-top: 4px; }
</style>
