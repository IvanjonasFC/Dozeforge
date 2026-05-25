<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';

  let activeTab = $state<'props' | 'logs' | 'bugreport' | 'tools' | 'ram' | 'io'>('props');

  // IO
  let ioStats = $state<any[]>([]);
  let ioLoading = $state(false);
  let ioError = $state<string | null>(null);

  // Live Task Manager (RAM + CPU)
  let ramInfo = $state<any>(null);
  let ramStreaming = $state(false);
  let ramUnlisten: UnlistenFn | null = null;

  // Props
  let props = $state<Record<string, string> | null>(null);
  let propsError = $state<string | null>(null);
  let propsLoading = $state(false);
  let propsFilter = $state('');

  // Thermal
  let thermalInfo = $state<{raw_value: number, label: string} | null>(null);
  let thermalLoading = $state(false);
  let thermalError = $state<string | null>(null);

  // Logs
  let logs = $state<string[]>([]);
  let logMode = $state<'logcat' | 'dmesg'>('logcat');
  let logStreaming = $state(false);
  let logUnlisten: UnlistenFn | null = null;
  let logContainer: HTMLElement | undefined = $state();
  let logHeuristics = $state<{ type: 'ANR' | 'Exception' | 'Crash', msg: string, time: string }[]>([]);

  // Bugreport
  let bugreportText = $state<string | null>(null);
  let bugreportLoading = $state(false);

  async function startRamStream() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device' || ramStreaming) return;
    try {
      ramUnlisten = await api.onRamUpdate((snap) => {
        ramInfo = snap;
      });
      await api.startRamStream(deviceStore.selected.serial);
      ramStreaming = true;
    } catch (e) {
      console.error(e);
    }
  }

  async function stopRamStream() {
    try {
      await api.stopRamStream();
    } catch {}
    if (ramUnlisten) { ramUnlisten(); ramUnlisten = null; }
    ramStreaming = false;
  }

  async function loadIoStats() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    ioLoading = true; ioError = null;
    try {
      ioStats = await api.getIoStats(deviceStore.selected.serial);
    } catch(e) {
      ioError = (e as DozeForgeError).message;
    } finally {
      ioLoading = false;
    }
  }

  async function loadProps() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    propsLoading = true;
    try {
      props = await api.getSystemProperties(deviceStore.selected.serial);
    } catch (e) {
      propsError = (e as DozeForgeError).message;
    } finally {
      propsLoading = false;
    }
  }

  async function loadThermal() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    thermalLoading = true;
    try {
      thermalInfo = await api.getThermalTelemetry(deviceStore.selected.serial);
    } catch (e) {
      thermalError = (e as DozeForgeError).message;
    } finally {
      thermalLoading = false;
    }
  }

  async function startLogs() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device' || logStreaming) return;
    logs = [];
    logHeuristics = [];
    try {
      logUnlisten = await api.onLogBatch((newLines) => {
        logs.push(...newLines);
        for (const l of newLines) {
          if (l.includes('FATAL EXCEPTION')) {
            logHeuristics.push({ type: 'Exception', msg: l.substring(0, 100), time: new Date().toLocaleTimeString() });
          } else if (l.includes('ANR in ')) {
            logHeuristics.push({ type: 'ANR', msg: l.substring(0, 100), time: new Date().toLocaleTimeString() });
          } else if (l.includes('WIN DEATH')) {
            logHeuristics.push({ type: 'Crash', msg: l.substring(0, 100), time: new Date().toLocaleTimeString() });
          }
        }
        if (logHeuristics.length > 5) logHeuristics.shift();

        if (logs.length > 1000) {
            logs.splice(0, logs.length - 1000);
        }
        if (logContainer) {
          const isScrolledToBottom = logContainer.scrollHeight - logContainer.clientHeight <= logContainer.scrollTop + 50;
          if (isScrolledToBottom && logContainer) {
             setTimeout(() => { if (logContainer) logContainer.scrollTop = logContainer.scrollHeight; }, 10);
          }
        }
      });
      await api.startLogStream(deviceStore.selected.serial, logMode);
      logStreaming = true;
    } catch (e) {
      console.error(e);
    }
  }

  async function stopLogs() {
    try {
      await api.stopLogStream();
    } catch {}
    if (logUnlisten) { logUnlisten(); logUnlisten = null; }
    logStreaming = false;
  }

  async function generateBugreport() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    bugreportLoading = true;
    bugreportText = 'Generating full bugreport (this usually takes 3 to 5 minutes)...';
    try {
      bugreportText = await api.generateBugreport(deviceStore.selected.serial);
    } catch (e) {
      bugreportText = `Error: ${(e as DozeForgeError).message}`;
    } finally {
      bugreportLoading = false;
    }
  }

  // ---- System Actions State & Logic ----
  let actionLoading = $state(false);
  let actionSuccess = $state<string | null>(null);
  let actionError = $state<string | null>(null);

  // Reboots
  async function rebootDevice(mode: string) {
    if (!deviceStore.selected) return;
    actionLoading = true; actionSuccess = null; actionError = null;
    try {
      await api.rebootDevice(deviceStore.selected.serial, mode);
      actionSuccess = `Rebooting to ${mode}...`;
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }


  async function resetDisplay() {
    if (!deviceStore.selected) return;
    actionLoading = true; actionSuccess = null; actionError = null;
    try {
      await api.resetDisplay(deviceStore.selected.serial);
      actionSuccess = 'Display metrics restored to factory defaults.';
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  // Sideload APK & Screenshot requires dialog import which we'll do at the top
  // we can use dynamic import so we don't break if dialog isn't statically imported
  async function pickApkAndInstall() {
    if (!deviceStore.selected) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ filters: [{ name: 'APK Files', extensions: ['apk'] }], multiple: false });
      if (!selected || Array.isArray(selected)) return;
      
      actionLoading = true; actionSuccess = null; actionError = null;
      const res = await api.installApk(deviceStore.selected.serial, selected as string, false, true);
      actionSuccess = `Installed successfully: ${res}`;
    } catch (e) {
      actionError = (e as DozeForgeError).message;
    } finally {
      actionLoading = false;
    }
  }

  async function takeScreenshotToPc() {
    if (!deviceStore.selected) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const savePath = await save({ filters: [{ name: 'PNG Image', extensions: ['png'] }] });
      if (!savePath) return;
      
      actionLoading = true; actionSuccess = null; actionError = null;
      await api.captureScreenshot(deviceStore.selected.serial, savePath);
      actionSuccess = `Screenshot saved to ${savePath}`;
    } catch (e) {
      actionError = (e as DozeForgeError).message;
    } finally {
      actionLoading = false;
    }
  }

  async function launchScrcpy() {
    if (!deviceStore.selected) return;
    try {
      await api.launchScrcpy(deviceStore.selected.serial);
    } catch (e) {
      actionError = (e as DozeForgeError).message;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') loadProps();
  });

  onDestroy(() => {
    stopLogs();
    stopRamStream();
  });

  $effect(() => {
    if (deviceStore.selected?.state === 'device' && activeTab === 'props') {
      if (!props && !propsLoading) loadProps();
      if (!thermalInfo && !thermalLoading) loadThermal();
    }
    if (deviceStore.selected?.state === 'device' && activeTab === 'ram' && !ramStreaming) {
      startRamStream();
    }
    if (activeTab !== 'ram' && ramStreaming) {
      stopRamStream();
    }
    if (deviceStore.selected?.state === 'device' && activeTab === 'io' && ioStats.length === 0 && !ioLoading) {
      loadIoStats();
    }
  });

  function getLogClass(line: string) {
    if (line.includes(' E ')) return 'log-e';
    if (line.includes(' W ')) return 'log-w';
    if (line.includes(' I ')) return 'log-i';
    if (line.includes(' D ')) return 'log-d';
    if (line.includes(' F ')) return 'log-f';
    return '';
  }

  const visibleProps = $derived.by(() => {
    if (!props) return [];
    if (!propsFilter) return Object.entries(props);
    const q = propsFilter.toLowerCase();
    return Object.entries(props).filter(([k, v]) =>
      k.toLowerCase().includes(q) || v.toLowerCase().includes(q)
    );
  });

  function highlight(text: string, search: string) {
    if (!search) return text;
    const s = search.toLowerCase();
    const t = text.toLowerCase();
    const idx = t.indexOf(s);
    if (idx === -1) return text;
    const before = text.slice(0, idx);
    const match = text.slice(idx, idx + search.length);
    const after = text.slice(idx + search.length);
    return `${before}<mark>${match}</mark>${after}`;
  }
  
  async function copyProp(k: string, v: string) {
    try {
      await navigator.clipboard.writeText(`${k}=${v}`);
    } catch {}
  }
