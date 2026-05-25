<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import AppName from '$components/AppName.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import SlideOver from '$components/SlideOver.svelte';
  import type { ProcessRow, ProcessSnapshot, ProcessState, TelemetryTick } from '$types';
  import type { UnlistenFn } from '@tauri-apps/api/event';

  let snap = $state<ProcessSnapshot | null>(null);
  let thermal = $state<{ raw_value: number, label: string } | null>(null);
  let lastTickTs = $state<number | null>(null);
  let streaming = $state(false);
  let error = $state<string | null>(null);

  let filter = $state('');
  let stateFilter = $state<'all' | 'zombie' | 'hog' | 'running'>('all');
  let hideSystem = $state(true);
  let selected = $state<ProcessRow | null>(null);
  let unsubscribe: UnlistenFn | null = null;

  async function startStream() {
    if (!deviceStore.selected || streaming) return;
    error = null;
    try {
      unsubscribe = await api.onTelemetryTick((tick: TelemetryTick) => {
        snap = tick.snapshot;
        lastTickTs = tick.ts_ms;
      });
      await api.startTelemetryStream(deviceStore.selected.serial, 3);
      streaming = true;
      // Trigger a first synchronous snapshot too so the user doesn't wait 3s
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
    try {
      await api.stopTelemetryStream();
    } catch {}
    if (unsubscribe) { unsubscribe(); unsubscribe = null; }
    streaming = false;
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') startStream();
  });
  onDestroy(() => { stopStream(); });

  const filtered = $derived.by<ProcessRow[]>(() => {
    if (!snap) return [];
    const serial = deviceStore.selected?.serial ?? null;
    const needle = filter.toLowerCase();
    return snap.rows.filter((r) => {
      if (hideSystem && !r.package) return false;
      if (needle) {
        const label = r.package ? labelStore.labelFor(serial, r.package).toLowerCase() : '';
        const hay = `${r.package ?? ''} ${label} ${r.args}`.toLowerCase();
        if (!hay.includes(needle)) return false;
      }
      if (stateFilter === 'zombie' && !r.is_zombie) return false;
      if (stateFilter === 'hog' && !r.is_hog_candidate) return false;
      if (stateFilter === 'running' && r.state !== 'running') return false;
      return true;
    });
  });

  function heatIntensity(cpu: number): number {
    return Math.min(1, cpu / 80);
  }

  function stateChar(s: ProcessState): string {
    switch (s) {
      case 'running': return 'R';
      case 'sleeping': return 'S';
      case 'uninterruptiblesleep': return 'D';
      case 'zombie': return 'Z';
      case 'stopped': return 'T';
      case 'idle': return 'I';
      default: return '?';
    }
  }

  function fmtRss(kb: number): string {
    if (kb < 1024) return `${kb} KB`;
    return `${(kb / 1024).toFixed(1)} MB`;
  }

  function fmtAge(ts: number | null): string {
    if (!ts) return '—';
    const s = Math.floor((Date.now() - ts) / 1000);
    if (s < 60) return `${s}s ago`;
    return `${Math.floor(s / 60)}m ago`;
  }

  async function restrict(pkg: string | null) {
    if (!pkg) return;
    selected = null;
    goto(`/actions/?pkg=${encodeURIComponent(pkg)}`);
  }

  let actionBusy = $state(false);

  async function hibernateApp(pkg: string | null) {
    if (!pkg || !deviceStore.selected) return;
    actionBusy = true; error = null;
    try {
      await api.hibernatePackage(deviceStore.selected.serial, pkg, true);
      alert(`Hibernated ${pkg}. It will be force-stopped and prevented from waking up until next launch.`);
      selected = null;
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { actionBusy = false; }
  }

  async function enableGameMode(pkg: string | null) {
    if (!pkg || !deviceStore.selected) return;
    actionBusy = true; error = null;
    try {
      await api.setGameMode(deviceStore.selected.serial, pkg, 2); // GAME_MODE_PERFORMANCE
      alert(`Game Mode (Performance) enabled for ${pkg}.`);
      selected = null;
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { actionBusy = false; }
  }
</script>

<header class="page-head">
  <div>
    <h1>Telemetry</h1>
    <p class="muted">
      Live process table. Polled every 3s while this page is open.
      <span class="badge" class:ok={streaming} class:elevated={!streaming}>
        {streaming ? '● Streaming' : '○ Paused'}
      </span>
    </p>
  </div>
  <div class="head-actions">
    {#if lastTickTs}
      <span class="muted age-label">Tick {fmtAge(lastTickTs)}</span>
    {/if}
    {#if streaming}
      <button onclick={stopStream}>Pause</button>
    {:else}
      <button class="primary" onclick={startStream} disabled={!deviceStore.selected}>Start</button>
    {/if}
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected.</p></div>
{:else}
  {#if error}<div class="error">{error}</div>{/if}

  <div class="stat-row">
    <div class="stat">
      <div class="stat-label">Processes</div>
      <div class="stat-value">{snap?.rows.length ?? '—'}</div>
    </div>
    <div class="stat" class:alert={thermal && thermal.raw_value >= 3}>
      <div class="stat-label">Thermal</div>
      <div class="stat-value" data-tier={thermal ? (thermal.raw_value >= 3 ? 'bad' : thermal.raw_value > 0 ? 'warn' : 'ok') : 'ok'}>
        {thermal?.label ?? '—'}
      </div>
    </div>
    <div class="stat" class:alert={snap && snap.zombie_count > 0}>
      <div class="stat-label">Zombies</div>
      <div class="stat-value" data-tier={snap && snap.zombie_count > 0 ? 'bad' : 'ok'}>
        {snap?.zombie_count ?? '—'}
      </div>
    </div>
    <div class="stat" class:alert={snap && snap.hog_candidate_count > 2}>
      <div class="stat-label">Hog candidates</div>
      <div class="stat-value" data-tier={snap && snap.hog_candidate_count > 2 ? 'bad' : 'warn'}>
        {snap?.hog_candidate_count ?? '—'}
      </div>
    </div>
    <div class="stat">
      <div class="stat-label">Total CPU</div>
      <div class="stat-value">
        {snap ? `${snap.total_cpu_percent.toFixed(0)}%` : '—'}
      </div>
    </div>
    <div class="stat">
      <div class="stat-label">Total RSS</div>
      <div class="stat-value">{snap ? fmtRss(snap.total_rss_kb) : '—'}</div>
    </div>
  </div>

  <div class="card filter-bar">
    <input
      type="search"
      placeholder="Filter by package or args…"
      bind:value={filter}
    />
    <label style="margin-right: auto; margin-left: 0.5rem; display: flex; align-items: center; gap: 0.35rem; font-size: var(--font-size-xs); cursor: pointer; color: var(--fg-2);">
      <input type="checkbox" bind:checked={hideSystem} style="width: auto;">
      Hide system threads
    </label>
    <div class="filter-pills">
      <button class:active={stateFilter === 'all'}     onclick={() => stateFilter = 'all'}>All</button>
      <button class:active={stateFilter === 'running'} onclick={() => stateFilter = 'running'}>R</button>
      <button class:active={stateFilter === 'zombie'}  onclick={() => stateFilter = 'zombie'}>Zombies</button>
      <button class:active={stateFilter === 'hog'}     onclick={() => stateFilter = 'hog'}>Hogs</button>
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
              <th>PID</th>
              <th>S</th>
              <th>%CPU</th>
              <th>RSS</th>
              <th>Package / Args</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each filtered as row (row.pid)}
              <tr
                class:zombie={row.is_zombie}
                class:hog={row.is_hog_candidate}
                onclick={() => selected = row}
              >
                <td class="mono pid">{row.pid}</td>
                <td>
                  <span class="state-badge" data-state={row.state}>{stateChar(row.state)}</span>
                </td>
                <td class="mono">
                  <div class="inline-bar" style="--bar-pct: {Math.min(100, (row.cpu_percent / 20) * 100)}%; --bar-color: {row.cpu_percent > 80 ? 'var(--bad)' : (row.cpu_percent > 40 ? 'var(--warn)' : 'var(--accent-dim)')}">
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
                  {#if row.is_zombie}<span class="badge critical">ZOMBIE</span>{/if}
                  {#if row.is_hog_candidate}<span class="badge elevated">HOG</span>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="muted footnote">
        Showing {filtered.length} of {snap.rows.length} processes. Click a row for details.
      </p>
    </div>
  {/if}
{/if}

<SlideOver open={selected !== null} onClose={() => selected = null} title="Process detail" width="440px">
  {#if selected}
    <div class="detail-block">
      <div class="detail-label">PID</div>
      <div class="mono detail-value">{selected.pid}</div>
    </div>
    <div class="detail-block">
      <div class="detail-label">User</div>
      <div class="mono detail-value">{selected.user}</div>
    </div>
    <div class="detail-block">
      <div class="detail-label">State</div>
      <div class="detail-value">
        <span class="state-badge" data-state={selected.state}>{stateChar(selected.state)}</span>
        <span class="muted">{selected.state}</span>
      </div>
    </div>
    <div class="detail-block">
      <div class="detail-label">%CPU (snapshot)</div>
      <div class="mono detail-value">{selected.cpu_percent.toFixed(1)}%</div>
    </div>
    <div class="detail-block">
      <div class="detail-label">RSS</div>
      <div class="mono detail-value">{fmtRss(selected.rss_kb)}</div>
    </div>
    {#if selected.package}
      <div class="detail-block">
        <div class="detail-label">App</div>
        <div class="detail-value">
          <AppName package={selected.package} size="md" />
        </div>
      </div>
    {/if}
    <div class="detail-block">
      <div class="detail-label">Args</div>
      <div class="mono detail-value detail-args">{selected.args}</div>
    </div>

    <div class="detail-actions">
      {#if selected.package}
        <div style="display: flex; flex-direction: column; gap: 0.5rem;">
          <button class="primary" onclick={() => restrict(selected!.package)} disabled={actionBusy}>
            Restrict in Actions →
          </button>
          <div style="display: flex; gap: 0.5rem; margin-top: 0.5rem;">
            <button class="danger" style="flex: 1; font-size: var(--font-size-sm);" onclick={() => hibernateApp(selected!.package)} disabled={actionBusy}>
              {actionBusy ? '…' : 'Hibernate App'}
            </button>
            <button style="flex: 1; font-size: var(--font-size-sm);" onclick={() => enableGameMode(selected!.package)} disabled={actionBusy}>
              {actionBusy ? '…' : 'Game Mode'}
            </button>
          </div>
          <p class="muted small" style="margin: 0.2rem 0 0; line-height: 1.4;">
            <strong>Hibernate:</strong> Force-stops the app and restricts background execution. <br/>
            <strong>Game Mode:</strong> Sets Android's Game Mode downscaling to Performance (API 31+).
          </p>
        </div>
      {:else}
        <p class="muted">No package detected; manual restriction only.</p>
      {/if}
    </div>
  {/if}
</SlideOver>

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  .head-actions { display: flex; align-items: center; gap: 0.85rem; }
  .age-label { font-size: var(--font-size-xs); }

  .stat-row {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 0.75rem;
    margin-bottom: 1rem;
  }
  .stat {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.75rem 1rem;
  }
  .stat.alert { border-color: rgba(239, 68, 68, 0.35); background: rgba(239, 68, 68, 0.04); }
  .stat-label { font-size: 10px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--fg-3); margin-bottom: 4px; }
  .stat-value {
    font-family: var(--font-mono);
    font-size: 22px;
    font-weight: 700;
    color: var(--fg-0);
    letter-spacing: -0.02em;
  }
  .stat-value[data-tier="bad"]  { color: var(--bad); }
  .stat-value[data-tier="warn"] { color: var(--warn); }
  .stat-value[data-tier="ok"]   { color: var(--good); }

  .filter-bar {
    display: flex;
    gap: 1rem;
    align-items: center;
    margin-bottom: 0.85rem;
    padding: 0.6rem 0.85rem;
  }
  .filter-bar input { flex: 1; max-width: 480px; }
  .filter-pills { display: flex; gap: 4px; }
  .filter-pills button {
    padding: 0.35rem 0.85rem;
    font-size: var(--font-size-xs);
    background: transparent;
    border: 1px solid var(--border);
  }
  .filter-pills button.active {
    background: var(--bg-3);
    color: var(--fg-0);
    border-color: var(--accent);
  }

  .table-card { padding: 0.85rem; }
  .proc-table { width: 100%; font-size: 12.5px; }
  .proc-table th { background: var(--bg-1); position: sticky; top: 0; }
  .proc-table tbody tr { cursor: pointer; }
  .proc-table tbody tr.zombie { background: rgba(239, 68, 68, 0.04); }
  .proc-table tbody tr.hog { background: rgba(245, 158, 11, 0.03); }
  .pid { color: var(--fg-2); width: 60px; }
  .args { max-width: 480px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pkg { color: var(--fg-0); }

  .state-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-weight: 700;
    font-size: 11px;
    color: var(--fg-2);
    background: var(--bg-3);
  }
  .state-badge[data-state="zombie"]  { background: var(--bad); color: white; }
  .state-badge[data-state="running"] { background: var(--good); color: #00131C; }
  .state-badge[data-state="uninterruptiblesleep"] { background: var(--warn); color: #00131C; }

  .footnote { font-size: var(--font-size-xs); margin: 0.75rem 0 0 0; }

  /* Slide-over body */
  .detail-block {
    margin-bottom: 1rem;
    padding-bottom: 0.85rem;
    border-bottom: 1px solid var(--border);
  }
  .detail-block:last-child { border-bottom: none; }
  .detail-label {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-3);
    margin-bottom: 0.35rem;
  }
  .detail-value {
    font-size: var(--font-size-base);
    color: var(--fg-0);
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .detail-args {
    font-size: var(--font-size-sm);
    word-break: break-all;
    white-space: normal;
    line-height: 1.5;
  }
  .detail-actions { margin-top: 1.5rem; }

  /* Visual Bars */
  .inline-bar {
    position: relative;
    display: inline-flex;
    align-items: center;
    width: 55px;
    height: 20px;
    background: var(--bg-3);
    border-radius: 4px;
    overflow: hidden;
  }
  .inline-bar-fill {
    position: absolute;
    left: 0; top: 0; bottom: 0;
    background: var(--bar-color);
    width: var(--bar-pct, 0%);
    opacity: 0.8;
  }
  .inline-bar-text {
    position: relative;
    z-index: 1;
    padding-left: 6px;
    font-size: 11px;
    font-weight: 600;
  }
</style>
