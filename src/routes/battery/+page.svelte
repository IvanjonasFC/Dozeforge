<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import AppName from '$components/AppName.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import { appModalStore } from '$stores/appModal.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import BatteryHistory from '$components/BatteryHistory.svelte';
  import StatCard from '$components/StatCard.svelte';
  import OemNote from '$components/OemNote.svelte';

  const IC_CYCLE = '<path d="M3 2v6h6"/><path d="M3 8a9 9 0 1 0 2.6-5.6L3 8"/>';
  const IC_BATT = '<rect x="2" y="7" width="16" height="10" rx="2.5"/><line x1="22" y1="11" x2="22" y2="13"/>';
  const IC_ZAP = '<polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>';
  import type { BatteryHealth, BatteryDrain, AppDrainEntry, AvrcpVersion, DisplaySettings, PerformanceSettings } from '$types';

  type Tab = 'health' | 'drain' | 'display' | 'performance' | 'history';
  let tab: Tab = $state('health');

  let battery: BatteryHealth | null = $state(null);
  let loadingHealth = $state(false);
  let errorHealth: string | null = $state(null);

  let drain: BatteryDrain | null = $state(null);
  let loadingDrain = $state(false);
  let errorDrain: string | null = $state(null);
  let drainFilter = $state('');
  let drainVerdictFilter = $state<'all' | 'zombie' | 'background_hog' | 'radio_hog'>('all');

  let display: DisplaySettings | null = $state(null);
  let loadingDisplay = $state(false);
  let errorDisplay: string | null = $state(null);
  let success: string | null = $state(null);

  let minRateInput = $state(60);
  let peakRateInput = $state(120);
  let applyBusy = $state(false);
  let btAbsVolDisabled = $state(false);
  let audioBusy = $state(false);

  let perfSettings: PerformanceSettings | null = $state(null);
  let loadingPerf = $state(false);
  let errorPerf: string | null = $state(null);
  let perfBusy = $state(false);
  let bypassEnabled = $state(false);
  let bypassBusy = $state(false);

  async function fetchPerf() {
    if (!deviceStore.selected) return;
    loadingPerf = true; errorPerf = null;
    try {
      perfSettings = await cache.getOrFetch('perf:' + deviceStore.selected.serial, TTL.short, () => api.getPerformanceSettings(deviceStore.selected!.serial));
    } catch (e) { errorPerf = (e as DozeForgeError).message; }
    finally { loadingPerf = false; }
  }

  async function setAnimScale(scale: number) {
    if (!deviceStore.selected) return;
    perfBusy = true; errorPerf = null; success = null;
    try {
      await api.setAnimationScales(deviceStore.selected.serial, scale);
      cache.invalidatePrefix('perf:');
      success = i18n.t('Animation scale set to {{scale}}x.', { scale });
      await fetchPerf();
    } catch (e) { errorPerf = (e as DozeForgeError).message; }
    finally { perfBusy = false; }
  }

  async function setBgLimit(limit: number | null) {
    if (!deviceStore.selected) return;
    perfBusy = true; errorPerf = null; success = null;
    try {
      await api.setBackgroundProcessLimit(deviceStore.selected.serial, limit);
      cache.invalidatePrefix('perf:');
      success = i18n.t('Background process limit set to {{value}}.', { value: limit === null ? i18n.t('Standard') : limit });
      await fetchPerf();
    } catch (e) { errorPerf = (e as DozeForgeError).message; }
    finally { perfBusy = false; }
  }

  let appRestrictions = $state<Record<string, any>>({});

  async function toggleChargeBypass() {
    if (!deviceStore.selected) return;
    bypassBusy = true; success = null;
    try {
      const next = !bypassEnabled;
      await api.setChargeBypass(deviceStore.selected.serial, next);
      bypassEnabled = next;
      success = next ? i18n.t('Charge bypass ENABLED. Power goes directly to components, bypassing the battery. Ideal for gaming.') : i18n.t('Charge bypass DISABLED. Normal charging resumed.');
    } catch (e) { errorHealth = (e as DozeForgeError).message; }
    finally { bypassBusy = false; }
  }

  async function fetchHealth() {
    if (!deviceStore.selected) return;
    loadingHealth = true; errorHealth = null;
    try {
      battery = await api.batteryHealth(deviceStore.selected.serial, true);
    } catch (e) { errorHealth = (e as DozeForgeError).message; }
    finally { loadingHealth = false; }
  }

  let drainTimedOut = $state(false);
  async function fetchDrain() {
    if (!deviceStore.selected) return;
    loadingDrain = true; errorDrain = null; drainTimedOut = false;
    const serial = deviceStore.selected.serial;
    try {
      // dumpsys batterystats can be slow — or hang — over Wi-Fi ADB. Cap the
      // wait so the UI recovers into the empty/retry state instead of loading
      // forever.
      drain = await Promise.race([
        api.batteryPerApp(serial),
        new Promise<never>((_, reject) => setTimeout(() => reject(new Error('__timeout__')), 30000)),
      ]) as typeof drain;
      console.debug('[DozeForge] battery_per_app →', {
        entries: drain?.entries?.length ?? 0,
        computed_drain_mah: drain?.computed_drain_mah,
        capacity_mah: drain?.capacity_mah,
      });
      if (drain?.entries) {
        // Only real, installed package names can be queried for app-ops.
        // System/shared UIDs surface as synthetic labels ("system:uid=1051",
        // "uid=1051") which fail backend package validation — skip them.
        const isRealPkg = (p: string) => p.includes('.') && !p.includes('=') && !p.includes(':');
        const pkgs = drain.entries.map(e => e.package).filter(isRealPkg).slice(0, 30);
        if (pkgs.length) {
          api.getAppRestrictionsBatch(serial, pkgs)
             .then(res => { appRestrictions = res; })
             .catch(e => console.warn(e));
        }
      }
    } catch (e) {
      if ((e as Error).message === '__timeout__') { drain = null; drainTimedOut = true; }
      else { errorDrain = (e as DozeForgeError).message; }
    } finally { loadingDrain = false; }
  }

  // In-app diagnostic for the per-app drain: runs the exact command on the
  // device and shows the "Estimated power use" section so we can see what the
  // ROM actually returns — no terminal or DevTools needed.
  let drainDiag = $state('');
  let drainDiagBusy = $state(false);
  async function diagnoseDrain() {
    if (!deviceStore.selected) return;
    drainDiagBusy = true; drainDiag = '';
    try {
      const raw = await api.runShell(deviceStore.selected.serial, 'dumpsys batterystats --charged');
      const idx = raw.indexOf('Estimated power use');
      const section = idx >= 0 ? raw.slice(idx) : raw;
      // Show the per-UID lines the parser needs (old format: "Uid u0a123: 23.4").
      const uidLines = (section.match(/^\s*Uid[^\n]*/gm) ?? []).slice(0, 20);
      drainDiag =
        `has_section: ${idx >= 0}\n` +
        `"Uid " lines found: ${uidLines.length}\n\n` +
        `--- First "Uid" lines ---\n${uidLines.join('\n') || '(NONE — format differs, need to adapt parser)'}\n\n` +
        `--- Section head (first 1800 chars) ---\n${section.slice(0, 1800)}`;
    } catch (e) {
      drainDiag = 'Error: ' + (e as DozeForgeError).message;
    } finally {
      drainDiagBusy = false;
    }
  }

  async function fetchDisplay() {
    if (!deviceStore.selected) return;
    loadingDisplay = true; errorDisplay = null;
    try {
      display = await cache.getOrFetch('display:' + deviceStore.selected.serial, TTL.medium, () => api.getDisplaySettings(deviceStore.selected!.serial));
      if (display.min_refresh_rate !== null) minRateInput = display.min_refresh_rate;
      if (display.peak_refresh_rate !== null) peakRateInput = display.peak_refresh_rate;
      btAbsVolDisabled = display.bt_absolute_volume_disabled;
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { loadingDisplay = false; }
  }

  // Load (and reload) whenever the connected device changes — covers the initial
  // mount, first connection, and reconnecting over wireless debugging.
  let lastLoadedSerial: string | null = null;
  $effect(() => {
    const dev = deviceStore.selected;
    if (dev && dev.state === 'device' && dev.serial !== lastLoadedSerial) {
      lastLoadedSerial = dev.serial;
      fetchHealth(); fetchDisplay(); fetchDrain();
    }
  });

  $effect(() => {
    if (tab === 'performance' && !perfSettings && !loadingPerf && deviceStore.selected) {
      fetchPerf();
    }
  });

  // ---- Per-app drain derivations ----
  const drainEntries = $derived.by<AppDrainEntry[]>(() => {
    const all = drain?.entries ?? [];
    const f = drainFilter.trim().toLowerCase();
    return all.filter((e) => {
      if (f) {
        const label = labelStore.labelFor(deviceStore.selected?.serial ?? null, e.package).toLowerCase();
        if (!e.package.toLowerCase().includes(f) && !label.includes(f)) return false;
      }
      if (drainVerdictFilter !== 'all' && e.verdict !== drainVerdictFilter) return false;
      return true;
    });
  });

  const drainTotals = $derived.by(() => {
    const all = drain?.entries ?? [];
    return {
      zombies: all.filter((e) => e.verdict === 'zombie').length,
      hogs: all.filter((e) => e.verdict === 'background_hog').length,
      radio: all.filter((e) => e.verdict === 'radio_hog').length,
      legitimate: all.filter(
        (e) => e.verdict === 'legitimate_foreground' || e.verdict === 'legitimate_media'
      ).length
    };
  });

  function verdictBadge(v: AppDrainEntry['verdict']): { cls: string; label: string } {
    switch (v) {
      case 'zombie':                return { cls: 'bad',  label: i18n.t('Zombie') };
      case 'background_hog':        return { cls: 'bad',  label: i18n.t('Background hog') };
      case 'radio_hog':             return { cls: 'warn', label: i18n.t('Radio hog') };
      case 'legitimate_media':      return { cls: 'ok',   label: i18n.t('Media playback') };
      case 'legitimate_foreground': return { cls: 'ok',   label: i18n.t('Foreground use') };
      default:                      return { cls: 'ok',   label: i18n.t('Negligible') };
    }
  }

  function fmtMah(v: number): string {
    if (v < 0.1) return '< 0.1';
    if (v < 10) return v.toFixed(2);
    return v.toFixed(1);
  }

  function topBreakdown(b: Record<string, number>, max = 3): string {
    const entries = Object.entries(b).sort((a, b) => b[1] - a[1]).slice(0, max);
    return entries.map(([k, v]) => `${k}=${v.toFixed(1)}`).join(', ');
  }

  let customDensity = $state('');
  let customResolution = $state('');
  let visualLoadBusy = $state(false);
  let fixedPerfBusy = $state(false);

  async function applyDensity() {
    if (!deviceStore.selected || !customDensity) return;
    applyBusy = true; errorDisplay = null; success = null;
    try {
      await api.setDisplayDensity(deviceStore.selected.serial, customDensity);
      success = i18n.t('Density set to {{value}}', { value: customDensity });
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { applyBusy = false; }
  }

  async function applyResolution() {
    if (!deviceStore.selected || !customResolution) return;
    applyBusy = true; errorDisplay = null; success = null;
    try {
      await api.setDisplaySize(deviceStore.selected.serial, customResolution);
      success = i18n.t('Resolution set to {{value}}', { value: customResolution });
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { applyBusy = false; }
  }

  async function resetDisplay() {
    if (!deviceStore.selected) return;
    applyBusy = true; errorDisplay = null; success = null;
    try {
      await api.resetDisplay(deviceStore.selected.serial);
      success = i18n.t('Display metrics reset to default');
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { applyBusy = false; }
  }

  async function toggleWindowBlurs(disable: boolean) {
    if (!deviceStore.selected) return;
    visualLoadBusy = true; errorDisplay = null; success = null;
    try {
      await api.setWindowBlurs(deviceStore.selected.serial, disable);
      success = disable ? i18n.t('Window blurs disabled.') : i18n.t('Window blurs enabled.');
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { visualLoadBusy = false; }
  }

  async function toggleReduceTransparency(enable: boolean) {
    if (!deviceStore.selected) return;
    visualLoadBusy = true; errorDisplay = null; success = null;
    try {
      await api.setReduceTransparency(deviceStore.selected.serial, enable);
      success = enable ? i18n.t('Transparency reduced.') : i18n.t('Transparency restored.');
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { visualLoadBusy = false; }
  }

  async function toggleFixedPerformanceMode(enable: boolean) {
    if (!deviceStore.selected) return;
    fixedPerfBusy = true; errorPerf = null; success = null;
    try {
      await api.setFixedPerformanceMode(deviceStore.selected.serial, enable);
      success = enable ? i18n.t('Fixed performance mode ENABLED.') : i18n.t('Fixed performance mode DISABLED.');
    } catch (e) { errorPerf = (e as DozeForgeError).message; }
    finally { fixedPerfBusy = false; }
  }

  async function applyRefreshRate() {
    if (!deviceStore.selected) return;
    applyBusy = true; errorDisplay = null; success = null;
    try {
      await api.applyRefreshRate(deviceStore.selected.serial, minRateInput, peakRateInput);
      cache.invalidatePrefix('display:');
      success = i18n.t('Refresh rate set: {{min}} – {{max}} Hz.', { min: minRateInput, max: peakRateInput });
      await fetchDisplay();
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { applyBusy = false; }
  }

  async function toggleBtAbsVol() {
    if (!deviceStore.selected) return;
    audioBusy = true; errorDisplay = null; success = null;
    try {
      const next = !btAbsVolDisabled;
      await api.setBluetoothAbsoluteVolume(deviceStore.selected.serial, next);
      cache.invalidatePrefix('display:');
      btAbsVolDisabled = next;
      success = i18n.t('Bluetooth Absolute Volume {{state}}. Re-pair the device for it to take effect.', { state: next ? i18n.t('disabled') : i18n.t('enabled') });
      await fetchDisplay();
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { audioBusy = false; }
  }

  async function toggleMasterMono() {
    if (!deviceStore.selected) return;
    audioBusy = true; errorDisplay = null; success = null;
    try {
      await api.setMasterMono(deviceStore.selected.serial, !display!.master_mono);
      cache.invalidatePrefix('display:');
      success = i18n.t('Master Mono {{state}}.', { state: !display!.master_mono ? i18n.t('enabled') : i18n.t('disabled') });
      await fetchDisplay();
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { audioBusy = false; }
  }

  async function toggleSpatialAudio() {
    if (!deviceStore.selected || display?.spatial_audio_enabled === null) return;
    audioBusy = true; errorDisplay = null; success = null;
    try {
      await api.setSpatialAudio(deviceStore.selected.serial, !display!.spatial_audio_enabled);
      cache.invalidatePrefix('display:');
      success = i18n.t('Spatial Audio {{state}}.', { state: !display!.spatial_audio_enabled ? i18n.t('enabled') : i18n.t('disabled') });
      await fetchDisplay();
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { audioBusy = false; }
  }

  async function setAvrcp(version: AvrcpVersion) {
    if (!deviceStore.selected) return;
    audioBusy = true; errorDisplay = null; success = null;
    try {
      await api.setAvrcpVersion(deviceStore.selected.serial, version);
      cache.invalidatePrefix('display:');
      success = i18n.t('AVRCP version set to {{v}}. Re-pair your BT device.', { v: version.replace('avrcp', '') });
      await fetchDisplay();
    } catch (e) { errorDisplay = (e as DozeForgeError).message; }
    finally { audioBusy = false; }
  }

  function setRatePreset(min: number, peak: number) { minRateInput = min; peakRateInput = peak; }
  function tempColor(c: number | null): string {
    if (c === null) return 'var(--fg-3)';
    if (c < 25) return 'var(--accent)';
    if (c < 35) return 'var(--good)';
    if (c < 42) return 'var(--warn)';
    return 'var(--bad)';
  }
  function tempPosition(c: number | null): number {
    if (c === null) return 0;
    return Math.min(100, Math.max(0, ((c + 10) / 70) * 100));
  }
  function healthColor(pct: number | null): string {
    if (pct === null) return 'var(--good)';
    if (pct < 60) return 'var(--bad)';
    if (pct < 80) return 'var(--warn)';
    return 'var(--good)';
  }
  function levelColor(pct: number | null): string {
    if (pct === null) return 'var(--accent)';
    if (pct < 15) return 'var(--bad)';
    if (pct < 30) return 'var(--warn)';
    return 'var(--accent)';
  }
  const healthOffset = $derived.by(() => {
    const pct = battery?.health_percent ?? null;
    if (pct === null) return 502;
    return 502 - (Math.min(100, Math.max(0, pct)) / 100) * 502;
  });

</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Battery')}</h1>
    <p class="muted">{i18n.t('Battery health, per-app drain, display refresh rate, and Bluetooth audio tuning.')}</p>
  </div>
</header>

<OemNote />

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else}
  <div class="seg" role="tablist">
    <button class:active={tab === 'health'}  onclick={() => tab = 'health'}  role="tab">{i18n.t('Health')}</button>
    <button class:active={tab === 'display'} onclick={() => tab = 'display'} role="tab">{i18n.t('Display')}</button>
    <button class:active={tab === 'performance'} onclick={() => tab = 'performance'} role="tab">{i18n.t('Performance')}</button>
    <button class:active={tab === 'history'} onclick={() => tab = 'history'} role="tab">{i18n.t('History')}</button>
  </div>

  {#if success}<div class="success">{success}</div>{/if}
  {#if errorDisplay}<div class="error">{errorDisplay}</div>{/if}

  {#if tab === 'health'}
    <div class="row-actions">
      <button class="primary" onclick={fetchHealth} disabled={loadingHealth}>
        {loadingHealth ? i18n.t('Loading...') : i18n.t('Refresh')}
      </button>
    </div>
    {#if errorHealth}<div class="error">{errorHealth}</div>{/if}
    {#if !battery}
      {#if loadingHealth}
        <div class="card"><Skeleton lines={6} /></div>
      {:else}
        <div class="card"><p class="muted">{errorHealth ?? i18n.t('No battery data yet. Press Refresh — over Wi-Fi it can take a few seconds.')}</p></div>
      {/if}
    {:else}
      <div class="grid hero-grid">
        <div class="card ring-card">
          <div class="rings-stack">
            <svg viewBox="0 0 200 200" class="big-ring">
              <circle cx="100" cy="100" r="80" stroke="var(--bg-4)" stroke-width="10" fill="none"/>
              <circle cx="100" cy="100" r="80" stroke={healthColor(battery.health_percent)} stroke-width="10" stroke-linecap="round"
                      fill="none" transform="rotate(-90 100 100)" stroke-dasharray="502"
                      stroke-dashoffset={healthOffset} class="arc"/>
              <circle cx="100" cy="100" r="60" stroke="var(--bg-4)" stroke-width="6" fill="none"/>
              <circle cx="100" cy="100" r="60" stroke={levelColor(battery.level_percent)} stroke-width="6" stroke-linecap="round"
                      fill="none" transform="rotate(-90 100 100)" stroke-dasharray="377"
                      stroke-dashoffset={377 - (Math.min(100, Math.max(0, battery.level_percent ?? 0)) / 100) * 377}
                      class="arc"/>
              {#if battery.health_percent !== null}
                <text x="100" y="92" text-anchor="middle" class="big-pct">{Math.round(battery.health_percent)}%</text>
                <text x="100" y="116" text-anchor="middle" class="big-sub">{i18n.t('capacity')}</text>
              {:else if battery.level_percent !== null}
                <!-- Health % needs charge_full_design from sysfs, which many ROMs
                     (e.g. Nothing) lock behind root. Fall back to live level. -->
                <text x="100" y="92" text-anchor="middle" class="big-pct">{battery.level_percent}%</text>
                <text x="100" y="112" text-anchor="middle" class="big-sub">{i18n.t('current level')}</text>
                <text x="100" y="128" text-anchor="middle" class="big-note">{i18n.t('health needs root')}</text>
              {:else}
                <text x="100" y="104" text-anchor="middle" class="big-pct big-muted">?</text>
              {/if}
            </svg>
            <div class="legend">
              <span><span class="dot" style="background: {healthColor(battery.health_percent)}"></span> {i18n.t('Capacity vs design')}</span>
              <span><span class="dot" style="background: {levelColor(battery.level_percent)}"></span> {i18n.t('Current level')}</span>
            </div>
          </div>
        </div>

        <div class="stats-col">
          <StatCard
            label={i18n.t('Cycle count')}
            value={battery.cycle_count !== null ? battery.cycle_count.toLocaleString() : '—'}
            sub={battery.cycle_count === null ? i18n.t('Not exposed by this ROM.') : ''}
            icon={IC_CYCLE}
          />
          <StatCard
            label={i18n.t('Capacity')}
            value={battery.charge_full_uah ? Math.round(battery.charge_full_uah / 1000).toLocaleString() : '—'}
            unit={battery.charge_full_uah && battery.charge_full_design_uah ? `/ ${Math.round(battery.charge_full_design_uah / 1000).toLocaleString()} mAh` : ''}
            icon={IC_BATT}
          />
          <StatCard
            label={i18n.t('Voltage')}
            value={battery.voltage_v !== null ? battery.voltage_v.toFixed(3) : '—'}
            unit={battery.voltage_v !== null ? 'V' : ''}
            icon={IC_ZAP}
          />
          <div class="card stat-tile">
            <div class="stat-label">{i18n.t('Status')}</div>
            <div class="status-row">
              <span class="badge ok">{battery.status ?? 'unknown'}</span>
              {#if battery.health_status}<span class="badge moderate">{battery.health_status}</span>{/if}
            </div>
          </div>
        </div>
      </div>

      <div class="card thermo-card">
        <div class="thermo-header">
          <h3>{i18n.t('Temperature')}</h3>
          {#if battery.temperature_c !== null}
            <div class="thermo-value mono" style="color: {tempColor(battery.temperature_c)}">
              {battery.temperature_c.toFixed(1)}°C
            </div>
          {/if}
        </div>
        {#if battery.temperature_c !== null}
          <div class="thermo-bar">
            <div class="thermo-track">
              <div class="thermo-gradient"></div>
              <div class="thermo-marker" style="left: {tempPosition(battery.temperature_c)}%"></div>
            </div>
            <div class="thermo-scale">
              <span>-10</span><span>15</span><span>40</span><span>60°C</span>
            </div>
          </div>
        {:else}
          <p class="muted">{i18n.t('Not exposed.')}</p>
        {/if}
      </div>

      <p class="muted source-note">{i18n.t('Source:')} <code class="mono">{battery.source ?? 'unavailable'}</code></p>
    {/if}

    <h2 style="margin-top: 2.5rem; margin-bottom: 0.75rem;">{i18n.t('Per-app drain')}</h2>
    <div class="card flat banner">
      <p>
        {i18n.t('Per-app battery drain since the last full charge. Each app gets a verdict so you can act without reading the breakdown.')} <strong>{i18n.t('Zombie')}</strong> {i18n.t('and')}
        <strong>{i18n.t('Background hog')}</strong> {i18n.t('rows are the highest-impact targets;')}
        <strong>{i18n.t('Media playback')}</strong> {i18n.t('and')} <strong>{i18n.t('Foreground use')}</strong> {i18n.t('are legitimate drain you should not restrict.')}
      </p>
    </div>

    <div class="row-actions">
      <button class="primary" onclick={fetchDrain} disabled={loadingDrain}>
        {loadingDrain ? i18n.t('Reading…') : i18n.t('Refresh')}
      </button>
    </div>

    {#if errorDrain}<div class="error">{errorDrain}</div>{/if}

    {#if loadingDrain && !drain}
      <div class="card"><Skeleton lines={8} /></div>
    {:else if !drain || drain.entries.length === 0}
      <div class="card" style="margin-bottom: 1rem; border-color: var(--warn); background: rgba(245, 158, 11, 0.05);">
        <p class="muted" style="color: var(--warn);">
          <strong style="color: var(--warn);">{i18n.t('No per-app drain data available.')}</strong><br/>
          {#if drainTimedOut}
            {i18n.t('Reading the battery stats took too long — it can be slow over Wi-Fi. Press Refresh to retry.')}
          {:else if battery?.status && battery.status.toLowerCase().includes('charg')}
            {i18n.t('Your phone is charging. Android only records per-app battery use while on battery — unplug the cable and refresh.')}
          {:else}
            {i18n.t('No battery usage was recorded since the last full charge, or this OEM strips these stats. Let the phone run on battery a while, then refresh.')}
          {/if}
          <br/><br/>
          <strong style="color: var(--warn);">{i18n.t('Tip:')}</strong>
          {i18n.t('Battery stats only exist while the phone is on battery. Connect over wireless debugging (unplugged) instead of USB — then let it discharge a while and refresh.')}
        </p>
        <div style="margin-top: 0.5rem;">
          <button class="btn outline small" onclick={diagnoseDrain} disabled={drainDiagBusy}>
            {drainDiagBusy ? i18n.t('Running…') : i18n.t('Run diagnostic')}
          </button>
        </div>
        {#if drainDiag}
          <pre style="margin-top: 0.75rem; max-height: 320px; overflow: auto; background: var(--bg-0); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 0.6rem; font-family: var(--font-mono); font-size: 11px; white-space: pre-wrap; color: var(--fg-2);">{drainDiag}</pre>
        {/if}
      </div>
    {:else}
      {#if drain.computed_drain_mah <= 0.5}
        <div class="charge-note">{i18n.t('Computed drain is ~0 because the phone is charging or was just fully charged. The per-app figures are cumulative estimates and the bars show relative weight — unplug and use the phone on battery for real drain numbers.')}</div>
      {/if}
      <!-- KPI strip -->
      <div class="grid drain-kpi">
        <div class="card stat-tile">
          <div class="stat-label">{i18n.t('Computed drain')}</div>
          <div class="big-num mono">{drain.computed_drain_mah.toFixed(1)} <span class="muted unit">mAh</span></div>
          {#if drain.actual_drain_min_mah !== null && drain.actual_drain_max_mah !== null}
            <p class="muted small">
              {i18n.t('Actual range: {{min}}–{{max}} mAh', { min: drain.actual_drain_min_mah.toFixed(0), max: drain.actual_drain_max_mah.toFixed(0) })}
            </p>
          {/if}
        </div>
        <div class="card stat-tile">
          <div class="stat-label">{i18n.t('Zombies')}</div>
          <div class="big-num mono" class:bad-num={drainTotals.zombies > 0}>{drainTotals.zombies}</div>
          <p class="muted small">{i18n.t('Live wakelock + no foreground use.')}</p>
        </div>
        <div class="card stat-tile">
          <div class="stat-label">{i18n.t('Background hogs')}</div>
          <div class="big-num mono" class:bad-num={drainTotals.hogs > 0}>{drainTotals.hogs}</div>
          <p class="muted small">{i18n.t('CPU-heavy in background.')}</p>
        </div>
        <div class="card stat-tile">
          <div class="stat-label">{i18n.t('Radio hogs')}</div>
          <div class="big-num mono" class:warn-num={drainTotals.radio > 0}>{drainTotals.radio}</div>
          <p class="muted small">{i18n.t('Sensor / GPS / Wi-Fi dominated.')}</p>
        </div>
      </div>

      <!-- Filter bar + table -->
      <div class="card" style="margin-top: 1rem;">
        <div class="row" style="justify-content: space-between; gap: 0.75rem; margin-bottom: 0.85rem; flex-wrap: wrap;">
          <div class="seg seg-small" role="tablist">
            <button class:active={drainVerdictFilter === 'all'}            onclick={() => drainVerdictFilter = 'all'}>{i18n.t('All')} ({drain.entries.length})</button>
            <button class:active={drainVerdictFilter === 'zombie'}         onclick={() => drainVerdictFilter = 'zombie'}>{i18n.t('Zombies')} ({drainTotals.zombies})</button>
            <button class:active={drainVerdictFilter === 'background_hog'} onclick={() => drainVerdictFilter = 'background_hog'}>{i18n.t('Hogs')} ({drainTotals.hogs})</button>
            <button class:active={drainVerdictFilter === 'radio_hog'}      onclick={() => drainVerdictFilter = 'radio_hog'}>{i18n.t('Radio')} ({drainTotals.radio})</button>
          </div>
          <input
            type="text"
            placeholder={i18n.t('Filter by package…')}
            bind:value={drainFilter}
            class="filter-input"
          />
        </div>

        {#if drainEntries.length === 0}
          <p class="muted">{i18n.t('No apps match the current filter.')}</p>
        {:else}
          <div class="scroll-y" style="max-height: 540px;">
            <table>
              <thead>
                <tr>
                  <th title={i18n.t('Application package name.')}>{i18n.t('Package')}</th>
                  <th title={i18n.t('Estimated milliamp-hours consumed since last full charge.')}>{i18n.t('Drain')}</th>
                  <th title={i18n.t('Share of total computed drain.')}>{i18n.t('Share')}</th>
                  <th title={i18n.t('Top three sub-components: cpu, wake (wakelocks), wifi, cell, sensor, gps, audio, video.')}>{i18n.t('Breakdown')}</th>
                  <th title={i18n.t('Currently holding a wakelock at the time of analysis.')}>{i18n.t('Wakelock')}</th>
                  <th title={i18n.t('A process for this app was in Z (zombie) state during the last sample.')}>{i18n.t('State')}</th>
                  <th title={i18n.t('Composite verdict of the battery drain behaviour.')}>{i18n.t('Verdict')}</th>
                </tr>
              </thead>
              <tbody>
                {#each drainEntries as e (e.uid)}
                  {@const b = verdictBadge(e.verdict)}
                  <tr class:row-bad={e.verdict === 'zombie' || e.verdict === 'background_hog'} class="app-row" onclick={() => appModalStore.open(e.package, 'battery')} style="cursor: pointer;" title={i18n.t('Click to optimize this app')}>
                    <td>
                      <AppName package={e.package} size="sm" hidePackage inline />
                      {#if appRestrictions[e.package]}
                        <div style="display: flex; gap: 4px; margin-top: 4px;">
                          {#if appRestrictions[e.package].wake_lock_ignored}
                            <span class="badge danger" style="font-size: 9px; padding: 2px 4px;">{i18n.t('Wake Blocked')}</span>
                          {/if}
                          {#if appRestrictions[e.package].run_in_background_ignored}
                            <span class="badge danger" style="font-size: 9px; padding: 2px 4px;">{i18n.t('Bg Blocked')}</span>
                          {/if}
                          {#if appRestrictions[e.package].standby_bucket === 'restricted'}
                            <span class="badge ok" style="font-size: 9px; padding: 2px 4px;">{i18n.t('Restricted')}</span>
                          {/if}
                        </div>
                      {/if}
                    </td>
                    <td class="mono"><strong>{fmtMah(e.drain_mah)}</strong> <span class="muted small">mAh</span></td>
                    <td class="mono">
                      <div class="share-bar">
                        <div class="share-fill" style="width: {Math.min(100, e.drain_share * 100).toFixed(1)}%"></div>
                        <span class="share-text">{(e.drain_share * 100).toFixed(1)}%</span>
                      </div>
                    </td>
                    <td class="small mono">{topBreakdown(e.breakdown) || '—'}</td>
                    <td>
                      {#if e.has_live_wakelock}
                        <span class="badge bad live-dot" title={i18n.t('Holding a partial wakelock right now.')}>{i18n.t('active')}</span>
                      {:else}
                        <span class="muted">—</span>
                      {/if}
                    </td>
                    <td>
                      {#if e.is_zombie}
                        <span class="badge bad live-dot" title={i18n.t('Zombie process detected.')}>{i18n.t('zombie')}</span>
                      {:else}
                        <span class="muted">—</span>
                      {/if}
                    </td>
                    <td><span class="badge {b.cls}">{b.label}</span></td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    {/if}
  {:else if tab === 'display'}
    <div class="card flat banner">
      <p>
        {i18n.t('Sets min_refresh_rate and peak_refresh_rate system-wide. Anchoring min = peak forces a fixed rate; setting min < peak lets LTPO adapt. Pixel 8 Pro supports 1–120 Hz.')}
      </p>
    </div>

    {#if !display}
      <div class="card"><Skeleton lines={4} /></div>
    {:else}
      <div class="card">
        <h3>{i18n.t("Current values")}</h3>
        <div class="dns-current">
          <div>
            <span class="muted small">{i18n.t("min:")}</span>
            <code class="mono pill">{display.min_refresh_rate ?? '—'} Hz</code>
          </div>
          <div>
            <span class="muted small">{i18n.t("peak:")}</span>
            <code class="mono pill">{display.peak_refresh_rate ?? '—'} Hz</code>
          </div>
          {#if display.max_frame_buffer_buffers !== null}
            <div>
              <span class="muted small">{i18n.t("FB buffers:")}</span>
              <code class="mono pill">{display.max_frame_buffer_buffers}</code>
            </div>
          {/if}
        </div>
      </div>

      <div class="card" style="margin-top: 1rem;">
        <h3>{i18n.t("Set refresh rate")}</h3>
        <div class="form-grid two">
          <label>
            {i18n.t("Min refresh rate (Hz)")}
            <input type="number" step="0.1" min="1" max="240" bind:value={minRateInput} />
          </label>
          <label>
            {i18n.t("Peak refresh rate (Hz)")}
            <input type="number" step="0.1" min="1" max="240" bind:value={peakRateInput} />
          </label>
        </div>
        <div class="preset-row">
          <span class="muted small">{i18n.t("Presets:")}</span>
          <button onclick={() => setRatePreset(60, 60)}>{i18n.t("Fixed 60")}</button>
          <button onclick={() => setRatePreset(60, 90)}>60–90 LTPO</button>
          <button onclick={() => setRatePreset(60, 120)}>60–120 LTPO</button>
          <button onclick={() => setRatePreset(120, 120)}>{i18n.t("Fixed 120")}</button>
          <button onclick={() => setRatePreset(1, 120)}>{i18n.t("1–120 (Pixel max LTPO)")}</button>
        </div>
        <button class="primary" style="margin-top: 1rem;" onclick={applyRefreshRate} disabled={applyBusy}>
          {applyBusy ? 'Applying…' : i18n.t("Apply refresh rate")}
        </button>
      </div>

      <div class="card" style="margin-top: 1rem;">
        <h3>{i18n.t("Display Overrides (WM)")}</h3>
        <p class="muted small" style="margin-bottom: 1rem;">{i18n.t("Modify pixel density and internal resolution. Be careful, extreme values can render the UI unusable.")}</p>
        
        <div style="display: flex; gap: 1rem; align-items: flex-end; flex-wrap: wrap; margin-bottom: 1rem;">
          <div>
            <span class="small muted" style="display:block; margin-bottom:0.3rem;">{i18n.t("Custom Density (DPI)")}</span>
            <div style="display: flex; gap: 0.5rem; align-items: center;">
              <input type="number" placeholder="e.g. 420" bind:value={customDensity} disabled={applyBusy} style="width: 120px;" />
              <button class="btn" onclick={applyDensity} disabled={!customDensity || applyBusy}>{i18n.t("Set DPI")}</button>
            </div>
          </div>
          <div>
            <span class="small muted" style="display:block; margin-bottom:0.3rem;">{i18n.t("Custom Resolution")}</span>
            <div style="display: flex; gap: 0.5rem; align-items: center;">
              <input type="text" placeholder="e.g. 1080x2400" bind:value={customResolution} disabled={applyBusy} style="width: 150px;" />
              <button class="btn" onclick={applyResolution} disabled={!customResolution || applyBusy}>{i18n.t("Set Resolution")}</button>
            </div>
          </div>
        </div>

        <button class="btn outline" onclick={resetDisplay} disabled={applyBusy}>{i18n.t("Restore Default Display Metrics")}</button>
      </div>

      <div class="card" style="margin-top: 1rem;">
        <h3 style="margin: 0 0 0.75rem;">{i18n.t("Reduce Visual Load")}</h3>
        <p class="muted small" style="margin: 0 0 1rem;">{i18n.t("Disable window blurs and transparency to reduce GPU workload and improve UI responsiveness. Recommended for mid-range and budget devices.")}</p>
        <div style="display: flex; gap: 0.5rem;">
            <button class="btn" onclick={() => toggleWindowBlurs(true)} disabled={visualLoadBusy}>{i18n.t("Disable Blurs")}</button>
            <button class="btn outline" onclick={() => toggleWindowBlurs(false)} disabled={visualLoadBusy}>{i18n.t("Restore Blurs")}</button>
        </div>
        <div style="display: flex; gap: 0.5rem; margin-top: 0.5rem;">
            <button class="btn" onclick={() => toggleReduceTransparency(true)} disabled={visualLoadBusy}>{i18n.t("Reduce Transparency")}</button>
            <button class="btn outline" onclick={() => toggleReduceTransparency(false)} disabled={visualLoadBusy}>{i18n.t("Restore Transparency")}</button>
        </div>
      </div>
    {/if}

  {:else if tab === 'performance'}
    <div class="card flat banner">
      <p>{i18n.t("Android forwards volume slider input directly to Bluetooth headset firmware. For DACs/DAPs that prefer software volume control (Fiio, Topping, Astell&Kern), disabling Absolute Volume restores Android's own attenuation curve.")}</p>
    </div>

    {#if !display}
      <div class="card"><Skeleton lines={3} /></div>
    {:else}
      <div class="card audio-card">
        <div>
          <h3 style="margin: 0 0 0.4rem 0;">{i18n.t("Bluetooth Absolute Volume")}</h3>
          <p class="muted" style="margin: 0;">
            {i18n.t('Currently:')}
            <code class="mono pill" data-state={btAbsVolDisabled ? 'off' : 'on'}>
              {btAbsVolDisabled ? i18n.t("DISABLED (software volume)") : i18n.t("ENABLED (Android default)")}
            </code>
          </p>
        </div>
        <button
          class={btAbsVolDisabled ? 'primary' : 'danger'}
          onclick={toggleBtAbsVol}
          disabled={audioBusy}
        >
          {audioBusy ? '…' : (btAbsVolDisabled ? i18n.t("Re-enable") : i18n.t("Disable"))}
        </button>
      </div>
      <p class="muted footnote">
        {i18n.t('⚠ This setting only takes effect after re-pairing the Bluetooth device. Unpair the headset, toggle airplane mode, then pair again.')}
      </p>

      <div class="card audio-card" style="margin-top: 1rem;">
        <div>
          <h3 style="margin: 0 0 0.4rem 0;">{i18n.t("Master Mono")}</h3>
          <p class="muted" style="margin: 0;">
            {i18n.t('Currently:')}
            <code class="mono pill" data-state={display.master_mono ? 'on' : 'off'}>
              {display.master_mono ? i18n.t("ENABLED (mono output)") : i18n.t("DISABLED (stereo — default)")}
            </code>
          </p>
          <p class="muted small" style="margin: 0.35rem 0 0;">{i18n.t("Merges left + right channels into one. Useful for single-sided hearing or mono speakers.")}</p>
        </div>
        <button
          class={display.master_mono ? 'primary' : 'danger'}
          onclick={toggleMasterMono}
          disabled={audioBusy}
        >
          {audioBusy ? '…' : (display.master_mono ? i18n.t("Disable mono") : i18n.t("Enable mono"))}
        </button>
      </div>

      <div class="card audio-card" style="margin-top: 1rem;">
        <div>
          <h3 style="margin: 0 0 0.4rem 0;">{i18n.t("Spatial Audio")}</h3>
          <p class="muted" style="margin: 0;">
            {i18n.t('Currently:')}
            {#if display.spatial_audio_enabled === null}
              <code class="mono pill">{i18n.t("Not available on this device")}</code>
            {:else}
              <code class="mono pill" data-state={display.spatial_audio_enabled ? 'on' : 'off'}>
                {display.spatial_audio_enabled ? i18n.t("ENABLED") : i18n.t("DISABLED")}
              </code>
            {/if}
          </p>
          <p class="muted small" style="margin: 0.35rem 0 0;">{i18n.t("Head-tracking 3D audio processing. Disable if you experience latency or prefer flat stereo output.")}</p>
        </div>
        {#if display.spatial_audio_enabled !== null}
          <button
            class={display.spatial_audio_enabled ? 'danger' : 'primary'}
            onclick={toggleSpatialAudio}
            disabled={audioBusy}
          >
            {audioBusy ? '…' : (display.spatial_audio_enabled ? i18n.t("Disable") : i18n.t("Enable"))}
          </button>
        {/if}
      </div>

      <div class="card" style="margin-top: 1rem; padding: 1.25rem;">
        <h3 style="margin: 0 0 0.75rem;">{i18n.t("AVRCP Version")}</h3>
        <p class="muted small" style="margin: 0 0 0.85rem;">
          {i18n.t('Audio/Video Remote Control Profile version. Higher versions support richer metadata (album art, browsing). Lower versions have broader device compatibility.')}
          {i18n.t('Currently:')} <code class="mono pill">{display.avrcp_version ?? 'default'}</code>
        </p>
        <div class="preset-row">
          <span class="muted small">{i18n.t("Set to:")}</span>
          <button class:active={display.avrcp_version === 'avrcp13'} onclick={() => setAvrcp('avrcp13')} disabled={audioBusy}>1.3</button>
          <button class:active={display.avrcp_version === 'avrcp14'} onclick={() => setAvrcp('avrcp14')} disabled={audioBusy}>1.4</button>
          <button class:active={display.avrcp_version === 'avrcp15'} onclick={() => setAvrcp('avrcp15')} disabled={audioBusy}>1.5</button>
          <button class:active={display.avrcp_version === 'avrcp16'} onclick={() => setAvrcp('avrcp16')} disabled={audioBusy}>1.6</button>
        </div>
        <p class="muted footnote">{i18n.t("⚠ Requires re-pairing the Bluetooth device to take effect.")}</p>
      </div>
    {/if}
    <h2 style="margin-top: 2.5rem; margin-bottom: 0.75rem;">{i18n.t("System Performance")}</h2>
    <div class="row-actions">
      <button class="primary" onclick={fetchPerf} disabled={loadingPerf}>
        {loadingPerf ? 'Reading…' : 'Refresh'}
      </button>
    </div>
    {#if errorPerf}<div class="error">{errorPerf}</div>{/if}
    {#if !perfSettings}
      <div class="card"><Skeleton lines={4} /></div>
    {:else}
      <div class="grid form-grid two">
        <div class="card" style="padding: 1.25rem;">
          <h3 style="margin: 0 0 0.75rem;">{i18n.t("Animation Scale")}</h3>
          <p class="muted small" style="margin: 0 0 0.85rem;">
            {i18n.t('Modifies window, transition, and animator durations. 0.5x makes the device feel significantly faster.')}
            {i18n.t('Currently:')} <code class="mono pill">{perfSettings.window_animation_scale ?? '1.0'}x</code>
          </p>
          <div class="preset-row">
            <button class:active={perfSettings.window_animation_scale === 1.0 || perfSettings.window_animation_scale === null} onclick={() => setAnimScale(1.0)} disabled={perfBusy}>{i18n.t("Stock (1.0x)")}</button>
            <button class:active={perfSettings.window_animation_scale === 0.5} onclick={() => setAnimScale(0.5)} disabled={perfBusy}>{i18n.t("Snappy (0.5x)")}</button>
            <button class:active={perfSettings.window_animation_scale === 0.0} onclick={() => setAnimScale(0.0)} disabled={perfBusy}>{i18n.t("Instant (0.0x)")}</button>
          </div>
        </div>
        
        <div class="card" style="padding: 1.25rem;">
          <h3 style="margin: 0 0 0.75rem;">{i18n.t("Fixed Performance Mode")}</h3>
          <p class="muted small" style="margin: 0 0 0.85rem;">
            {i18n.t('Locks CPU/GPU clocks to a high fixed state to prevent thermal throttling. Use only for gaming sessions. Turn off for normal use.')}
          </p>
          <div style="display: flex; gap: 0.5rem; margin-top: 1rem;">
            <button class="btn" onclick={() => toggleFixedPerformanceMode(true)} disabled={fixedPerfBusy}>{i18n.t("Enable Fixed Performance")}</button>
            <button class="btn outline" onclick={() => toggleFixedPerformanceMode(false)} disabled={fixedPerfBusy}>{i18n.t('Disable')}</button>
          </div>
        </div>
        <div class="card" style="padding: 1.25rem;">
          <h3 style="margin: 0 0 0.75rem;">{i18n.t("Background Process Limit")}</h3>
          <p class="muted small" style="margin: 0 0 0.85rem;">
            {i18n.t('Limits the number of cached background processes. Useful for devices with very low RAM.')}
            {i18n.t('Currently:')} <code class="mono pill">{perfSettings.background_process_limit ?? 'Standard'}</code>
          </p>
          <div class="preset-row">
            <button class:active={perfSettings.background_process_limit === null} onclick={() => setBgLimit(null)} disabled={perfBusy}>{i18n.t("Standard")}</button>
            <button class:active={perfSettings.background_process_limit === 4} onclick={() => setBgLimit(4)} disabled={perfBusy}>{i18n.t("At most 4")}</button>
            <button class:active={perfSettings.background_process_limit === 2} onclick={() => setBgLimit(2)} disabled={perfBusy}>{i18n.t("At most 2")}</button>
            <button class:active={perfSettings.background_process_limit === 0} onclick={() => setBgLimit(0)} disabled={perfBusy}>{i18n.t("No bg processes")}</button>
          </div>
        </div>
      </div>
    {/if}
  {:else if tab === 'history'}
    <BatteryHistory />
  {/if}
{/if}
<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 540px; }
  .seg {
    display: inline-flex; gap: 2px; padding: 3px;
    background: var(--control-bg); border: 1px solid var(--border); border-radius: 99px;
    margin-bottom: 1rem;
  }
  .seg button {
    padding: 0.45rem 1rem; border-radius: 99px;
    background: transparent; border: none; color: var(--fg-2);
    font-size: var(--font-size-sm); font-weight: 500;
  }
  .seg button.active {
    background: var(--bg-4); color: var(--fg-0);
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }
  .success {
    padding: 0.65rem 1rem; background: rgba(16, 185, 129, 0.1);
    border-left: 3px solid var(--good); border-radius: var(--radius);
    color: var(--good); margin-bottom: 1rem; font-size: var(--font-size-sm);
  }
  .banner {
    padding: 0.65rem 1rem; border-color: rgba(255, 107, 0, 0.2);
    background: rgba(255, 107, 0, 0.04); margin-bottom: 1rem;
  }
  .banner p { margin: 0; font-size: var(--font-size-sm); color: var(--fg-1); }
  .row-actions { display: flex; justify-content: flex-end; margin-bottom: 0.85rem; }
  .hero-grid { grid-template-columns: 360px 1fr; align-items: stretch; }
  @media (max-width: 920px) { .hero-grid { grid-template-columns: 1fr; } }
  .ring-card { display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 1.5rem; }
  .rings-stack { display: flex; flex-direction: column; align-items: center; gap: 1rem; }
  .big-ring { width: 240px; height: 240px; }
  .arc { transition: stroke-dashoffset 800ms cubic-bezier(0.16, 1, 0.3, 1); }
  .big-pct { font-size: 36px; font-weight: 700; fill: var(--fg-0); font-family: var(--font-mono); letter-spacing: -0.025em; }
  .big-pct.big-muted, .big-num.ring-muted { fill: var(--fg-3); color: var(--fg-3); }
  .big-sub { font-size: 10px; fill: var(--fg-3); text-transform: uppercase; letter-spacing: 0.1em; }
  .big-note { font-size: 8px; fill: var(--fg-4, var(--fg-3)); text-transform: uppercase; letter-spacing: 0.08em; opacity: 0.7; }
  .legend { display: flex; flex-direction: column; gap: 6px; font-size: var(--font-size-xs); color: var(--fg-2); }
  .legend span { display: flex; align-items: center; gap: 8px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
  .stats-col { display: grid; gap: 0.85rem; grid-template-columns: 1fr 1fr; }
  .stat-tile { padding: 1rem 1.15rem; }
  .stat-label { font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--fg-3); font-weight: 600; margin-bottom: 0.5rem; }
  .big-num { font-size: 26px; font-weight: 700; letter-spacing: -0.02em; color: var(--fg-0); line-height: 1.1; }
  .big-num .unit { font-size: 13px; font-weight: 400; }
  .small { font-size: var(--font-size-xs); margin-top: 0.4rem; }
  .status-row { display: flex; gap: 0.5rem; flex-wrap: wrap; margin: 0.3rem 0; }
  .thermo-card { margin-top: 1rem; padding: 1.25rem 1.5rem; }
  .thermo-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
  .thermo-header h3 { margin: 0; }
  .thermo-value { font-size: 28px; font-weight: 700; letter-spacing: -0.02em; }
  .thermo-bar { width: 100%; }
  .thermo-track { position: relative; height: 14px; border-radius: 7px; overflow: hidden; background: var(--bg-3); }
  .thermo-gradient { position: absolute; inset: 0; background: linear-gradient(90deg, #38BDF8 0%, #10B981 30%, #F59E0B 70%, #EF4444 100%); opacity: 0.85; }
  .thermo-marker { position: absolute; top: -3px; width: 4px; height: 20px; background: white; border-radius: 2px; box-shadow: 0 0 0 1px var(--bg-0), 0 0 8px rgba(0, 0, 0, 0.8); transition: left 800ms cubic-bezier(0.16, 1, 0.3, 1); }
  .thermo-scale { display: flex; justify-content: space-between; margin-top: 0.45rem; font-size: 10.5px; color: var(--fg-3); font-family: var(--font-mono); }
  .source-note { font-size: var(--font-size-xs); margin: 1rem 0 0; }
  .dns-current { display: flex; gap: 1.5rem; flex-wrap: wrap; margin-top: 0.5rem; }
  .dns-current .small { font-size: var(--font-size-xs); }
  .pill { display: inline-block; padding: 2px 8px; border-radius: 99px; background: var(--bg-3); font-size: var(--font-size-xs); }
  .pill[data-state="off"] { color: var(--good); background: rgba(16, 185, 129, 0.1); }
  .pill[data-state="on"]  { color: var(--fg-2); }
  .form-grid { display: grid; gap: 1rem; }
  .form-grid.two { grid-template-columns: 1fr 1fr; }
  @media (max-width: 600px) { .form-grid.two { grid-template-columns: 1fr; } }
  .form-grid label { display: flex; flex-direction: column; gap: 4px; font-size: var(--font-size-sm); color: var(--fg-2); }
  .preset-row { display: flex; align-items: center; gap: 0.45rem; flex-wrap: wrap; margin-top: 1rem; }
  .preset-row button { font-size: var(--font-size-xs); padding: 0.35rem 0.75rem; }
  .audio-card { display: flex; justify-content: space-between; align-items: center; gap: 1rem; padding: 1.25rem; }
  .footnote { font-size: var(--font-size-xs); margin: 0.85rem 0 0; }

  /* ---------- By-app drain ---------- */
  .drain-kpi {
    grid-template-columns: repeat(4, 1fr);
    gap: 0.75rem;
    margin-top: 1rem;
  }
  @media (max-width: 900px) { .drain-kpi { grid-template-columns: 1fr 1fr; } }
  @media (max-width: 500px) { .drain-kpi { grid-template-columns: 1fr; } }

  .bad-num  { color: var(--bad); }
  .warn-num { color: var(--warn); }

  .seg-small button {
    font-size: var(--font-size-xs);
    padding: 0.35rem 0.75rem;
  }

  .filter-input {
    padding: 0.4rem 0.75rem;
    background: var(--bg-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--fg-0);
    font-size: var(--font-size-sm);
    min-width: 220px;
  }
  .filter-input::placeholder { color: var(--fg-3); }

  .charge-note {
    background: var(--warn-soft, rgba(245, 158, 11, 0.1));
    border: 1px solid rgba(245, 158, 11, 0.28);
    color: var(--fg-2); font-size: var(--font-size-xs); line-height: 1.5;
    padding: 0.6rem 0.85rem; border-radius: var(--radius); margin-bottom: 1rem;
  }
  .share-bar {
    position: relative;
    width: 110px;
    height: 18px;
    background: var(--bg-3);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .share-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: linear-gradient(90deg, var(--accent), var(--warn));
    transition: width 200ms ease-out;
  }
  .share-text {
    position: relative;
    z-index: 1;
    display: block;
    text-align: right;
    padding: 0 6px;
    line-height: 18px;
    font-size: var(--font-size-xs);
    color: var(--fg-0);
    mix-blend-mode: difference;
  }

  tr.row-bad td:first-child { border-left: 2px solid var(--bad); padding-left: calc(0.65rem - 2px); }
  tr.app-row:hover { background: var(--bg-3); }
  tr.app-row:hover td { color: var(--fg-0); }

  .live-dot::before {
    content: '';
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    margin-right: 5px;
    animation: pulse-dot 1.6s infinite;
  }
  @keyframes pulse-dot {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .preset-row button.active {
    background: var(--accent);
    color: var(--on-accent);
    border-color: var(--accent);
  }
</style>