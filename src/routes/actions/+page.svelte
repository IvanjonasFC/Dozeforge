<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { snapshotStore } from '$stores/snapshots.svelte';
  import { cache } from '$stores/cache.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import type {
    OptimizationAction,
    OptimizationReport,
    Profile,
    ProfilePreview,
    StandbyBucket
  } from '$types';
  import { formatTimestamp } from '$utils/format';

  type Mode = 'profiles' | 'manual' | 'snapshots';
  let mode = $state<Mode>('profiles');

  // ===== Profiles =====
  const profiles: Array<{ id: Profile; title: string; subtitle: string; tier: string; icon: string }> = [
    { id: 'conservative', title: 'Conservative', subtitle: 'Only known offenders.',     tier: 'moderate', icon: '🛡️' },
    { id: 'balanced',     title: 'Balanced',     subtitle: 'Recommended default.',      tier: 'ok',       icon: '⚖️' },
    { id: 'aggressive',   title: 'Aggressive',   subtitle: 'Restrict every user app.',  tier: 'elevated', icon: '⚡' },
    { id: 'nuclear',      title: 'Nuclear',      subtitle: 'Maximum savings.',          tier: 'critical', icon: '☢️' }
  ];

  let selectedProfile = $state<Profile | null>(null);
  let preview = $state<ProfilePreview | null>(null);
  let report = $state<OptimizationReport | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function loadPreview(p: Profile) {
    if (!deviceStore.selected) return;
    selectedProfile = p;
    preview = null;
    report = null;
    busy = true;
    error = null;
    try {
      preview = await api.previewProfile(deviceStore.selected.serial, p);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      busy = false;
    }
  }
  async function applyProfile() {
    if (!deviceStore.selected || !selectedProfile || !preview) return;
    const n = preview.summary.total_actions;
    if (!confirm(`Apply ${selectedProfile.toUpperCase()} (${n} actions)? A snapshot is taken first.`)) return;
    busy = true; error = null;
    try {
      report = await api.applyProfile(deviceStore.selected.serial, selectedProfile);
      cache.invalidateAll();
      await snapshotStore.refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }
  function clearProfile() {
    selectedProfile = null; preview = null; report = null;
  }

  // ===== Manual =====
  let pkg = $state('');
  let bucket = $state<StandbyBucket>('restricted');
  let revokeBg = $state(true);
  let revokeWl = $state(true);
  let killNow = $state(false);
  let manualBusy = $state(false);
  let manualReport = $state<OptimizationReport | null>(null);
  let manualError = $state<string | null>(null);

  onMount(() => {
    const qpkg = page.url.searchParams.get('pkg');
    if (qpkg) {
      pkg = qpkg;
      mode = 'manual';
    }
    snapshotStore.refresh();
  });

  const manualActions = $derived.by<OptimizationAction[]>(() => {
    if (!pkg) return [];
    const list: OptimizationAction[] = [
      { kind: 'set_standby_bucket', package: pkg, bucket }
    ];
    if (revokeBg) {
      list.push({ kind: 'set_app_op', package: pkg, op: 'RUN_IN_BACKGROUND', mode: 'ignore' });
      list.push({ kind: 'set_app_op', package: pkg, op: 'RUN_ANY_IN_BACKGROUND', mode: 'ignore' });
    }
    if (revokeWl) list.push({ kind: 'set_app_op', package: pkg, op: 'WAKE_LOCK', mode: 'ignore' });
    if (killNow)  list.push({ kind: 'kill_package', package: pkg });
    return list;
  });

  async function applyManual() {
    if (!deviceStore.selected || manualActions.length === 0) return;
    manualBusy = true; manualError = null; manualReport = null;
    try {
      manualReport = await api.applyOptimization(deviceStore.selected.serial, manualActions);
      cache.invalidateAll();
      await snapshotStore.refresh();
    } catch (e) {
      manualError = (e as DozeForgeError).message;
    } finally { manualBusy = false; }
  }

  async function extractApk() {
    if (!deviceStore.selected || !pkg) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const savePath = await save({ filters: [{ name: 'APK File', extensions: ['apk'] }], defaultPath: `${pkg}.apk` });
      if (!savePath) return;
      manualBusy = true; manualError = null; manualReport = null;
      const res = await api.extractApk(deviceStore.selected.serial, pkg, savePath);
      manualError = res; // Using error field to show success text for now to avoid creating a new variable
    } catch (e) {
      manualError = (e as DozeForgeError).message;
    } finally {
      manualBusy = false;
    }
  }

  // ===== Snapshots =====
  let rollbackBusy = $state<string | null>(null);
  let rollbackError = $state<string | null>(null);

  async function rollback(id: string) {
    if (!deviceStore.selected) return;
    if (!confirm('Roll back to this snapshot? Device state will be restored.')) return;
    rollbackBusy = id; rollbackError = null;
    try {
      await snapshotStore.rollback(deviceStore.selected.serial, id);
      cache.invalidateAll();
      alert('Rollback completed.');
    } catch (e) {
      rollbackError = (e as DozeForgeError).message;
    } finally { rollbackBusy = null; }
  }
