<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { cache } from '$stores/cache.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import AppName from '$components/AppName.svelte';
  import RiskBadge from '$components/RiskBadge.svelte';
  import type {
    BloatwareReport,
    BloatwareRecommendation,
    BloatPresetDto,
    BloatPreset,
    Recommendation,
  } from '$types';

  let recommendations = $state<BloatwareRecommendation[]>([]);
  let presets = $state<BloatPresetDto[]>([]);
  let selected = $state<Set<string>>(new Set());
  let loading = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let report = $state<BloatwareReport | null>(null);
  let filter = $state('');
  let tierFilter = $state<'all' | 'moderate' | 'elevated' | 'critical'>('all');
  let recFilter = $state<'all' | Recommendation>('all');

  // ---- Preset modal state ----
  let activePresetPreview = $state<{ preset: BloatPresetDto; pkgs: string[] } | null>(null);
  let presetBusy = $state(false);

  onMount(async () => {
    presets = await api.listBloatPresets().catch(() => []);
    if (deviceStore.selected) refresh();
  });

  async function refresh() {
    if (!deviceStore.selected) return;
    loading = true;
    error = null;
    try {
      recommendations = await api.bloatwareRecommendations(deviceStore.selected.serial);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  const filtered = $derived.by(() => {
    const serial = deviceStore.selected?.serial ?? null;
    const needle = filter.toLowerCase();
    return recommendations.filter((r) => {
      if (needle) {
        const label = labelStore.labelFor(serial, r.package).toLowerCase();
        if (!r.package.toLowerCase().includes(needle) && !label.includes(needle)) return false;
      }
      if (tierFilter !== 'all' && r.tier !== tierFilter) return false;
      if (recFilter !== 'all' && r.recommendation !== recFilter) return false;
      return true;
    });
  });

  const counts = $derived.by(() => {
    const base = { safe: 0, bloat: 0, careful: 0, critical: 0 };
    for (const r of recommendations) {
      if (r.recommendation === 'safe_to_disable') base.safe++;
      else if (r.recommendation === 'preinstalled_bloat') base.bloat++;
      else if (r.recommendation === 'system_use_with_care') base.careful++;
      else base.critical++;
    }
    return base;
  });

  function toggle(pkg: string) {
    const next = new Set(selected);
    if (next.has(pkg)) next.delete(pkg);
    else next.add(pkg);
    selected = next;
  }

  function selectAllVisible() {
    const next = new Set(selected);
    for (const r of filtered) {
      if (r.recommendation !== 'do_not_touch') next.add(r.package);
    }
    selected = next;
  }

  function clearSelection() { selected = new Set(); }

  async function applyDisable() {
    if (!deviceStore.selected || selected.size === 0) return;
    const safeOnly = [...selected].filter((p) => {
      const r = recommendations.find((x) => x.package === p);
      return r && r.recommendation !== 'do_not_touch';
    });
    if (safeOnly.length === 0) return;
    if (!confirm(`Disable ${safeOnly.length} package${safeOnly.length === 1 ? '' : 's'} via "pm disable-user --user 0"? This is fully reversible from this same page.`)) return;
    busy = true; error = null; report = null;
    try {
      report = await api.disableBloatware(deviceStore.selected.serial, safeOnly);
      cache.invalidatePrefix('packages:');
      cache.invalidatePrefix('inventory:');
      cache.invalidatePrefix('overview:');
      cache.invalidatePrefix('miscat:');
      selected = new Set();
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }

  async function applyEnable() {
    if (!deviceStore.selected || selected.size === 0) return;
    busy = true; error = null; report = null;
    try {
      report = await api.enableBloatware(deviceStore.selected.serial, [...selected]);
      cache.invalidatePrefix('packages:');
      cache.invalidatePrefix('inventory:');
      cache.invalidatePrefix('overview:');
      cache.invalidatePrefix('miscat:');
      selected = new Set();
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }

  // ---- Preset flow ----
  async function previewPreset(preset: BloatPresetDto) {
    if (!deviceStore.selected) return;
    presetBusy = true; error = null;
    try {
      const pkgs = await api.previewBloatPreset(deviceStore.selected.serial, preset.id);
      activePresetPreview = { preset, pkgs };
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { presetBusy = false; }
  }

  function applyPresetSelection() {
    if (!activePresetPreview) return;
    const next = new Set(selected);
    for (const p of activePresetPreview.pkgs) next.add(p);
    selected = next;
    activePresetPreview = null;
  }

  function recBadgeMeta(r: Recommendation): { cls: string; label: string; icon: string } {
    switch (r) {
      case 'safe_to_disable':      return { cls: 'rec-safe',     label: 'Safe to disable',     icon: '✓' };
      case 'preinstalled_bloat':   return { cls: 'rec-bloat',    label: 'Preinstalled bloat',  icon: '◐' };
      case 'system_use_with_care': return { cls: 'rec-careful',  label: 'Use with care',       icon: '!' };
      case 'do_not_touch':         return { cls: 'rec-critical', label: 'Do not touch',        icon: '✕' };
    }
  }
</script>

<header class="page-head">
  <div>
    <h1>Bloatware</h1>
    <p class="muted">
      <code>pm disable-user --user 0</code> hides and disables packages without removing them from disk. Fully reversible via <code>pm enable</code>. Critical packages are blocked by the risk classifier regardless of selection.
    </p>
  </div>
  <div>
    <button onclick={refresh} disabled={loading}>{loading ? 'Loading…' : 'Reload'}</button>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty">Select a device first.</div>
{:else}
  <!-- ============ Presets ============ -->
  <section class="card preset-card">
    <div class="preset-head">
      <div>
        <h3>One-click presets</h3>
        <p class="muted small">
          Selects every package on this device matching the preset's categories. Critical system packages are always excluded automatically. Click a preset to preview before applying.
        </p>
      </div>
    </div>
    <div class="preset-grid">
      {#each presets as p (p.id)}
        <button class="preset-btn" onclick={() => previewPreset(p)} disabled={presetBusy}>
          <div class="preset-label">{p.label}</div>
          <div class="preset-desc">{p.description}</div>
        </button>
      {/each}
    </div>
  </section>

  <!-- ============ Stat strip ============ -->
  <div class="stat-strip">
    <button class="stat-tile" class:active={recFilter === 'safe_to_disable'}
            onclick={() => recFilter = recFilter === 'safe_to_disable' ? 'all' : 'safe_to_disable'}>
      <div class="stat-num good-num">{counts.safe}</div>
      <div class="stat-label">Safe to disable</div>
    </button>
    <button class="stat-tile" class:active={recFilter === 'preinstalled_bloat'}
            onclick={() => recFilter = recFilter === 'preinstalled_bloat' ? 'all' : 'preinstalled_bloat'}>
      <div class="stat-num bloat-num">{counts.bloat}</div>
      <div class="stat-label">Preinstalled bloat</div>
    </button>
    <button class="stat-tile" class:active={recFilter === 'system_use_with_care'}
            onclick={() => recFilter = recFilter === 'system_use_with_care' ? 'all' : 'system_use_with_care'}>
      <div class="stat-num warn-num">{counts.careful}</div>
      <div class="stat-label">Use with care</div>
    </button>
    <button class="stat-tile" class:active={recFilter === 'do_not_touch'}
            onclick={() => recFilter = recFilter === 'do_not_touch' ? 'all' : 'do_not_touch'}>
      <div class="stat-num bad-num">{counts.critical}</div>
      <div class="stat-label">Do not touch</div>
    </button>
  </div>

  <!-- ============ Filter + actions ============ -->
  <div class="card">
    <div class="row" style="justify-content: space-between; flex-wrap: wrap; gap: 0.75rem;">
      <div class="row" style="flex: 1; gap: 0.5rem; min-width: 0;">
        <input type="search" placeholder="Filter by app name or package…" bind:value={filter} />
        <select bind:value={tierFilter} style="width: 11rem;">
          <option value="all">All risk tiers</option>
          <option value="moderate">Moderate</option>
          <option value="elevated">Elevated</option>
          <option value="critical">Critical</option>
        </select>
      </div>
      <div class="row">
        <button onclick={selectAllVisible} disabled={filtered.length === 0}>
          Select visible
        </button>
        <button onclick={clearSelection} disabled={selected.size === 0}>
          Clear ({selected.size})
        </button>
        <button class="danger" onclick={applyDisable} disabled={busy || selected.size === 0}>
          Disable ({selected.size})
        </button>
        <button class="primary" onclick={applyEnable} disabled={busy || selected.size === 0}>
          Enable ({selected.size})
        </button>
      </div>
    </div>

    {#if error}
      <div class="error" style="margin: 1rem 0;">{error}</div>
    {/if}

    {#if recFilter !== 'all'}
      <p class="muted small" style="margin: 0.75rem 0 0;">
        Filtering by recommendation: <strong>{recBadgeMeta(recFilter).label}</strong> ·
        <button class="link-btn" onclick={() => recFilter = 'all'}>clear filter</button>
      </p>
    {/if}

    <div class="scroll-y" style="margin-top: 1rem;">
      <table>
        <thead>
          <tr>
            <th style="width: 28px;"></th>
            <th>App</th>
            <th>UID</th>
            <th>Risk</th>
            <th>Recommendation</th>
            <th>Why</th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as r (r.package)}
            {@const meta = recBadgeMeta(r.recommendation)}
            <tr class:row-rec-safe={r.recommendation === 'safe_to_disable'}
                class:row-rec-bloat={r.recommendation === 'preinstalled_bloat'}
                class:row-rec-careful={r.recommendation === 'system_use_with_care'}
                class:row-rec-critical={r.recommendation === 'do_not_touch'}>
              <td>
                <input
                  type="checkbox"
                  checked={selected.has(r.package)}
                  onchange={() => toggle(r.package)}
                  disabled={r.recommendation === 'do_not_touch'}
                />
              </td>
              <td><AppName package={r.package} size="sm" /></td>
              <td class="mono">{counts ? '' : ''}{recommendations.find((x) => x.package === r.package)?.tier === 'critical' ? '—' : ''}</td>
              <td><RiskBadge tier={r.tier} /></td>
              <td>
                <span class="rec-badge {meta.cls}" title={meta.label}>
                  <span class="rec-icon">{meta.icon}</span> {meta.label}
                </span>
              </td>
              <td class="muted small">{r.notes}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="muted" style="margin-top: 0.75rem;">
      Showing {filtered.length} of {recommendations.length} packages.
    </p>

    {#if report}
      <div style="margin-top: 1rem;">
        <h3>Result</h3>
        <p>Disabled: <strong>{report.disabled.length}</strong>, Failed: <strong>{report.failed.length}</strong></p>
        {#if report.failed.length > 0}
          <table>
            <thead><tr><th>App</th><th>Reason</th></tr></thead>
            <tbody>
              {#each report.failed as [pkg, err], i (i)}
                <tr><td><AppName package={pkg} size="sm" hidePackage /></td><td class="mono small">{err}</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<!-- ===== Preset preview modal ===== -->
{#if activePresetPreview}
  <div class="modal-backdrop" onclick={() => activePresetPreview = null} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
      <h3>{activePresetPreview.preset.label}</h3>
      <p class="muted small">{activePresetPreview.preset.description}</p>

      {#if activePresetPreview.pkgs.length === 0}
        <p class="muted" style="margin-top: 1rem;">
          No packages on this device match this preset. You're already clean for this category.
        </p>
        <div class="modal-actions">
          <button onclick={() => activePresetPreview = null}>Close</button>
        </div>
      {:else}
        <p style="margin-top: 1rem;">
          <strong>{activePresetPreview.pkgs.length}</strong> package{activePresetPreview.pkgs.length === 1 ? '' : 's'} on this device match{activePresetPreview.pkgs.length === 1 ? 'es' : ''} this preset:
        </p>
        <div class="modal-scroll">
          <ul class="modal-list">
            {#each activePresetPreview.pkgs as pkg (pkg)}
              <li><AppName package={pkg} size="sm" /></li>
            {/each}
          </ul>
        </div>
        <p class="muted small" style="margin-top: 0.5rem;">
          Clicking "Add to selection" only stages them. Nothing is disabled until you click the red Disable button below.
        </p>
        <div class="modal-actions">
          <button onclick={() => activePresetPreview = null}>Cancel</button>
          <button class="primary" onclick={applyPresetSelection}>
            Add {activePresetPreview.pkgs.length} to selection
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.25rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }

  /* Presets */
  .preset-card { margin-bottom: 1rem; }
  .preset-head h3 { margin: 0 0 0.25rem; }
  .preset-head p { margin: 0 0 1rem; }
  .preset-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 0.75rem;
  }
  .preset-btn {
    text-align: left;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.85rem 1rem;
    cursor: pointer;
    transition: border-color var(--t-fast), background var(--t-fast);
    color: inherit;
    font-family: inherit;
    white-space: normal;
    word-break: break-word;
    height: 100%;
  }
  .preset-btn:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--bg-3);
  }
  .preset-label { font-weight: 600; color: var(--fg-0); margin-bottom: 0.35rem; }
  .preset-desc { color: var(--fg-2); font-size: var(--font-size-xs); line-height: 1.45; }

  /* Stat strip */
  .stat-strip {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.6rem;
    margin-bottom: 1rem;
  }
  @media (max-width: 700px) { .stat-strip { grid-template-columns: 1fr 1fr; } }
  .stat-tile {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.85rem 1rem;
    text-align: left;
    cursor: pointer;
    transition: border-color var(--t-fast), background var(--t-fast);
    color: inherit;
    font-family: inherit;
  }
  .stat-tile:hover { border-color: var(--border-strong); }
  .stat-tile.active { border-color: var(--accent); background: var(--bg-3); }
  .stat-num { font-family: var(--font-mono); font-size: 22px; font-weight: 700; line-height: 1; }
  .stat-label { font-size: var(--font-size-xs); color: var(--fg-2); margin-top: 4px; }
  .good-num { color: var(--good); }
  .bloat-num { color: var(--accent); }
  .warn-num { color: var(--warn); }
  .bad-num { color: var(--bad); }

  /* Recommendation badge */
  .rec-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0.55rem;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-weight: 600;
    white-space: nowrap;
  }
  .rec-icon {
    display: inline-flex;
    width: 16px; height: 16px;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    font-size: 10px;
  }
  .rec-safe     { background: rgba(34, 197, 94, 0.12); color: var(--good); border: 1px solid rgba(34, 197, 94, 0.25); }
  .rec-safe .rec-icon { background: rgba(34, 197, 94, 0.25); }
  .rec-bloat    { background: rgba(56, 189, 248, 0.10); color: var(--accent); border: 1px solid rgba(56, 189, 248, 0.25); }
  .rec-bloat .rec-icon { background: rgba(56, 189, 248, 0.20); }
  .rec-careful  { background: rgba(245, 158, 11, 0.10); color: var(--warn); border: 1px solid rgba(245, 158, 11, 0.25); }
  .rec-careful .rec-icon { background: rgba(245, 158, 11, 0.22); }
  .rec-critical { background: rgba(239, 68, 68, 0.10); color: var(--bad); border: 1px solid rgba(239, 68, 68, 0.25); }
  .rec-critical .rec-icon { background: rgba(239, 68, 68, 0.22); }

  /* Row tinting */
  tr.row-rec-critical { opacity: 0.55; }

  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    padding: 0;
    cursor: pointer;
    font-family: inherit;
    text-decoration: underline;
  }
  .link-btn:hover { color: var(--fg-0); }

  /* Modal */
  .modal-backdrop {
    position: fixed; inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex; align-items: center; justify-content: center;
    z-index: 100;
    backdrop-filter: blur(4px);
  }
  .modal {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 1.5rem;
    max-width: 560px;
    width: 90%;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
  }
  .modal h3 { margin: 0 0 0.5rem; }
  .modal-scroll {
    overflow-y: auto;
    margin-top: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.5rem;
    flex: 1;
    min-height: 100px;
  }
  .modal-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.4rem; }
  .modal-list li { padding: 0.35rem 0.5rem; background: var(--bg-2); border-radius: var(--radius-sm); }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 1rem;
  }
</style>
