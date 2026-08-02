<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import { appModalStore } from '$stores/appModal.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import { toast } from '$stores/toast.svelte';
  import AppName from '$components/AppName.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import type { ProcessRow, ProcessSnapshot, ProcessState, TelemetryTick } from '$types';
  import type { UnlistenFn } from '@tauri-apps/api/event';

  let snap = $state<ProcessSnapshot | null>(null);
  let thermal = $state<{ raw_value: number, label: string, temperature: number | null } | null>(null);
  let lastTickTs = $state<number | null>(null);
  let cpuHistory = $state<number[]>([]);
  let streaming = $state(false);
  let error = $state<string | null>(null);

  // Filters and Grouping
  let filter = $state('');
  let stateFilter = $state<'all' | 'zombie' | 'hog' | 'running' | 'background' | 'foreground' | 'system' | 'user' | 'high_cpu' | 'high_ram'>('all');
  let viewMode = $state<'threads' | 'apps'>('threads');

  let expandedRowId = $state<number | string | null>(null);
  let actionBusy = $state(false);
  let unsubscribe: UnlistenFn | null = null;

  async function startStream() {
    if (!deviceStore.selected || streaming) return;
    error = null;
    try {
      unsubscribe = await api.onTelemetryTick((tick: TelemetryTick) => {
        snap = tick.snapshot;
        lastTickTs = tick.ts_ms;
        if (tick.cpu_history) cpuHistory = tick.cpu_history;
      });
      await api.startTelemetryStream(deviceStore.selected.serial, 3);
      streaming = true;
      try {
        snap = await api.processStatus(deviceStore.selected.serial);
        thermal = await api.getThermalStatus(deviceStore.selected.serial);
        lastTickTs = Date.now();
      } catch {}
    } catch (e) {
      error = (e as DozeForgeError).message;
    }
  }

  async function stopStream() {
    try { await api.stopTelemetryStream(); } catch {}
    if (unsubscribe) { unsubscribe(); unsubscribe = null; }
    streaming = false;
  }

  onMount(() => { if (deviceStore.selected?.state === 'device') startStream(); });
  onDestroy(() => { stopStream(); });

  const filtered = $derived.by<ProcessRow[]>(() => {
    if (!snap) return [];
    const serial = deviceStore.selected?.serial ?? null;
    const needle = filter.toLowerCase();
    
    let rows = snap.rows.filter((r) => {
      if (needle) {
        const label = r.package ? labelStore.labelFor(serial, r.package).toLowerCase() : '';
        const hay = `${r.package ?? ''} ${label} ${r.args}`.toLowerCase();
        if (!hay.includes(needle)) return false;
      }
      if (stateFilter === 'zombie' && !r.is_zombie) return false;
      if (stateFilter === 'hog' && !r.is_smart_hog && !r.is_hog_candidate) return false;
      if (stateFilter === 'running' && r.state !== 'running') return false;
      if (stateFilter === 'system' && !r.package) return false; // Crude approximation
      if (stateFilter === 'user' && r.package === null) return false;
      if (stateFilter === 'high_cpu' && r.cpu_percent < 5) return false;
      if (stateFilter === 'high_ram' && r.rss_kb < 102400) return false;
      return true;
    });

    if (viewMode === 'apps') {
      const grouped = new Map<string, ProcessRow>();
      for (const r of rows) {
        if (!r.package) continue;
        const existing = grouped.get(r.package);
        if (existing) {
          existing.cpu_percent += r.cpu_percent;
          existing.rss_kb += r.rss_kb;
          if (r.is_zombie) existing.is_zombie = true;
          if (r.is_smart_hog) existing.is_smart_hog = true;
          existing.args = `${existing.args.split(' ')[0]} (+ threads)`;
        } else {
          grouped.set(r.package, { ...r });
        }
      }
      rows = Array.from(grouped.values());
      rows.sort((a, b) => b.cpu_percent - a.cpu_percent);
    }

    return rows;
  });

  const topConsumer = $derived(snap?.rows.length ? snap.rows.reduce((prev, curr) => (prev.cpu_percent > curr.cpu_percent) ? prev : curr) : null);

  function stateChar(s: ProcessState): string {
    switch (s) {
      case 'running': return 'R';
      case 'sleeping': return 'S';
      case 'uninterruptiblesleep': return 'D';
      case 'zombie': return 'Z';
      case 'stopped': return 'T';
      default: return '?';
    }
  }

  function fmtRss(kb: number | undefined): string {
    if (!kb) return '0 MB';
    if (kb < 1024) return `${kb.toFixed(0)} KB`;
    return `${(kb / 1024).toFixed(1)} MB`;
  }

  // --- Actions ---
  async function restrict(pkg: string | null) {
    if (!pkg) return;
    appModalStore.open(pkg);
  }

  async function killAllZombies() {
    if (!deviceStore.selected) return;
    actionBusy = true; error = null;
    try {
      await invoke('kill_all_zombies', { serial: deviceStore.selected.serial });
      toast.success(i18n.t('All zombie processes have been terminated.'));
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { actionBusy = false; }
  }

  async function killProcess(pid: number, name: string) {
    if (!deviceStore.selected || !confirm(i18n.t('Force kill process {{name}} (PID {{pid}})? This may cause app crashes or system instability.', { name, pid }))) return;
    actionBusy = true; error = null;
    try {
      await invoke('run_shell', { serial: deviceStore.selected.serial, command: `kill -9 ${pid}` });
      toast.success(i18n.t('Sent kill signal to PID {{pid}}.', { pid }));
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { actionBusy = false; }
  }

  async function trimMemory() {
    if (!deviceStore.selected) return;
    actionBusy = true; error = null;
    try {
      await invoke('trim_memory', { serial: deviceStore.selected.serial });
      toast.success(i18n.t('System-wide memory trim requested.'));
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { actionBusy = false; }
  }

  async function toggleFixedPerf() {
    if (!deviceStore.selected) return;
    actionBusy = true; error = null;
    try {
      await api.setFixedPerformanceMode(deviceStore.selected.serial, true);
      toast.success(i18n.t('Fixed Performance Mode enabled.'));
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { actionBusy = false; }
  }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Telemetry')}</h1>
    <p class="muted">
      {i18n.t('Live process monitor & optimization. Polled every 3s.')}
      <span class="badge" class:ok={streaming} class:elevated={!streaming}>
        {streaming ? i18n.t('● Streaming') : i18n.t('○ Paused')}
      </span>
    </p>
  </div>
  <div class="head-actions">
    {#if streaming}
      <button onclick={stopStream}>{i18n.t('Pause')}</button>
    {:else}
      <button class="primary" onclick={startStream} disabled={!deviceStore.selected}>{i18n.t('Start')}</button>
    {/if}
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else}
  {#if error}<div class="error">{error}</div>{/if}

  <div class="stat-grid">
    <!-- CPU Breakdown -->
    <div class="stat-card">
      <div class="stat-label">{i18n.t('CPU Breakdown')}</div>
      <div class="stat-value">{snap ? (snap.cpu_user + snap.cpu_sys + snap.cpu_iowait).toFixed(1) : '—'}%</div>
      <div class="stat-sub">
        <span style="color:var(--good)">{i18n.t('User')}: {snap?.cpu_user?.toFixed(1) ?? 0}%</span> &nbsp;
        <span style="color:var(--warn)">{i18n.t('Sys')}: {snap?.cpu_sys?.toFixed(1) ?? 0}%</span> &nbsp;
        <span style="color:var(--bad)">{i18n.t('IO')}: {snap?.cpu_iowait?.toFixed(1) ?? 0}%</span>
      </div>
    </div>

    <!-- RAM & Swap -->
    <div class="stat-card">
      <div class="stat-label">{i18n.t('Mem Available & Swap')}</div>
      <div class="stat-value">{snap ? `${snap.mem_available_mb} MB` : '—'}</div>
      <div class="stat-sub">
        {i18n.t('Swap Used')}: {snap ? `${snap.swap_total_mb - snap.swap_free_mb} MB` : '0 MB'} / {snap ? `${snap.swap_total_mb} MB` : '0 MB'}
      </div>
    </div>

    <!-- Thermal & Top -->
    <div class="stat-card">
      <div class="stat-label">{i18n.t('Thermal Status')}</div>
      <div class="stat-value" style="color: {thermal && thermal.raw_value >= 3 ? 'var(--bad)' : 'var(--fg-0)'}">
        {thermal?.temperature ? `${thermal.temperature}°C` : (thermal?.label ?? i18n.t('Cool'))}
      </div>
      <div class="stat-sub">{i18n.t('Top')}: {topConsumer?.package ? labelStore.labelFor(deviceStore.selected.serial, topConsumer.package) : (topConsumer?.args.split(' ')[0] ?? '—')} ({topConsumer?.cpu_percent.toFixed(1) ?? 0}%)</div>
    </div>

    <!-- CPU Trend -->
    <div class="stat-card">
      <div class="stat-label">{i18n.t('CPU Trend (3 min)')}</div>
      <div class="spark-container">
        <svg viewBox="0 0 60 20" class="sparkline" preserveAspectRatio="none">
          {#if cpuHistory.length > 0}
            <polyline fill="none" stroke="var(--accent)" stroke-width="1.5"
                      points={cpuHistory.map((val, i) => `${i},${Math.max(0, 20 - (val / 100) * 20)}`).join(' ')} />
          {/if}
        </svg>
      </div>
    </div>
  </div>

  <div class="mass-actions" style="margin-bottom: 1rem; display: flex; gap: 0.5rem; flex-wrap: wrap;">
    <button class="primary" onclick={killAllZombies} disabled={actionBusy || !snap?.zombie_count}>{i18n.t('Kill all zombies')} ({snap?.zombie_count ?? 0})</button>
    <button class="primary outline" onclick={trimMemory} disabled={actionBusy}>{i18n.t('Trim Memory')}</button>
    <button class="primary outline" onclick={toggleFixedPerf} disabled={actionBusy}>{i18n.t('Fixed Performance Mode')}</button>
  </div>

  <div class="card filter-bar">
    <input type="search" placeholder={i18n.t('Filter by package...')} bind:value={filter} />
    <label style="margin-right: auto; margin-left: 0.5rem; display: flex; align-items: center; gap: 0.35rem; font-size: var(--font-size-xs); cursor: pointer; color: var(--fg-2);">
      <input type="checkbox" checked={viewMode === 'apps'} onchange={(e) => viewMode = e.currentTarget.checked ? 'apps' : 'threads'} style="width: auto;">
      {i18n.t('Group by App')}
    </label>
    <div class="filter-pills">
      <select bind:value={stateFilter} style="padding: 0.35rem 0.5rem; font-size: var(--font-size-xs); background: var(--bg-3); border: 1px solid var(--border); border-radius: 4px; color: var(--fg-0);">
        <option value="all">{i18n.t('All Processes')}</option>
        <option value="running">{i18n.t('Running (R)')}</option>
        <option value="zombie">{i18n.t('Zombies')}</option>
        <option value="hog">{i18n.t('Smart Hogs')}</option>
        <option value="high_cpu">{i18n.t('High CPU (>5%)')}</option>
        <option value="high_ram">{i18n.t('High RAM (>100MB)')}</option>
      </select>
    </div>
  </div>

  {#if !snap}
    <div class="card"><Skeleton lines={10} /></div>
  {:else}
    <div class="card table-card">
      <div class="scroll-y" style="max-height: 60vh;">
        <table class="proc-table">
          <thead>
            <tr>
              <th>{i18n.t('PID')}</th>
              <th>{i18n.t('S')}</th>
              <th>{i18n.t('%CPU')}</th>
              <th>{i18n.t('RSS')}</th>
              <th>{i18n.t('Package / Args')}</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each filtered as row (row.pid)}
              <!-- Main Row -->
              <tr class:zombie={row.is_zombie} class:hog={row.is_smart_hog || row.is_hog_candidate} class:expanded={expandedRowId === row.pid}
                  onclick={() => expandedRowId = expandedRowId === row.pid ? null : row.pid}>
                <td class="mono pid">{viewMode === 'apps' ? '—' : row.pid}</td>
                <td><span class="state-badge" data-state={row.state}>{stateChar(row.state)}</span></td>
                <td class="mono">
                  <div class="inline-bar" style="--bar-pct: {Math.min(100, (row.cpu_percent / 20) * 100)}%; --bar-color: {row.cpu_percent > 50 ? 'var(--bad)' : (row.cpu_percent > 20 ? 'var(--warn)' : 'var(--accent-dim)')}">
                    <div class="inline-bar-fill"></div>
                    <span class="inline-bar-text">{row.cpu_percent.toFixed(1)}%</span>
                  </div>
                </td>
                <td class="mono">
                  <div class="inline-bar" style="width: 75px; --bar-pct: {Math.min(100, (row.rss_kb / 1024 / 500) * 100)}%; --bar-color: {row.rss_kb > 500 * 1024 ? 'var(--warn)' : 'var(--fg-3)'}">
                    <div class="inline-bar-fill" style="opacity: 0.3;"></div>
                    <span class="inline-bar-text">{fmtRss(row.rss_kb)}</span>
                  </div>
                </td>
                <td class="args">
                  {#if row.package}
                    <AppName package={row.package} size="sm" hidePackage inline />
                  {:else}
                    <span class="muted mono">{row.args}</span>
                  {/if}
                </td>
                <td>
                  {#if row.is_zombie}<span class="badge critical">{i18n.t('ZOMBIE')}</span>{/if}
                  {#if row.is_smart_hog || row.is_hog_candidate}<span class="badge elevated">{i18n.t('HOG')}</span>{/if}
                </td>
              </tr>
              <!-- Expandable Inline Action Row -->
              {#if expandedRowId === row.pid}
                <tr class="inline-expanded-row">
                  <td colspan="6">
                    <div class="expanded-panel">
                      <div class="expanded-stats">
                        <div><strong>{i18n.t('PID')}:</strong> {row.pid} | <strong>{i18n.t('User')}:</strong> {row.user}</div>
                        <div><strong>{i18n.t('Full Args')}:</strong> <span class="mono muted">{row.args}</span></div>
                      </div>
                      <div class="expanded-actions">
                        {#if row.package}
                          <button class="primary outline" onclick={() => restrict(row.package)}>{i18n.t('Optimize App Options')}</button>
                        {/if}
                        <button class="danger outline" onclick={() => killProcess(row.pid, row.package || row.args.split(' ')[0] || '')}>{i18n.t('Force Kill Process')}</button>
                      </div>
                    </div>
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; }
  .page-head p { margin: 0; display: flex; align-items: center; gap: 0.5rem; }
  
  .stat-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.75rem;
    margin-bottom: 1rem;
  }
  .stat-card {
    background: var(--card-bg);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    padding: 0.85rem;
    display: flex;
    flex-direction: column;
  }
  .stat-label { font-size: 10px; text-transform: uppercase; color: var(--fg-3); margin-bottom: 4px; font-weight: 700; letter-spacing: 0.05em; }
  .stat-value { font-family: var(--font-mono); font-size: 20px; font-weight: 700; color: var(--fg-0); margin-bottom: 4px; }
  .stat-sub { font-size: 11px; color: var(--fg-2); }
  
  .spark-container { height: 28px; width: 100%; margin-top: 4px; }
  .sparkline { width: 100%; height: 100%; }

  .filter-bar { display: flex; gap: 1rem; align-items: center; margin-bottom: 0.85rem; padding: 0.6rem 0.85rem; }
  .filter-bar input[type="search"] { flex: 1; max-width: 480px; }

  .table-card { padding: 0.85rem; }
  .proc-table { width: 100%; font-size: 12.5px; border-collapse: collapse; }
  .proc-table th { background: var(--bg-1); position: sticky; top: 0; text-align: left; padding: 0.5rem; z-index: 2; border-bottom: 1px solid var(--border); }
  .proc-table td { padding: 0.4rem 0.5rem; border-bottom: 1px solid var(--border); }
  
  .proc-table tbody tr { cursor: pointer; transition: background 0.1s; }
  .proc-table tbody tr:hover { background: var(--bg-3); }
  .proc-table tbody tr.expanded { background: var(--bg-3); }
  .proc-table tbody tr.zombie { background: rgba(239, 68, 68, 0.05); }
  .proc-table tbody tr.hog { background: rgba(245, 158, 11, 0.05); }
  
  .inline-expanded-row td { padding: 0; border-bottom: 1px solid var(--border); background: var(--bg-2); }
  .expanded-panel { padding: 0.85rem 1rem; display: flex; justify-content: space-between; align-items: flex-start; border-left: 3px solid var(--accent); }
  .expanded-stats { font-size: 12px; color: var(--fg-1); display: flex; flex-direction: column; gap: 0.4rem; }
  .expanded-actions { display: flex; gap: 0.5rem; }

  .pid { color: var(--fg-2); width: 60px; }
  .args { max-width: 480px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .state-badge {
    display: inline-flex; align-items: center; justify-content: center;
    width: 22px; height: 22px; border-radius: 4px; font-family: var(--font-mono);
    font-weight: 700; font-size: 11px; color: var(--fg-2); background: var(--bg-3);
  }
  .state-badge[data-state="zombie"]  { background: var(--bad); color: white; }
  .state-badge[data-state="running"] { background: var(--good); color: var(--on-accent); }
  .state-badge[data-state="uninterruptiblesleep"] { background: var(--warn); color: var(--on-accent); }

  .inline-bar {
    position: relative; display: inline-flex; align-items: center;
    width: 55px; height: 20px; background: var(--bg-3); border-radius: 4px; overflow: hidden;
  }
  .inline-bar-fill {
    position: absolute; left: 0; top: 0; bottom: 0;
    background: var(--bar-color); width: var(--bar-pct, 0%); opacity: 0.8;
  }
  .inline-bar-text { position: relative; z-index: 1; padding-left: 6px; font-size: 11px; font-weight: 600; }
</style>
