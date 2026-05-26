<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import AppName from '$components/AppName.svelte';
  import type { PackageSize, StorageOverview, CompileMode } from '$types';

  type Tab = 'overview' | 'inventory' | 'optimize' | 'compile';
  let tab: Tab = $state('overview');

  let overview: StorageOverview | null = $state(null);
  let inventory: PackageSize[] = $state([]);
  let compileTarget = $state('');
  let compileMode: CompileMode = $state('speed');
  let compileBusy = $state(false);
  let compileMsg: string | null = $state(null);
  let compileErr: string | null = $state(null);

  const COMPILE_MODES: { value: CompileMode; label: string; desc: string }[] = [
    { value: 'speed',         label: 'speed',         desc: 'Full AOT — fastest startup, largest disk impact' },
    { value: 'speed-profile', label: 'speed-profile', desc: 'Compile hot paths from usage profile (default)' },
    { value: 'everything',    label: 'everything',    desc: 'Compile every method, regardless of profile' },
    { value: 'quicken',       label: 'quicken',       desc: 'Lightweight bytecode rewrites' },
    { value: 'verify',        label: 'verify',        desc: 'Only verify, no compilation' },
    { value: 'extract',       label: 'extract',       desc: 'Just unpack APK, skip both verify and compile' }
  ];

  async function runCompile() {
    if (!deviceStore.selected || !compileTarget.trim()) return;
    compileBusy = true; compileMsg = null; compileErr = null;
    try {
      await api.compilePackage(deviceStore.selected.serial, compileTarget.trim(), compileMode);
      compileMsg = `Compiled ${compileTarget.trim()} in mode '${compileMode}'.`;
      cache.invalidatePrefix('storage:');
      cache.invalidatePrefix('inventory:');
    } catch (e) {
      compileErr = (e as DozeForgeError).message;
    } finally { compileBusy = false; }
  }

  async function runResetCompilation() {
    if (!deviceStore.selected || !compileTarget.trim()) return;
    if (!confirm(`Reset AOT cache for ${compileTarget.trim()}?\nThe app will recompile on next launch — first open may be slow.`)) return;
    compileBusy = true; compileMsg = null; compileErr = null;
    try {
      await api.resetCompilation(deviceStore.selected.serial, compileTarget.trim());
      compileMsg = `Compilation cache reset for ${compileTarget.trim()}. Recompiles on next launch.`;
      cache.invalidatePrefix('storage:');
      cache.invalidatePrefix('inventory:');
    } catch (e) {
      compileErr = (e as DozeForgeError).message;
    } finally { compileBusy = false; }
  }
  let loading = $state(false);
  let loadingInventory = $state(false);
  let busy = $state(false);
  let error: string | null = $state(null);
  let success: string | null = $state(null);
  let topAppsArt: Record<string, string> | null = $state(null);
  let analyzingArt = $state(false);

  async function analyzeArtStatus() {
    if (!deviceStore.selected) return;
    analyzingArt = true; error = null;
    try {
      const pkgs = inventory.slice(0, 20).map(p => p.package);
      if (pkgs.length === 0) return;
      topAppsArt = await api.getArtStatusBatch(deviceStore.selected.serial, pkgs);
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { analyzingArt = false; }
  }

  async function clearTempFiles() {
    if (!deviceStore.selected) return;
    busy = true; error = null; success = null;
    try {
      await api.clearTempFiles(deviceStore.selected.serial);
      success = "Temporary files (/data/local/tmp) cleared.";
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  async function smartClean() {
    if (!deviceStore.selected) return;
    if (!confirm("Aggressively trim all caches across the system?")) return;
    busy = true; error = null; success = null;
    try {
      cache.invalidate('storage:' + deviceStore.selected.serial);
      await api.trimSystemCaches(deviceStore.selected.serial, 1000 * 1e9);
      success = "Smart Clean completed.";
      await refreshOverview(true);
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  // Inventory controls
  let invFilter = $state('');
  let selected: Set<string> = $state(new Set());

  // Trim slider — target free GB
  let trimTargetGb = $state(5);
  // Confirmation flags for destructive actions
  let dexoptConfirmed = $state(false);

  async function refreshOverview(force = false) {
    if (!deviceStore.selected) return;
    loading = true; error = null;
    try {
      if (force) cache.invalidate('storage:' + deviceStore.selected.serial);
      overview = await cache.getOrFetch('storage:' + deviceStore.selected.serial, TTL.medium, () => api.storageOverview(deviceStore.selected!.serial));
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  async function forceRescanInventory() {
    if (!deviceStore.selected) return;
    cache.invalidate('inventory:' + deviceStore.selected.serial);
    await refreshInventory();
  }

  async function refreshInventory() {
    if (!deviceStore.selected) return;
    loadingInventory = true; error = null;
    try {
      const inv = await cache.getOrFetch('inventory:' + deviceStore.selected.serial, TTL.inventory, () => api.storageInventory(deviceStore.selected!.serial));
      inventory = inv;
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loadingInventory = false;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') refreshOverview();
  });

  // Auto-load inventory once when user switches to inventory tab
  $effect(() => {
    if (tab === 'inventory' && inventory.length === 0 && !loadingInventory && deviceStore.selected) {
      refreshInventory();
    }
  });

  // Auto-refresh overview when switching to it
  $effect(() => {
    if (tab === 'overview' && deviceStore.selected) {
      refreshOverview(true);
    }
  });

  function toggle(pkg: string) {
    const s = new Set(selected);
    if (s.has(pkg)) s.delete(pkg);
    else s.add(pkg);
    selected = s;
  }
  function clearSelection() { selected = new Set(); }

  async function clearCacheSingle(pkg: string) {
    if (!deviceStore.selected) return;
    busy = true; error = null; success = null;
    try {
      await api.clearAppCache(deviceStore.selected.serial, [pkg]);
      success = `Cache cleared for ${pkg}`;
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  async function openAppInfo(pkg: string) {
    if (!deviceStore.selected) return;
    try {
      await api.openAppSettings(deviceStore.selected.serial, pkg);
    } catch (e) { error = (e as DozeForgeError).message; }
  }

  function fmtBytes(b: number | null): string {
    if (b === null || b === undefined) return '—';
    if (b >= 1e9) return `${(b / 1e9).toFixed(2)} GB`;
    if (b >= 1e6) return `${(b / 1e6).toFixed(1)} MB`;
    if (b >= 1e3) return `${(b / 1e3).toFixed(1)} KB`;
    return `${b} B`;
  }

  function pct(used: number | null, total: number | null): number | null {
    if (used === null || total === null || total === 0) return null;
    return Math.round((used / total) * 100);
  }

  async function clearCacheSelected() {
    if (!deviceStore.selected || selected.size === 0) return;
    const pkgs = Array.from(selected);
    if (!confirm(`Clear CACHE only on ${pkgs.length} app(s)?\n\nThis preserves logins and user data.`)) return;
    busy = true; error = null; success = null;
    try {
      const report = await api.clearAppCache(deviceStore.selected.serial, pkgs);
      const ok = report.outcomes.filter(o => o.success).length;
      success = `Cleared cache on ${ok}/${pkgs.length} app(s).`;
      cache.invalidatePrefix('storage:');
      cache.invalidatePrefix('overview:');
      clearSelection();
      await refreshOverview(true);
      await refreshInventory();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }

  async function trimGlobal() {
    if (!deviceStore.selected) return;
    const targetBytes = Math.floor(trimTargetGb * 1e9);
    if (!confirm(`Ask Android to free up cache until at least ${trimTargetGb} GB are available on /data?`)) return;
    busy = true; error = null; success = null;
    try {
      cache.invalidate('storage:' + deviceStore.selected.serial);
      await api.trimSystemCaches(deviceStore.selected.serial, targetBytes);
      success = `Trim requested with target ${trimTargetGb} GB free.`;
      await refreshOverview(true);
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  async function runDexopt() {
    if (!deviceStore.selected) return;
    if (!dexoptConfirmed) {
      alert('Please tick the warning checkbox first.');
      return;
    }
    if (!confirm('Last chance — start ART recompilation NOW?\n\nThis will run for ~30-45min, peg the CPU, heat the device, and drain battery fast.\n\nOnly proceed if device is charging and you can leave it idle.')) return;
    busy = true; error = null; success = null;
    try {
      await api.runBgDexopt(deviceStore.selected.serial);
      success = 'Dexopt job started. It will run in the background on the device.';
      cache.invalidatePrefix('storage:');
      cache.invalidatePrefix('overview:');
      dexoptConfirmed = false;
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }

  const dataUsedPct = $derived.by<number | null>(() => {
    if (!overview) return null;
    return pct(
      overview.diskstats.data_total_bytes !== null && overview.diskstats.data_free_bytes !== null
        ? overview.diskstats.data_total_bytes - overview.diskstats.data_free_bytes
        : null,
      overview.diskstats.data_total_bytes
    );
  });

  const visibleInventory: PackageSize[] = $derived.by(() => {
    if (!invFilter) return inventory.slice(0, 300);
    const q = invFilter.toLowerCase();
    return inventory.filter(p => p.package.toLowerCase().includes(q)).slice(0, 300);
  });

  function formatPackageName(pkg: string) {
    if (pkg.startsWith('com.google.android.')) return pkg.replace('com.google.android.', 'Google: ');
    if (pkg.startsWith('com.android.')) return pkg.replace('com.android.', 'Android: ');
    return pkg;
  }
  
  function storageColor(pct: number | null): string {
    if (pct === null) return 'linear-gradient(90deg, var(--accent-dim), var(--accent))';
    if (pct > 90) return 'linear-gradient(90deg, rgba(239, 68, 68, 0.4), var(--bad))';
    if (pct > 75) return 'linear-gradient(90deg, rgba(245, 158, 11, 0.4), var(--warn))';
    return 'linear-gradient(90deg, var(--accent-dim), var(--accent))';
  }
</script>

<header class="page-head">
  <div>
    <h1>Storage</h1>
    <p class="muted">
      Disk inventory, cache trimming, and ART recompilation.
      Without root, app data and per-app cache sizes are not readable —
      what you see here is APK code size + system-level cache stats.
    </p>
  </div>
  <button class="primary" onclick={() => refreshOverview(true)} disabled={loading || !deviceStore.selected}>
    {loading ? 'Reading…' : 'Refresh'}
  </button>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected.</p></div>
{:else}
  <div class="seg" role="tablist">
    <button class:active={tab === 'overview'}  onclick={() => tab = 'overview'}  role="tab">Overview</button>
    <button class:active={tab === 'inventory'} onclick={() => tab = 'inventory'} role="tab">
      Inventory <span class="seg-count">{inventory.length || '–'}</span>
    </button>
    <button class:active={tab === 'optimize'}  onclick={() => tab = 'optimize'}  role="tab">Optimize</button>
    <button class:active={tab === 'compile'}   onclick={() => tab = 'compile'}  role="tab">Compile</button>
  </div>

  {#if error}<div class="error">{error}</div>{/if}
  {#if success}<div class="success">{success}</div>{/if}

  <!-- ===== OVERVIEW TAB ===== -->
  {#if tab === 'overview'}
    {#if !overview}
      <div class="card"><Skeleton lines={4} /></div>
    {:else}
      {#if dataUsedPct !== null && dataUsedPct >= 90}
        <div class="card flat banner" style="background: rgba(239, 68, 68, 0.1); border-color: var(--bad); margin-bottom: 1rem;">
          <p style="color: var(--bad);"><strong>Critical:</strong> Storage is almost full (>90%). Risk of app failures. Please free up space immediately.</p>
        </div>
      {:else if dataUsedPct !== null && dataUsedPct >= 85}
        <div class="card flat banner" style="background: rgba(245, 158, 11, 0.1); border-color: var(--warn); margin-bottom: 1rem;">
          <p style="color: var(--warn);"><strong>Warning:</strong> Storage usage is high (>85%). Performance may degrade. Consider running Smart Clean.</p>
        </div>
      {/if}

      <div class="grid two-grid">
        <!-- Data partition -->
        <div class="card big-card">
          <div class="card-label">Data partition</div>
          <div class="big-value mono">
            {fmtBytes(overview.diskstats.data_free_bytes)} <span class="muted unit">free</span>
          </div>
          <div class="bar">
            <div class="bar-fill" style="width: {dataUsedPct ?? 0}%; background: {storageColor(dataUsedPct)};"></div>
          </div>
          <div class="meta-row">
            <span>
              <strong class="mono">{fmtBytes(overview.diskstats.data_total_bytes)}</strong>
              <span class="muted">total</span>
            </span>
            {#if dataUsedPct !== null}
              <span class="muted">{dataUsedPct}% used</span>
            {/if}
          </div>
        </div>

        <!-- System cache -->
        <div class="card big-card">
          <div class="card-label">System cache</div>
          {#if overview.diskstats.cache_total_bytes !== null}
            <div class="big-value mono">
              {fmtBytes(overview.diskstats.cache_total_bytes)}
            </div>
            <p class="muted small">
              Free: <span class="mono">{fmtBytes(overview.diskstats.cache_free_bytes)}</span>
            </p>
          {:else}
            <div class="big-value mono" style="color: var(--fg-3);">N/A</div>
            <p class="muted small">Partition not available on this device.</p>
          {/if}
        </div>
      </div>

      <div class="grid two-grid" style="margin-top: 0.85rem;">
        <div class="card stat-tile">
          <div class="stat-label">Recent write speed</div>
          {#if overview.diskstats.recent_write_speed_kb_s !== null}
            <div class="stat-val mono">
              {(overview.diskstats.recent_write_speed_kb_s / 1024).toFixed(1)}
              <span class="muted unit">MB/s</span>
            </div>
          {:else}
            <div class="stat-val muted">—</div>
          {/if}
          <p class="muted small">Reported by Android diskstats benchmark.</p>
        </div>

        <div class="card stat-tile">
          <div class="stat-label">Encryption</div>
          {#if overview.diskstats.file_based_encryption === true}
            <span class="badge ok">FBE enabled</span>
          {:else if overview.diskstats.file_based_encryption === false}
            <span class="badge moderate">FBE disabled</span>
          {:else}
            <span class="muted">—</span>
          {/if}
          <p class="muted small">File-Based Encryption (Android 7+).</p>
        </div>

        <div class="card stat-tile" style="grid-column: 1 / -1; display: flex; justify-content: space-between; align-items: center;">
          <div>
            <div class="stat-label" style="margin-bottom: 0.2rem;">Temporary Files</div>
            <p class="muted small" style="margin:0;">Clear old ADB sideloads and temporary system files from /data/local/tmp.</p>
          </div>
          <button class="secondary" onclick={clearTempFiles} disabled={busy}>Clean Temp Files</button>
        </div>
      </div>
    {/if}

  <!-- ===== INVENTORY TAB ===== -->
  {:else if tab === 'inventory'}
    <div class="card flat banner">
      <p>
        Select applications from the list to clear their cache safely. Cache and user data are not accessible without root, so sizes shown are base installation files only.
      </p>
    </div>

    <div class="card filter-bar">
      <input
        type="search"
        placeholder="Search apps..."
        bind:value={invFilter}
      />
      <div class="filter-actions">
        <button onclick={forceRescanInventory} disabled={loadingInventory}>
          {loadingInventory ? 'Scanning…' : 'Refresh List'}
        </button>
        <button onclick={clearSelection} disabled={selected.size === 0}>
          Clear ({selected.size})
        </button>
      </div>
    </div>

    {#if loadingInventory && inventory.length === 0}
      <div class="card">
        <Skeleton lines={10} />
      </div>
    {:else if inventory.length === 0}
      <div class="card empty"><p class="muted">No apps found.</p></div>
    {:else}
      <div class="card table-card">
        <div class="scroll-y" style="max-height: 55vh;">
          <table>
            <thead>
              <tr>
                <th style="width: 30px;"></th>
                <th>Application Name</th>
                <th style="text-align: right;">Network Data</th>
                <th style="text-align: right;">App Size</th>
              </tr>
            </thead>
            <tbody>
              {#each visibleInventory as p (p.package)}
                <tr class:selected={selected.has(p.package)} onclick={() => toggle(p.package)}>
                  <td>
                    <input
                      type="checkbox"
                      checked={selected.has(p.package)}
                      onclick={(e) => e.stopPropagation()}
                      onchange={() => toggle(p.package)}
                      aria-label="select"
                    />
                  </td>
                  <td>
                    <AppName package={p.package} />
                  </td>
                  <td class="mono" style="text-align: right; vertical-align: middle;">
                    <span class="muted" style="font-size: 11px;">0 B</span>
                  </td>
                  <td style="text-align: right; vertical-align: middle;">
                    <span class="badge" style="font-family: var(--font-mono); font-size: 11px;">{fmtBytes(p.apk_bytes)}</span>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        {#if inventory.length > visibleInventory.length}
          <p class="muted footnote">
            Showing {visibleInventory.length} of {inventory.length} installed applications.
          </p>
        {/if}
      </div>

      <div class="action-bar" class:active={selected.size > 0}>
        <span class="muted">{selected.size > 0 ? `${selected.size} selected` : 'Smart Clean'}</span>
        <div class="action-buttons">
          {#if selected.size === 0}
            <button class="primary" onclick={smartClean} disabled={busy}>
              {busy ? 'Cleaning…' : 'Smart Clean (All)'}
            </button>
          {:else}
            <button class="primary" onclick={clearCacheSelected} disabled={busy}>
              {busy ? 'Processing…' : `Clear Cache (${selected.size})`}
            </button>
          {/if}
        </div>
      </div>
    {/if}

  <!-- ===== OPTIMIZE TAB ===== -->
  {:else if tab === 'optimize'}
    <!-- Trim caches -->
    <div class="card">
      <h3>Trim system caches</h3>
      <p class="muted">
        Asks Android to free up cache from all apps until at least
        <strong>N&nbsp;GB</strong> are available on <code>/data</code>. The system
        decides which apps to trim — non-destructive, no user data is touched.
      </p>
      <div class="trim-controls">
        <label class="trim-target">
          Target free space:
          <input
            type="range"
            min="1"
            max="50"
            step="1"
            bind:value={trimTargetGb}
          />
          <strong class="mono trim-val">{trimTargetGb} GB</strong>
        </label>
        <button class="primary" onclick={trimGlobal} disabled={busy}>
          {busy ? 'Trimming…' : 'Trim now'}
        </button>
      </div>
    </div>

    <!-- ART optimizer -->
    <div class="card destructive" style="margin-top: 1rem;">
      <h3>
        <svg class="warn-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
        Recompile ART (background dexopt)
      </h3>
      <p>
        Triggers <code>cmd package bg-dexopt-job</code>. Forces Android to
        recompile installed apps from DEX bytecode to AOT machine code.
        Improves app launch latency after OTA updates or major Play Store updates.
      </p>
      <ul class="caveats">
        <li><strong>Duration:</strong> 30-45 min of continuous high CPU use.</li>
        <li><strong>Thermal:</strong> Device will get warm — possible throttle.</li>
        <li><strong>Battery:</strong> Drains 15-25% of charge during the job.</li>
        <li><strong>Recommendation:</strong> Run only while device is charging and on a cool surface. <em>Do not</em> run it monthly — Android already does it when idle.</li>
      </ul>
      <label class="confirm-checkbox">
        <input type="checkbox" bind:checked={dexoptConfirmed} />
        I understand and the device is plugged in.
      </label>
      <button class="danger" onclick={runDexopt} disabled={busy || !dexoptConfirmed}>
        {busy ? 'Started…' : 'Run dexopt job'}
      </button>
    </div>

  <!-- ===== COMPILE TAB (Block H 2.2 + 2.3) ===== -->
  {:else if tab === 'compile'}
    <div class="card flat banner">
      <p>
        <strong>AOT compilation per app.</strong>
        <code>speed</code> = full ahead-of-time compilation for fastest launch
        (large disk cost). <code>--reset</code> wipes the AOT cache so the app
        recompiles on next launch.
      </p>
    </div>

    <div class="card" style="margin-bottom: 1rem;">
      <div style="display: flex; justify-content: space-between; align-items: center;">
        <div>
          <h3 style="margin: 0 0 0.5rem;">Intelligent ART Analyzer</h3>
          <p class="muted small" style="margin: 0;">Scan your Top 20 largest apps to see if they need compilation.</p>
        </div>
        <button class="secondary" onclick={analyzeArtStatus} disabled={analyzingArt || inventory.length === 0}>
          {analyzingArt ? 'Scanning…' : 'Analyze Top Apps'}
        </button>
      </div>

      {#if topAppsArt}
        <div class="scroll-y" style="max-height: 200px; margin-top: 1rem;">
          <table style="width: 100%; font-size: 12px;">
            <tbody>
              {#each Object.entries(topAppsArt) as [pkg, status]}
                <tr onclick={() => compileTarget = pkg} style="cursor: pointer;" title="Click to target this app">
                  <td style="padding: 0.5rem;">
                    <div style="font-weight: 500; color: var(--fg-0);">{formatPackageName(pkg)}</div>
                    <div class="muted small mono">{pkg}</div>
                  </td>
                  <td style="padding: 0.5rem; text-align: right; vertical-align: middle;">
                    <span class="badge {status.includes('speed') ? 'ok' : (status.includes('verify') ? 'bad' : 'moderate')}">{status}</span>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>

    <div class="card">
      <h3>Target package</h3>
      <p class="muted small">
        Use exact package name (e.g. <code class="mono">com.spotify.music</code>).
        Find it in the Bloatware tab if you don't know it.
      </p>
      <input
        type="text"
        placeholder="com.example.app"
        bind:value={compileTarget}
        spellcheck="false"
        autocomplete="off"
        style="margin-top: 0.5rem;"
      />

      <h3 style="margin-top: 1.25rem;">Compile mode</h3>
      <select bind:value={compileMode} style="max-width: 380px;">
        {#each COMPILE_MODES as m (m.value)}
          <option value={m.value}>{m.label} — {m.desc}</option>
        {/each}
      </select>

      {#if compileMsg}<div class="success" style="margin-top: 1rem;">{compileMsg}</div>{/if}
      {#if compileErr}<div class="error" style="margin-top: 1rem;">{compileErr}</div>{/if}

      <div class="compile-actions">
        <button
          class="primary"
          onclick={runCompile}
          disabled={compileBusy || !compileTarget.trim()}
        >
          {compileBusy ? 'Compiling… (up to 5 min)' : `Compile (${compileMode})`}
        </button>
        <span class="muted small">↑ this can take 30s to 5 min depending on app size</span>
      </div>
    </div>

    <div class="card" style="margin-top: 1rem;">
      <h3>Reset compilation cache</h3>
      <p class="muted">
        Wipes the AOT compilation for the target package above. The app will
        recompile from scratch when next opened — first launch may be slow
        but subsequent ones return to normal. Use this when an app starts
        stuttering after a system update.
      </p>
      <button
        class="warn"
        onclick={runResetCompilation}
        disabled={compileBusy || !compileTarget.trim()}
        style="margin-top: 0.5rem;"
      >
        {compileBusy ? '…' : `↻ Reset compilation for ${compileTarget.trim() || '(pick app above)'}`}
      </button>
    </div>
  {/if}
{/if}

<style>
  .page-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    margin-bottom: 1.5rem;
    gap: 1rem;
  }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 580px; }

  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: 99px;
    margin-bottom: 1rem;
  }
  .seg button {
    padding: 0.45rem 1rem;
    border-radius: 99px;
    background: transparent;
    border: none;
    color: var(--fg-2);
    font-size: var(--font-size-sm);
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }
  .seg button.active {
    background: var(--bg-4);
    color: var(--fg-0);
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }
  .seg-count {
    background: var(--bg-3);
    color: var(--fg-2);
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 99px;
    font-family: var(--font-mono);
  }
  .seg button.active .seg-count { background: var(--accent); color: #00131C; }

  .success {
    padding: 0.65rem 1rem;
    background: rgba(16, 185, 129, 0.1);
    border-left: 3px solid var(--good);
    border-radius: var(--radius);
    color: var(--good);
    margin-bottom: 1rem;
    font-size: var(--font-size-sm);
  }

  .two-grid { grid-template-columns: 1fr 1fr; }
  @media (max-width: 920px) { .two-grid { grid-template-columns: 1fr; } }

  .big-card { padding: 1.25rem 1.5rem; display: flex; flex-direction: column; gap: 0.6rem; }
  .card-label {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-3);
    font-weight: 600;
  }
  .big-value {
    font-size: 32px;
    font-weight: 700;
    color: var(--fg-0);
    letter-spacing: -0.025em;
    line-height: 1.1;
  }
  .big-value .unit { font-size: 14px; font-weight: 400; }
  .bar {
    width: 100%;
    height: 6px;
    background: var(--bg-4);
    border-radius: 3px;
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-dim), var(--accent));
    transition: width 600ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  .meta-row {
    display: flex;
    justify-content: space-between;
    font-size: var(--font-size-sm);
    color: var(--fg-2);
  }

  .stat-tile { padding: 1rem 1.15rem; }
  .stat-label {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-3);
    font-weight: 600;
    margin-bottom: 0.5rem;
  }
  .stat-val {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--fg-0);
    line-height: 1.1;
  }
  .stat-val .unit { font-size: 12px; font-weight: 400; }
  .small { font-size: var(--font-size-xs); margin-top: 0.4rem; }

  /* Inventory */
  .banner {
    padding: 0.65rem 1rem;
    border-color: rgba(56, 189, 248, 0.2);
    background: rgba(56, 189, 248, 0.04);
    margin-bottom: 1rem;
  }
  .banner p { margin: 0; font-size: var(--font-size-sm); color: var(--fg-1); }

  .filter-bar {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    margin-bottom: 0.85rem;
    padding: 0.6rem 0.85rem;
  }
  .filter-bar input[type="search"] { flex: 1; min-width: 240px; max-width: 360px; }
  .filter-actions { margin-left: auto; display: flex; gap: 0.4rem; }
  .filter-actions button { font-size: var(--font-size-xs); padding: 0.35rem 0.75rem; }

  .table-card { padding: 0.5rem; }
  table { width: 100%; font-size: 12.5px; }
  th { background: var(--bg-1); position: sticky; top: 0; z-index: 1; padding: 0.55rem 0.75rem; }
  tbody tr { cursor: pointer; transition: background var(--t-fast); }
  tbody tr:hover { background: var(--bg-3); }
  tbody tr.selected { background: rgba(56, 189, 248, 0.05); }
  tbody td { padding: 0.5rem 0.75rem; }

  .action-bar {
    margin-top: 1rem;
    padding: 0.85rem 1.15rem;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    display: flex;
    justify-content: space-between;
    align-items: center;
    transition: border-color var(--t-base);
  }
  .action-bar.active { border-color: var(--accent); }
  .action-buttons { display: flex; gap: 0.55rem; }

  /* Optimize */
  .trim-controls {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 1rem;
    flex-wrap: wrap;
  }
  .trim-target {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    flex: 1;
    font-size: var(--font-size-sm);
    color: var(--fg-2);
  }
  .trim-target input[type="range"] {
    flex: 1;
    min-width: 180px;
    accent-color: var(--accent);
  }
  .trim-val {
    min-width: 64px;
    text-align: right;
    color: var(--fg-0);
    font-size: 18px;
    letter-spacing: -0.02em;
  }

  .destructive {
    border-color: rgba(239, 68, 68, 0.3);
    background: linear-gradient(180deg, rgba(239, 68, 68, 0.04) 0%, var(--bg-2) 100%);
  }
  .destructive h3 {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    color: var(--bad);
  }
  .warn-icon {
    font-size: 18px;
    line-height: 1;
  }
  .caveats {
    margin: 0.85rem 0;
    padding-left: 1.25rem;
    font-size: var(--font-size-sm);
    color: var(--fg-1);
  }
  .caveats li { margin-bottom: 0.35rem; }
  .confirm-checkbox {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    margin: 1rem 0;
    padding: 0.65rem 0.85rem;
    background: var(--bg-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    font-size: var(--font-size-sm);
  }
  .confirm-checkbox input { width: auto; cursor: pointer; }

  .footnote { font-size: var(--font-size-xs); margin: 0.5rem 0 0; }
  .compile-actions { display: flex; align-items: center; gap: 1rem; flex-wrap: wrap; margin-top: 1rem; }
  button.warn {
    background: rgba(245, 158, 11, 0.1);
    color: var(--warn);
    border: 1px solid rgba(245, 158, 11, 0.4);
  }
  button.warn:hover:not(:disabled) {
    background: rgba(245, 158, 11, 0.2);
    border-color: var(--warn);
  }
</style>