</script>

<header class="page-head">
  <div>
    <h1>System Info</h1>
    <p class="muted">Deep diagnostics, build properties, and real-time logs.</p>
  </div>
</header>

<div class="tabs">
  <button class:active={activeTab === 'props'} onclick={() => activeTab = 'props'}>Build Props</button>
  <button class:active={activeTab === 'ram'} onclick={() => activeTab = 'ram'}>Live RAM</button>
  <button class:active={activeTab === 'io'} onclick={() => activeTab = 'io'}>Storage I/O</button>
  <button class:active={activeTab === 'logs'} onclick={() => activeTab = 'logs'}>Live Logs</button>
  <button class:active={activeTab === 'bugreport'} onclick={() => activeTab = 'bugreport'}>Bugreport</button>
  <button class:active={activeTab === 'tools'} onclick={() => activeTab = 'tools'}>System Actions</button>
</div>

<div class="tab-content">
  {#if !deviceStore.selected}
    <div class="card p-card"><p class="muted">No device connected.</p></div>
  {:else if activeTab === 'io'}
    <div class="card p-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
        <div>
          <h3>UFS Storage Degradation Monitor</h3>
          <p class="muted small">Shows cumulative read/write bytes per package (requires Root Mode).</p>
        </div>
        <button class="btn" onclick={loadIoStats} disabled={ioLoading}>Refresh</button>
      </div>
      {#if ioLoading && ioStats.length === 0}
        <Skeleton lines={5} />
      {:else if ioError}
        <div class="error">{ioError}</div>
      {:else if ioStats.length > 0}
        <div class="table-container">
          <table class="data-table">
            <thead>
              <tr>
                <th>UID</th>
                <th>Foreground Read</th>
                <th>Foreground Write</th>
                <th>Background Read</th>
                <th>Background Write</th>
              </tr>
            </thead>
            <tbody>
              {#each ioStats.sort((a,b) => (b.bg_write_bytes + b.fg_write_bytes) - (a.bg_write_bytes + a.fg_write_bytes)).slice(0, 50) as stat}
                <tr>
                  <td class="mono">{stat.uid}</td>
                  <td class="mono">{(stat.fg_read_bytes / 1024 / 1024).toFixed(2)} MB</td>
                  <td class="mono">{(stat.fg_write_bytes / 1024 / 1024).toFixed(2)} MB</td>
                  <td class="mono">{(stat.bg_read_bytes / 1024 / 1024).toFixed(2)} MB</td>
                  <td class="mono" style="color: var(--warn);">{(stat.bg_write_bytes / 1024 / 1024).toFixed(2)} MB</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {:else if deviceStore.selected.state === 'bootloader'}
    <div class="card p-card" style="border-left: 3px solid var(--accent);">
      <h3>Fastboot Mode Active</h3>
      <p class="muted">Device is currently in bootloader/fastboot mode.</p>
      <div style="margin-top: 1rem; display: flex; gap: 1rem;">
        <button class="primary" onclick={async () => { if (deviceStore.selected) await api.fastbootReboot(deviceStore.selected.serial); }}>Reboot to System</button>
      </div>
    </div>
  {:else if deviceStore.selected.state !== 'device'}
    <div class="card p-card"><p class="muted">Device is offline or unauthorized.</p></div>
  {:else if activeTab === 'ram'}
    <div class="card p-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
        <div>
          <h3>Live Task Manager</h3>
          <p class="muted small">Real-time CPU and Memory (RSS) usage via ADB top stream. (Deep Sleep not interrupted)</p>
        </div>
        {#if ramStreaming}
          <div class="badge" style="background: var(--accent); color: white; border: none; font-size: 11px;">LIVE</div>
        {/if}
      </div>
      {#if !ramInfo}
        <Skeleton lines={5} />
      {:else}
        <div style="display: flex; gap: 1rem; margin-bottom: 1rem;">
          <div class="stat-box" style="flex: 1; padding: 1rem; background: var(--bg-hover); border-radius: 6px;">
            <div class="muted small">Total CPU (Foreground Apps)</div>
            <div style="font-size: 1.5rem; font-weight: bold;">{ramInfo.total_cpu_percent.toFixed(1)}%</div>
          </div>
          <div class="stat-box" style="flex: 1; padding: 1rem; background: var(--bg-hover); border-radius: 6px;">
            <div class="muted small">Total RSS (Memory)</div>
            <div style="font-size: 1.5rem; font-weight: bold; color: var(--warn);">{(ramInfo.total_rss_kb / 1024).toFixed(0)} MB</div>
          </div>
          <div class="stat-box" style="flex: 1; padding: 1rem; background: var(--bg-hover); border-radius: 6px;">
            <div class="muted small">Zombie Processes</div>
            <div style="font-size: 1.5rem; font-weight: bold; color: var(--good);">{ramInfo.zombie_count}</div>
          </div>
        </div>
        <h4>Top Background Processes</h4>
        <div class="table-container" style="margin-top: 1rem; max-height: 50vh; overflow-y: auto;">
          <table class="data-table">
            <thead style="position: sticky; top: 0; background: var(--bg-1);">
              <tr>
                <th>Package / Command</th>
                <th>PID</th>
                <th>CPU %</th>
                <th>RSS (Memory)</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              {#each ramInfo.rows.slice(0, 30) as proc}
                <tr>
                  <td class="mono" style="font-size: 0.9rem;">{proc.package || proc.args}</td>
                  <td>{proc.pid}</td>
                  <td class="mono">{proc.cpu_percent.toFixed(1)}%</td>
                  <td class="mono">{(proc.rss_kb / 1024).toFixed(1)} MB</td>
                  <td style="text-align: right;">
                    {#if proc.package}
                      <button class="action-btn danger" style="padding: 0.25rem 0.5rem; font-size: 0.8rem;" onclick={async () => {
                        if (!deviceStore.selected) return;
                        await api.forceStopPackage(deviceStore.selected.serial, proc.package);
                      }}>Force Stop</button>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {:else if activeTab === 'props'}
    <div class="card p-card">
      {#if propsLoading || thermalLoading}
        <Skeleton lines={10} />
      {:else if propsError}
        <div class="error">{propsError}</div>
      {:else if props}
        {#if thermalInfo}
          <div class="card" style="margin-bottom: 1rem; padding: 1rem; border: 1px solid var(--accent);">
            <h4 style="margin: 0 0 0.5rem 0;">Thermal Telemetry</h4>
            <div style="display: flex; gap: 1rem;">
              <div class="stat-box" style="flex: 1; padding: 0.75rem; background: var(--bg-hover); border-radius: 6px;">
                <div class="muted small">Thermal Status Label</div>
                <div style="font-size: 1.25rem; font-weight: bold; color: {thermalInfo.raw_value >= 3 ? 'var(--danger)' : 'var(--good)'};">{thermalInfo.label}</div>
              </div>
              <div class="stat-box" style="flex: 1; padding: 0.75rem; background: var(--bg-hover); border-radius: 6px;">
                <div class="muted small">Raw Value</div>
                <div style="font-size: 1.25rem; font-weight: bold;" class="mono">{thermalInfo.raw_value}</div>
              </div>
            </div>
          </div>
        {/if}
        <div class="filter-bar">
          <input type="search" placeholder="Filter properties..." bind:value={propsFilter} />
        </div>
        <div class="scroll-y props-container">
          <div class="props-list">
            {#each visibleProps as [k, v]}
              <div class="prop-item group">
                <div class="prop-key mono">{@html highlight(k, propsFilter)}</div>
                <div class="prop-val mono">{@html highlight(v, propsFilter)}</div>
                <button class="copy-btn" onclick={() => copyProp(k, v)} title="Copy {k}">
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>
                </button>
              </div>
            {/each}
          </div>
          {#if visibleProps.length === 0}
            <div class="muted" style="text-align: center; padding: 2rem;">No properties found for "{propsFilter}"</div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  {#if deviceStore.selected?.state === 'device' && activeTab === 'logs'}
    <div class="card p-card log-card">
      <div class="log-controls">
        <select bind:value={logMode} disabled={logStreaming}>
          <option value="logcat">Logcat (Android OS)</option>
          <option value="dmesg">Dmesg (Kernel)</option>
        </select>
        {#if logStreaming}
          <button onclick={stopLogs} class="danger">Stop Stream</button>
        {:else}
          <button onclick={startLogs} class="primary">Start Stream</button>
        {/if}
        <button onclick={() => logs = []} class="ghost">Clear</button>
        <span class="muted" style="margin-left: auto; font-size: 11px;">Max 1000 lines buffer</span>
      </div>
      {#if logHeuristics.length > 0}
        <div style="margin-bottom: 1rem; border: 1px solid var(--warn); border-radius: 6px; padding: 0.5rem; background: var(--bg-hover);">
          <h4 style="margin: 0 0 0.5rem 0; color: var(--warn); font-size: 0.9rem;">Heuristics: Detected Issues</h4>
          {#each logHeuristics as h}
            <div class="mono small" style="margin-bottom: 0.25rem;">
              <span class="badge" style="background: var(--bg-3); margin-right: 0.5rem;">{h.time}</span>
              <strong style="color: {h.type === 'ANR' ? 'var(--warn)' : 'var(--error)'};">{h.type}:</strong> {h.msg}
            </div>
          {/each}
        </div>
      {/if}
      <div class="log-viewer" bind:this={logContainer}>
        {#if logs.length === 0}
          <div class="empty-logs muted">No logs yet. Press Start Stream.</div>
        {:else}
          {#each logs as line}
            <div class="log-line {getLogClass(line)}">{line}</div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}

  {#if deviceStore.selected?.state === 'device' && activeTab === 'bugreport'}
    <div class="card p-card">
      <h3>System Bugreport</h3>
      <p class="muted" style="margin-bottom: 1.5rem;">Generates a comprehensive ZIP archive with dumpsys, logcat, and kernel metrics. Useful for deep analysis or submitting to Android Issue Tracker.</p>
      <button class="primary" onclick={generateBugreport} disabled={bugreportLoading}>
        {bugreportLoading ? 'Generating (Please wait 3-5 minutes)...' : 'Generate Bugreport'}
      </button>
      
      {#if bugreportText}
        <div class="bugreport-output scroll-y" style="max-height: 50vh;">
          {bugreportText}
        </div>
      {/if}
    </div>
  {/if}

  {#if deviceStore.selected?.state === 'device' && activeTab === 'tools'}
    {#if actionError}
      <div class="card error" style="margin-bottom: 1rem;">{actionError}</div>
    {/if}
    {#if actionSuccess}
      <div class="card ok" style="margin-bottom: 1rem;">{actionSuccess}</div>
    {/if}

    <div class="grid two-grid">
      <!-- Advanced Power Menu -->
      <div class="card">
        <h3>Power Menu</h3>
        <p class="muted small">Reboot the device into different modes via ADB.</p>
        <div style="display: flex; gap: 0.5rem; flex-wrap: wrap; margin-top: 1rem;">
          <button class="btn outline" onclick={() => rebootDevice('system')} disabled={actionLoading}>System</button>
          <button class="btn outline" onclick={() => rebootDevice('recovery')} disabled={actionLoading}>Recovery</button>
          <button class="btn outline" onclick={() => rebootDevice('bootloader')} disabled={actionLoading}>Bootloader</button>
          <button class="btn outline" onclick={() => rebootDevice('download')} disabled={actionLoading}>Download</button>
        </div>
      </div>

      <!-- Sideloading & Capture -->
      <div class="card">
        <h3>Utilities</h3>
        <p class="muted small">Install apps, capture the screen, or launch Screen Mirroring via Scrcpy.</p>
        <div style="display: flex; gap: 0.5rem; flex-wrap: wrap; margin-top: 1rem;">
          <button class="btn primary" onclick={pickApkAndInstall} disabled={actionLoading}>Install APK (Sideload)</button>
          <button class="btn" onclick={takeScreenshotToPc} disabled={actionLoading}>Screenshot to PC</button>
          <button class="btn primary" onclick={launchScrcpy} disabled={actionLoading}>
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 0.5rem;"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>
            Launch Scrcpy (120fps)
          </button>
        </div>
      </div>


    </div>
  {/if}
</div>

<style>
  .page-head { margin-bottom: 1.5rem; display: flex; justify-content: space-between; align-items: flex-end; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; }
  
  .tabs {
    display: flex;
    gap: 0.25rem;
    margin-bottom: 1rem;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0;
  }
  .tabs button {
    background: transparent;
    border: none;
    color: var(--fg-2);
    font-size: var(--font-size-sm);
    font-weight: 600;
    padding: 0.75rem 1.25rem;
    cursor: pointer;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    border-bottom: 2px solid transparent;
    transition: all var(--t-fast);
  }
  .tabs button:hover { background: var(--bg-2); color: var(--fg-1); }
  .tabs button.active {
    color: var(--fg-0);
    border-bottom-color: var(--accent);
  }
  
  .p-card { padding: 1.25rem; }
  
  .filter-bar {
    display: flex;
    gap: 0.75rem;
    margin-bottom: 1rem;
  }
  .filter-bar input[type="search"] {
    width: 100%;
    max-width: 400px;
  }

  .props-container { max-height: 60vh; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-1); }
  .props-list { display: flex; flex-direction: column; }
  .prop-item {
    padding: 0.85rem 1.25rem;
    border-bottom: 1px solid var(--border-soft);
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    transition: background 0.1s;
    position: relative;
  }
  .prop-item:hover { background: var(--bg-2); }
  .prop-item:last-child { border-bottom: none; }
  .prop-key { color: var(--fg-0); font-weight: 600; font-size: 11.5px; word-break: break-all; padding-right: 2rem; }
  .prop-val { color: var(--fg-2); font-size: 11.5px; word-break: break-all; padding-right: 2rem; }
  
  :global(.prop-item mark) {
    background: rgba(245, 158, 11, 0.25);
    color: var(--fg-0);
    border-radius: 2px;
    padding: 0 1px;
  }
  
  .copy-btn {
    position: absolute;
    right: 1rem;
    top: 50%;
    transform: translateY(-50%);
    opacity: 0;
    transition: opacity 0.2s, background 0.1s;
    background: var(--bg-3);
    border: 1px solid var(--border);
    color: var(--fg-2);
    padding: 0.4rem;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .prop-item:hover .copy-btn { opacity: 1; }
  .copy-btn:hover { background: var(--bg-4); color: var(--fg-0); }
  
  .log-card { display: flex; flex-direction: column; gap: 1rem; height: 70vh; }
  .log-controls { display: flex; gap: 0.75rem; align-items: center; }
  .log-controls select {
    padding: 0.4rem 0.75rem;
    background: var(--bg-2);
    color: var(--fg-0);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--font-size-sm);
  }
  .log-viewer {
    flex: 1;
    background: #0f172a; /* Slate 900 */
    border-radius: var(--radius);
    padding: 0.75rem 1rem;
    overflow-y: auto;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: #e2e8f0;
    line-height: 1.4;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.2);
  }
  .log-line { 
    white-space: pre-wrap; 
    word-break: break-all; 
    padding: 1px 0; 
    border-bottom: 1px solid rgba(255,255,255,0.02);
  }
  .log-line:hover { background: rgba(255,255,255,0.03); }
  .log-e { color: #f87171; }
  .log-w { color: #fbbf24; }
  .log-i { color: #38bdf8; }
  .log-d { color: #94a3b8; }
  .log-f { color: #ef4444; font-weight: 700; }
  
  .empty-logs { display: flex; align-items: center; justify-content: center; height: 100%; color: var(--fg-3); }
  
  .bugreport-output {
    margin-top: 1.5rem;
    padding: 1.25rem;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: pre-wrap;
    color: var(--fg-1);
    line-height: 1.5;
  }
</style>