</script>

<header class="page-head">
  <div>
    <h1>Actions</h1>
    <p class="muted">Apply optimizations. Every change is reversible via snapshot.</p>
  </div>
  <div class="seg">
    <button class:active={mode === 'profiles'} onclick={() => mode = 'profiles'}>Quick profiles</button>
    <button class:active={mode === 'manual'}   onclick={() => mode = 'manual'}>Single app</button>
    <button class:active={mode === 'snapshots'} onclick={() => mode = 'snapshots'}>Snapshots</button>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected.</p></div>
{:else if mode === 'profiles'}
  {#if !selectedProfile}
    <div class="profile-grid">
      {#each profiles as p (p.id)}
        <button class="profile-card" data-tier={p.tier} onclick={() => loadPreview(p.id)}>
          <div class="profile-tier" data-tier={p.tier}></div>
          <div class="profile-content">
            <div class="profile-icon">{p.icon}</div>
            <div>
              <h3>{p.title}</h3>
              <p>{p.subtitle}</p>
            </div>
          </div>
        </button>
      {/each}
    </div>
    <div class="card flat" style="margin-top: 1.5rem;">
      <h3>Always excluded</h3>
      <p class="muted">
        Communication apps (WhatsApp, Telegram, Signal, banking), input methods,
        accessibility services, and any system-critical package are never touched
        by these profiles.
      </p>
    </div>
  {:else}
    <div class="card">
      <div class="row" style="justify-content: space-between; align-items: flex-start;">
        <div>
          <h2>{profiles.find(p => p.id === selectedProfile)?.title} — Preview</h2>
          <p class="muted">Review before committing. Nothing applied yet.</p>
        </div>
        <button onclick={clearProfile}>← Back</button>
      </div>
      {#if busy && !preview}
        <Skeleton lines={5} />
      {:else if preview}
        <div class="summary-grid">
          <div class="metric">
            <div class="metric-value">{preview.summary.total_actions}</div>
            <div class="metric-label">Total actions</div>
          </div>
          <div class="metric">
            <div class="metric-value" style="color: var(--bad)">{preview.summary.bloatware_disabled}</div>
            <div class="metric-label">Bloatware disabled</div>
          </div>
          <div class="metric">
            <div class="metric-value" style="color: var(--accent)">{preview.summary.apps_restricted}</div>
            <div class="metric-label">Apps restricted</div>
          </div>
          <div class="metric">
            <div class="metric-value" style="color: var(--warn)">{preview.summary.wakelocks_revoked}</div>
            <div class="metric-label">Wakelocks revoked</div>
          </div>
          <div class="metric">
            <div class="metric-value">{preview.summary.doze_whitelist_cleaned}</div>
            <div class="metric-label">Doze cleaned</div>
          </div>
          <div class="metric">
            <div class="metric-value" style="color: var(--fg-2)">{preview.summary.packages_excluded}</div>
            <div class="metric-label">Auto-excluded</div>
          </div>
        </div>
        <div style="margin-top: 1.5rem;">
          <button class="primary" onclick={applyProfile} disabled={busy || preview.summary.total_actions === 0}>
            {busy ? 'Applying…' : `Apply ${preview.summary.total_actions} action(s)`}
          </button>
        </div>

        {#if report}
          <div class="report-block" style="margin-top: 1.5rem;">
            <h3>Result</h3>
            <p>
              Snapshot: <code class="mono">{report.snapshot_id.slice(0, 12)}…</code><br/>
              <strong>{report.outcomes.filter(o => o.success).length}</strong> / {report.outcomes.length} ok
            </p>
          </div>
        {/if}

        <details style="margin-top: 1rem;">
          <summary>Action breakdown</summary>
          <ul class="mono action-list">
            {#each preview.actions as a, i (i)}
              <li>{a.kind}{'package' in a ? ' → ' + a.package : ''}</li>
            {/each}
          </ul>
        </details>
        <details style="margin-top: 0.5rem;">
          <summary>Auto-excluded ({preview.excluded_packages.length})</summary>
          <ul class="excluded-list">
            {#each preview.excluded_packages as [p, reason] (p)}
              <li><code class="mono">{p}</code> <span class="muted small">— {reason}</span></li>
            {/each}
          </ul>
        </details>
      {/if}
      {#if error}<div class="error" style="margin-top: 1rem;">{error}</div>{/if}
    </div>
  {/if}

{:else if mode === 'manual'}
  <div class="card">
    <h3>Single-app optimization</h3>
    <p class="muted">Surgical restriction of one package. Take a snapshot first.</p>
    <div class="form-grid">
      <label>
        Package
        <input type="text" bind:value={pkg} placeholder="com.example.app" spellcheck="false" autocomplete="off"/>
      </label>
      <label>
        Standby bucket
        <select bind:value={bucket}>
          <option value="active">Active</option>
          <option value="working_set">Working Set</option>
          <option value="frequent">Frequent</option>
          <option value="rare">Rare</option>
          <option value="restricted">Restricted</option>
        </select>
      </label>
    </div>
    <div class="checklist">
      <label><input type="checkbox" bind:checked={revokeBg}/> Revoke RUN_IN_BACKGROUND</label>
      <label><input type="checkbox" bind:checked={revokeWl}/> Revoke WAKE_LOCK</label>
      <label><input type="checkbox" bind:checked={killNow}/> Force-stop now (am kill)</label>
    </div>
    <div style="margin-bottom: 1rem;">
      <button class="btn outline" onclick={extractApk} disabled={!pkg || manualBusy}>Extract APK to PC</button>
    </div>
    {#if manualActions.length > 0}
      <div class="action-preview">
        <h4>{manualActions.length} step(s):</h4>
        <ul class="mono">
          {#each manualActions as a, i (i)}
            <li>{a.kind}{'package' in a ? ' → ' + a.package : ''}</li>
          {/each}
        </ul>
      </div>
    {/if}
    <button class="primary" onclick={applyManual} disabled={manualBusy || manualActions.length === 0}>
      {manualBusy ? 'Applying…' : `Apply (${manualActions.length})`}
    </button>
    {#if manualError}<div class="error" style="margin-top: 1rem;">{manualError}</div>{/if}
    {#if manualReport}
      <div class="report-block" style="margin-top: 1rem;">
        <h4>Done</h4>
        <p>Snapshot <code class="mono">{manualReport.snapshot_id.slice(0, 12)}…</code> · {manualReport.outcomes.filter(o => o.success).length}/{manualReport.outcomes.length} ok</p>
      </div>
    {/if}
  </div>

{:else}
  <!-- Snapshots tab -->
  <div class="card">
    <div class="row" style="justify-content: space-between; margin-bottom: 1rem;">
      <h3 style="margin: 0;">{snapshotStore.list.length} snapshot(s)</h3>
      <button onclick={() => snapshotStore.refresh()} disabled={snapshotStore.loading}>
        {snapshotStore.loading ? '…' : 'Refresh'}
      </button>
    </div>
    {#if snapshotStore.list.length === 0}
      <p class="muted">No snapshots yet.</p>
    {:else}
      <table>
        <thead><tr><th>ID</th><th>When</th><th>Device</th><th>SDK</th><th>Pkgs</th><th></th></tr></thead>
        <tbody>
          {#each snapshotStore.list as s (s.id)}
            <tr>
              <td class="mono">{s.id.slice(0, 12)}…</td>
              <td>{formatTimestamp(s.created_at)}</td>
              <td class="mono small">{s.device_serial}</td>
              <td>{s.sdk_int}</td>
              <td>{s.packages}</td>
              <td>
                <button class="danger" onclick={() => rollback(s.id)} disabled={rollbackBusy !== null}>
                  {rollbackBusy === s.id ? '…' : 'Rollback'}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#if rollbackError}<div class="error" style="margin-top: 1rem;">{rollbackError}</div>{/if}
  </div>
{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; }

  .seg {
    display: flex;
    gap: 2px;
    padding: 3px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: 99px;
  }
  .seg button {
    padding: 0.4rem 0.95rem;
    border-radius: 99px;
    background: transparent;
    border: none;
    color: var(--fg-2);
    font-size: var(--font-size-sm);
  }
  .seg button.active {
    background: var(--bg-4);
    color: var(--fg-0);
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }

  /* Profiles */
  .profile-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 0.85rem; }
  .profile-card {
    background: linear-gradient(135deg, var(--bg-1) 0%, var(--bg-2) 100%);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 1.5rem;
    text-align: left;
    cursor: pointer;
    position: relative;
    overflow: hidden;
    transition: all var(--t-fast);
    box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  }
  .profile-card:hover { 
    border-color: var(--border-strong); 
    transform: translateY(-2px); 
    box-shadow: 0 8px 24px rgba(0,0,0,0.25);
  }
  .profile-card[data-tier="ok"]:hover { box-shadow: 0 8px 24px color-mix(in srgb, var(--good) 20%, transparent); border-color: color-mix(in srgb, var(--good) 50%, transparent); }
  .profile-card[data-tier="moderate"]:hover { box-shadow: 0 8px 24px color-mix(in srgb, var(--accent) 20%, transparent); border-color: color-mix(in srgb, var(--accent) 50%, transparent); }
  .profile-card[data-tier="elevated"]:hover { box-shadow: 0 8px 24px color-mix(in srgb, var(--warn) 20%, transparent); border-color: color-mix(in srgb, var(--warn) 50%, transparent); }
  .profile-card[data-tier="critical"]:hover { box-shadow: 0 8px 24px color-mix(in srgb, var(--bad) 20%, transparent); border-color: color-mix(in srgb, var(--bad) 50%, transparent); }

  .profile-content { display: flex; gap: 1rem; align-items: center; }
  .profile-icon { font-size: 28px; line-height: 1; filter: grayscale(0.2); }
  .profile-tier {
    position: absolute;
    top: 0; left: 0;
    width: 3px;
    height: 100%;
  }
  .profile-tier[data-tier="moderate"] { background: var(--accent); }
  .profile-tier[data-tier="ok"]       { background: var(--good); }
  .profile-tier[data-tier="elevated"] { background: var(--warn); }
  .profile-tier[data-tier="critical"] { background: var(--bad); }
  .profile-card h3 { margin: 0 0 0.25rem 0; color: var(--fg-0); font-size: var(--font-size-lg); }
  .profile-card p { margin: 0; color: var(--fg-2); font-size: var(--font-size-sm); line-height: 1.4; }

  /* Summary grid */
  .summary-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
    margin-top: 1.25rem;
  }
  .metric {
    background: var(--bg-3);
    border-radius: var(--radius);
    padding: 0.85rem 1rem;
    text-align: center;
  }
  .metric-value {
    font-family: var(--font-mono);
    font-size: 24px;
    font-weight: 700;
    color: var(--fg-0);
    letter-spacing: -0.02em;
  }
  .metric-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-3);
    margin-top: 4px;
  }

  .report-block {
    background: rgba(16, 185, 129, 0.08);
    border-left: 3px solid var(--good);
    padding: 0.85rem 1rem;
    border-radius: var(--radius);
  }
  .report-block h3, .report-block h4 { margin: 0 0 0.4rem 0; }
  .report-block p { margin: 0; }

  .action-list { max-height: 240px; overflow-y: auto; margin: 0.5rem 0 0; padding-left: 1.25rem; font-size: var(--font-size-xs); }
  .excluded-list { list-style: none; padding: 0; margin: 0.5rem 0 0; max-height: 240px; overflow-y: auto; }
  .excluded-list li { padding: 0.25rem 0; border-bottom: 1px solid var(--border); }

  details { background: var(--bg-3); padding: 0.65rem 0.85rem; border-radius: var(--radius); }
  details summary { cursor: pointer; font-weight: 600; color: var(--fg-1); }

  /* Manual */
  .form-grid { display: grid; grid-template-columns: 2fr 1fr; gap: 1rem; margin: 1rem 0; }
  .form-grid label { display: flex; flex-direction: column; gap: 4px; font-size: var(--font-size-sm); color: var(--fg-2); }
  .checklist { display: flex; flex-direction: column; gap: 0.45rem; margin-bottom: 1rem; }
  .checklist label { display: flex; gap: 0.55rem; align-items: center; color: var(--fg-1); }
  .checklist input { width: auto; }
  .action-preview {
    background: var(--bg-3);
    padding: 0.85rem 1rem;
    border-radius: var(--radius);
    margin-bottom: 1rem;
  }
  .action-preview h4 { margin: 0 0 0.4rem 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--fg-3); }
  .action-preview ul { margin: 0; padding-left: 1.25rem; font-size: var(--font-size-xs); color: var(--fg-1); }

  .small { font-size: var(--font-size-xs); }
</style>
