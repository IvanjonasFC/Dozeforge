<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { snapshotStore } from '$stores/snapshots.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import CapabilitiesBanner from '$components/CapabilitiesBanner.svelte';
  import AppName from '$components/AppName.svelte';
  import type {
    OverviewSnapshot,
    MiscategorizedApp,
    PrivacyState,
    PrivacyAppEntry,
    StorageOverview
  } from '$types';

  let snap: OverviewSnapshot | null = $state(null);
  let miscat: MiscategorizedApp[] = $state([]);
  let privacy: PrivacyState | null = $state(null);
  let storage: StorageOverview | null = $state(null);
  let loadedAt: Date | null = $state(null);
  let loading = $state(false);
  let error: string | null = $state(null);

  async function refresh() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    const serial = deviceStore.selected.serial;
    loading = true;
    error = null;
    try {
      const [ov, ma] = await Promise.all([
        cache.getOrFetch('overview:' + serial, TTL.short, () => api.overviewSnapshot(serial)),
        cache.getOrFetch('miscat:' + serial, TTL.medium, () => api.miscategorizedApps(serial)),
        snapshotStore.refresh()
      ]);
      snap = ov; miscat = ma;
      loadedAt = new Date();
      
      cache.getOrFetch('privacy:' + serial, TTL.medium, () => api.getPrivacyState(serial)).then(pr => privacy = pr);
      cache.getOrFetch('storage:' + serial, TTL.medium, () => api.storageOverview(serial)).then(st => storage = st);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  function forceRefresh() {
    if (!deviceStore.selected) return;
    const serial = deviceStore.selected.serial;
    cache.invalidate('overview:' + serial);
    cache.invalidate('miscat:' + serial);
    cache.invalidate('privacy:' + serial);
    cache.invalidate('storage:' + serial);
    refresh();
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') refresh();
  });

  $effect(() => {
    if (deviceStore.selected?.state === 'device' && !loadedAt) refresh();
  });

  function fmtAge(d: Date | null): string {
    if (!d) return '';
    const s = Math.floor((Date.now() - d.getTime()) / 1000);
    if (s < 60) return `${s}s ago`;
    if (s < 3600) return `${Math.floor(s / 60)}m ago`;
    return `${Math.floor(s / 3600)}h ago`;
  }

  function fmtBytes(n: number | null | undefined): string {
    if (n == null) return '—';
    const u = ['B','KB','MB','GB','TB'];
    let i = 0; let v = n;
    while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
    return `${v.toFixed(v < 10 ? 1 : 0)} ${u[i]}`;
  }

  // ----- Health score computation -----
  // Weighted blend of: battery health (40%), background noise (25%),
  // disk pressure (15%), system policy state (20%).
  const healthScore = $derived.by<number | null>(() => {
    if (!snap) return null;
    const battery = snap.battery.health_percent ?? snap.battery.level_percent ?? 100;
    const miscatPenalty = Math.min(100, miscat.length * 12);
    const bgScore = 100 - miscatPenalty;
    const diskUsed = dataUsedPercent ?? 0;
    const diskScore = Math.max(0, 100 - diskUsed);
    // Policy bonus: each privacy action the user has applied is a tiny boost
    const policyBoost = Math.min(20, (privacy?.scan.apps.length ?? 0));
    const raw = battery * 0.40 + bgScore * 0.25 + diskScore * 0.15 + (80 + policyBoost) * 0.20;
    return Math.round(Math.max(0, Math.min(100, raw)));
  });

  const scoreBand = $derived.by(() => {
    if (healthScore === null) return 'neutral';
    if (healthScore >= 80) return 'good';
    if (healthScore >= 60) return 'warn';
    return 'bad';
  });

  const scoreSummary = $derived.by(() => {
    if (healthScore === null) return 'Connect a device to see your score.';
    if (healthScore >= 85) return 'Your device is well-tuned. Minor cleanup possible.';
    if (healthScore >= 70) return 'Good shape — a few areas could use attention.';
    if (healthScore >= 50) return 'Several optimizations available. See cards below.';
    return 'Significant background load and disk pressure detected.';
  });

  // Score ring math (circumference of r=80 circle = 502)
  const RING_CIRC = 502;
  const ringOffset = $derived.by(() => {
    if (healthScore === null) return RING_CIRC;
    return RING_CIRC - (healthScore / 100) * RING_CIRC;
  });

  // Inner ring (battery level): circumference of r=60 circle = 377
  const INNER_CIRC = 377;
  const innerOffset = $derived.by(() => {
    const lvl = snap?.battery.level_percent;
    if (lvl === null || lvl === undefined) return INNER_CIRC;
    return INNER_CIRC - (Math.min(100, Math.max(0, lvl)) / 100) * INNER_CIRC;
  });
  const innerRingColor = $derived.by(() => {
    const lvl = snap?.battery.level_percent;
    if (lvl === null || lvl === undefined) return 'var(--fg-3)';
    if (lvl >= 60) return 'var(--good)';
    if (lvl >= 25) return 'var(--warn)';
    return 'var(--bad)';
  });

  // ----- Action cards (3) -----
  const ringColorVar = $derived(
    scoreBand === 'good' ? 'var(--good)' :
    scoreBand === 'warn' ? 'var(--warn)' :
    scoreBand === 'bad'  ? 'var(--bad)'  : 'var(--fg-3)'
  );

  const cacheBytes = $derived.by<number>(() => {
    if (!storage) return 0;
    return (storage.diskstats.cache_total_bytes ?? 0) - (storage.diskstats.cache_free_bytes ?? 0);
  });

  const dataUsedPercent = $derived.by<number | null>(() => {
    const ds = storage?.diskstats;
    if (!ds || !ds.data_total_bytes || ds.data_free_bytes == null) return null;
    return ((ds.data_total_bytes - ds.data_free_bytes) / ds.data_total_bytes) * 100;
  });

  const firewalledCount = $derived.by(() => {
    if (!privacy) return 0;
    return privacy.scan.apps.filter((a) => a.firewall_active).length;
  });

  function tempColor(c: number | null): string {
    if (c === null) return 'var(--fg-3)';
    if (c < 25) return 'var(--accent)';
    if (c < 35) return 'var(--good)';
    if (c < 42) return 'var(--warn)';
    return 'var(--bad)';
  }
</script>

<header class="page-head">
  <div>
    <h1>Overview</h1>
    <p class="muted">Your device at a glance — start here.</p>
  </div>
  <div class="head-actions">
    {#if loadedAt}<span class="muted age-label">Updated {fmtAge(loadedAt)}</span>{/if}
    <button class="primary" onclick={forceRefresh} disabled={loading || !deviceStore.selected}>
      {#if loading}
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" class="spin">
          <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2v3.5h-3.5"/>
        </svg>
        Refreshing…
      {:else}Refresh{/if}
    </button>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected. Plug in a phone via USB with debugging enabled.</p></div>
{:else if loading && !snap}
  <div class="card"><Skeleton lines={8} /></div>
{:else if error}
  <div class="error">{error}</div>
{:else if snap}
  <!-- ===== HERO: Health Score ===== -->
  <section class="hero-card" data-band={scoreBand}>
    <div class="hero-ring">
      <svg viewBox="0 0 200 200" class="big-ring">
        <!-- Outer track + arc (health score) -->
        <circle cx="100" cy="100" r="80" stroke="var(--bg-4)" stroke-width="10" fill="none"/>
        <circle cx="100" cy="100" r="80" stroke={ringColorVar} stroke-width="10" stroke-linecap="round"
                fill="none" transform="rotate(-90 100 100)"
                stroke-dasharray={RING_CIRC} stroke-dashoffset={ringOffset} class="ring-arc"/>
        <!-- Inner track + arc (battery level) -->
        <circle cx="100" cy="100" r="60" stroke="var(--bg-4)" stroke-width="6" fill="none"/>
        <circle cx="100" cy="100" r="60" stroke={innerRingColor} stroke-width="6" stroke-linecap="round"
                fill="none" transform="rotate(-90 100 100)"
                stroke-dasharray={INNER_CIRC} stroke-dashoffset={innerOffset} class="ring-arc"/>
        {#if healthScore !== null}
          <text x="100" y="94" text-anchor="middle" class="ring-num">{healthScore}</text>
          <text x="100" y="116" text-anchor="middle" class="ring-unit">/ 100</text>
        {:else}
          <text x="100" y="108" text-anchor="middle" class="ring-num ring-muted">?</text>
        {/if}
      </svg>
    </div>
    <div class="hero-text">
      <div class="hero-eyebrow">Device health score</div>
      <h2 class="hero-title">
        {#if healthScore !== null}
          Your phone is at <span class="hero-num" style="color: {ringColorVar}">{healthScore}%</span> of optimal
        {:else}
          Calculating…
        {/if}
      </h2>
      <p class="hero-sub">{scoreSummary}</p>
      <div class="hero-meta">
        <span><strong>{miscat.length}</strong> apps mis-categorized</span>
        <span>·</span>
        <span><strong>{firewalledCount}</strong> apps firewalled</span>
        <span>·</span>
        <span><strong>{snap.battery.health_percent?.toFixed(0) ?? '—'}%</strong> battery health</span>
      </div>
    </div>
  </section>

  <!-- ===== 3 ACTION CARDS ===== -->
  <h3 class="section-title">Recommended actions</h3>
  <div class="action-grid">
    <button class="action-card" onclick={() => goto('/sleep/')}>
      <div class="action-icon" style="color: var(--bad); background: color-mix(in srgb, var(--bad) 15%, transparent);">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>
      </div>
      <div class="action-body">
        <div class="action-title">
          {#if miscat.length === 0}
            No background drain detected
          {:else}
            <span class="counter">{miscat.length}</span>
            app{miscat.length === 1 ? '' : 's'} draining battery in background
          {/if}
        </div>
        <p class="action-desc">
          {#if miscat.length > 0}
            These apps run in active bucket while you barely use them. Demote them to restricted.
          {:else}
            Your wake-locks and standby buckets look healthy.
          {/if}
        </p>
        <span class="action-cta">Go to Sleep →</span>
      </div>
    </button>

    <button class="action-card" onclick={() => goto('/privacy/')}>
      <div class="action-icon" style="color: var(--warn); background: color-mix(in srgb, var(--warn) 15%, transparent);">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
      </div>
      <div class="action-body">
        <div class="action-title">
          {#if privacy?.dns.mode === 'hostname'}
            DNS hardened ✓
          {:else if firewalledCount > 0}
            <span class="counter">{firewalledCount}</span> apps firewalled, DNS not hardened
          {:else}
            Privacy not yet configured
          {/if}
        </div>
        <p class="action-desc">
          {#if privacy?.dns.mode === 'hostname'}
            Using {privacy.dns.hostname}. Block individual apps from background or clipboard.
          {:else}
            Set a privacy DNS (AdGuard, Cloudflare) and firewall background-hungry apps.
          {/if}
        </p>
        <span class="action-cta">Go to Privacy →</span>
      </div>
    </button>

    <button class="action-card" onclick={() => goto('/storage/')}>
      <div class="action-icon" style="color: var(--accent); background: color-mix(in srgb, var(--accent) 15%, transparent);">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="12" x2="2" y2="12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><line x1="6" y1="16" x2="6.01" y2="16"/><line x1="10" y1="16" x2="10.01" y2="16"/></svg>
      </div>
      <div class="action-body">
        <div class="action-title">
          {#if cacheBytes > 1_000_000_000}
            <span class="counter">{fmtBytes(cacheBytes)}</span> of system caches
          {:else if dataUsedPercent != null && dataUsedPercent > 85}
            Disk {dataUsedPercent!.toFixed(0)}% full
          {:else}
            Storage well-managed
          {/if}
        </div>
        <p class="action-desc">
          {#if cacheBytes > 1_000_000_000}
            Trim system caches and clear per-app caches to recover space.
          {:else}
            Inventory your installed apps by APK size and recompile slow ones.
          {/if}
        </p>
        <span class="action-cta">Go to Storage →</span>
      </div>
    </button>
  </div>

  <CapabilitiesBanner />

  <!-- ===== Device snapshot (existing data) ===== -->
  <h3 class="section-title">Right now</h3>
  <div class="grid three-col">
    <div class="card">
      <div class="card-eyebrow">Battery</div>
      <div class="card-main mono">{snap.battery.level_percent?.toFixed(0) ?? '—'}%</div>
      <div class="card-detail">
        <span class="muted">{snap.battery.status ?? 'unknown'}</span>
        {#if snap.battery.temperature_c !== null}
          · <span style="color: {tempColor(snap.battery.temperature_c)}">{snap.battery.temperature_c.toFixed(1)}°C</span>
        {/if}
      </div>
    </div>
    <div class="card">
      <div class="card-eyebrow">Storage</div>
      <div class="card-main mono">
        {dataUsedPercent != null ? dataUsedPercent.toFixed(0) : '—'}%
      </div>
      <div class="card-detail muted">
        {#if storage}
          {fmtBytes((storage?.diskstats?.data_total_bytes ?? 0) - (storage?.diskstats?.data_free_bytes ?? 0))}
          of {fmtBytes(storage?.diskstats?.data_total_bytes)}
        {:else}
          Loading...
        {/if}
      </div>
    </div>
    <div class="card">
      <div class="card-eyebrow">Memory (RAM)</div>
      <div class="card-main mono">
        {#if snap.ram_used_mb && snap.ram_total_mb}
          {((snap.ram_used_mb / snap.ram_total_mb) * 100).toFixed(0)}%
        {:else}
          —%
        {/if}
      </div>
      <div class="card-detail muted">
        {#if snap.ram_used_mb && snap.ram_total_mb}
          {(snap.ram_used_mb / 1024).toFixed(1)} GB of {(snap.ram_total_mb / 1024).toFixed(1)} GB
        {:else}
          Unknown
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; }
  .head-actions { display: flex; align-items: center; gap: 0.85rem; }
  .age-label { font-size: var(--font-size-xs); }
  .spin { animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* === Hero card === */
  .hero-card {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2rem;
    align-items: center;
    padding: 2rem;
    background: linear-gradient(135deg, var(--bg-1) 0%, var(--bg-2) 100%);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    margin-bottom: 1.5rem;
    position: relative;
    overflow: hidden;
  }
  .hero-card::before {
    content: '';
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 15% 50%, currentColor 0%, transparent 50%);
    opacity: 0.08;
    pointer-events: none;
  }
  .hero-card[data-band="good"]  { color: var(--good); }
  .hero-card[data-band="warn"]  { color: var(--warn); }
  .hero-card[data-band="bad"]   { color: var(--bad); }
  .hero-card[data-band="neutral"] { color: var(--fg-3); }

  .hero-ring { display: flex; align-items: center; justify-content: center; }
  .big-ring { width: 220px; height: 220px; flex-shrink: 0; }
  .ring-arc { transition: stroke-dashoffset 1s cubic-bezier(0.16, 1, 0.3, 1); }
  .ring-num {
    font-size: 44px; font-weight: 700; fill: var(--fg-0);
    font-family: var(--font-mono); letter-spacing: -0.04em;
  }
  .ring-num.ring-muted { fill: var(--fg-3); }
  .ring-unit { font-size: 11px; fill: var(--fg-3); font-family: var(--font-mono); }

  .hero-text { z-index: 1; }
  .hero-eyebrow {
    font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.12em;
    color: var(--fg-3); font-weight: 600; margin-bottom: 0.4rem;
  }
  .hero-title { font-size: 26px; line-height: 1.2; margin: 0 0 0.6rem 0; letter-spacing: -0.025em; }
  .hero-num { font-family: var(--font-mono); font-weight: 700; font-size: 30px; }
  .hero-sub { color: var(--fg-1); margin: 0 0 1rem 0; font-size: var(--font-size-sm); }
  .hero-meta { display: flex; gap: 0.55rem; font-size: var(--font-size-xs); color: var(--fg-2); flex-wrap: wrap; }
  .hero-meta strong { color: var(--fg-0); font-family: var(--font-mono); margin-right: 1px; }

  /* === Section titles === */
  .section-title {
    font-size: 11px; text-transform: uppercase; letter-spacing: 0.12em;
    color: var(--fg-3); font-weight: 700; margin: 1.75rem 0 0.85rem 0;
  }

  /* === Action cards === */
  .action-grid {
    display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.85rem;
    margin-bottom: 1.5rem;
  }
  @media (max-width: 920px) { .action-grid { grid-template-columns: 1fr; } }

  .action-card {
    display: flex; gap: 1rem; padding: 1.15rem 1.25rem;
    background: var(--bg-1); border: 1px solid var(--border);
    border-radius: var(--radius-lg); text-align: left;
    cursor: pointer; transition: all var(--t-fast);
    width: 100%; min-width: 0;
    color: inherit; font-family: inherit;
    box-shadow: 0 4px 12px rgba(0,0,0,0.2);
  }
  .action-card:hover {
    border-color: var(--border-strong);
    background: var(--bg-2);
    transform: translateY(-2px);
    box-shadow: 0 6px 16px rgba(0,0,0,0.3);
  }
  .action-icon {
    flex-shrink: 0;
    width: 44px; height: 44px;
    display: flex; align-items: center; justify-content: center;
    border-radius: 10px;
  }
  .action-icon svg { display: block; }
  .action-body { flex: 1; min-width: 0; overflow: hidden; }
  .action-title {
    font-size: var(--font-size-base); font-weight: 600;
    color: var(--fg-0); line-height: 1.3; margin-bottom: 0.35rem;
    overflow-wrap: anywhere;
  }
  .action-title .counter {
    font-family: var(--font-mono); color: var(--accent); font-weight: 700;
  }
  .action-desc {
    font-size: var(--font-size-xs); color: var(--fg-2);
    margin: 0 0 0.65rem 0; line-height: 1.5;
    overflow-wrap: anywhere;
  }
  .action-cta {
    font-size: 11px; color: var(--accent); font-weight: 600;
    letter-spacing: 0.02em;
  }
  .action-card:hover .action-cta { color: var(--fg-0); }

  /* === Right-now grid (existing) === */
  .grid.three-col {
    display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.85rem;
  }
  @media (max-width: 600px) { .grid.three-col { grid-template-columns: 1fr; } }
  .card-eyebrow {
    font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.08em;
    color: var(--fg-3); font-weight: 600; margin-bottom: 0.5rem;
  }
  .card-main {
    font-size: 28px; font-weight: 700; letter-spacing: -0.02em;
    color: var(--fg-0); line-height: 1.1;
  }
  .card-detail { font-size: var(--font-size-sm); color: var(--fg-2); margin-top: 0.35rem; }

  /* === Compact drain table === */
  .compact-drain { padding: 0.85rem 1.15rem; }
  .drain-table { width: 100%; font-size: var(--font-size-sm); }
  .drain-table th {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-3);
    font-weight: 600;
    padding-bottom: 0.65rem;
    border-bottom: 1px solid var(--border);
  }
  .drain-table td { padding: 0.55rem 0.5rem; }
  .drain-table .unit { font-size: 10.5px; font-weight: 400; }
  .drain-table tr.row-bad td:first-child {
    border-left: 2px solid var(--bad);
    padding-left: calc(0.5rem - 2px);
  }
  .drain-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 0.75rem;
    padding-top: 0.65rem;
    border-top: 1px solid var(--border);
    font-size: var(--font-size-xs);
  }
  .drain-footer strong { color: var(--fg-0); }
  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    padding: 0;
    cursor: pointer;
    font-family: inherit;
    font-size: var(--font-size-xs);
    font-weight: 600;
  }
  .link-btn:hover { color: var(--fg-0); }
</style>