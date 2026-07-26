<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { snapshotStore } from '$stores/snapshots.svelte';
  import { cache } from '$stores/cache.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import { formatTimestamp } from '$utils/format';
  import type { Profile, ProfilePreview, OptimizationReport, RollbackReport } from '$types';

  type PresetMeta = { id: Profile; name: string; tone: string; desc: string };
  const PRESETS: PresetMeta[] = [
    { id: 'conservative', name: 'Conservative', tone: 'good',   desc: 'Safest. Only restricts obvious background offenders. Recommended first run.' },
    { id: 'balanced',     name: 'Balanced',     tone: 'accent', desc: 'Recommended. Solid battery savings with low risk of breaking apps.' },
    { id: 'aggressive',   name: 'Aggressive',   tone: 'warn',   desc: 'Restricts all user apps. Some notifications may be delayed.' },
    { id: 'nuclear',      name: 'Nuclear',      tone: 'bad',    desc: 'Maximum savings. May break background sync for many apps. Use with care.' }
  ];

  let selected = $state<Profile | null>(null);
  let preview = $state<ProfilePreview | null>(null);
  let previewing = $state(false);
  let applying = $state(false);
  let report = $state<OptimizationReport | null>(null);
  let error = $state<string | null>(null);
  let showExcluded = $state(false);

  // Rollback UI state
  let rollbackReport = $state<RollbackReport | null>(null);
  let rollbackBusy = $state<string | null>(null);

  const serial = $derived(deviceStore.selected?.serial ?? null);
  const ready = $derived(deviceStore.selected?.state === 'device');

  onMount(() => { snapshotStore.refresh(); });

  async function doPreview(profile: Profile) {
    if (!serial) return;
    selected = profile;
    preview = null; report = null; error = null; showExcluded = false;
    previewing = true;
    try {
      preview = await api.previewProfile(serial, profile);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      previewing = false;
    }
  }

  async function apply() {
    if (!serial || !selected) return;
    const count = preview?.summary.total_actions ?? 0;
    if (!confirm(i18n.t('Apply the {{name}} profile? {{count}} actions will run. A snapshot is saved first so you can undo.', { name: selected, count }))) return;
    applying = true; error = null; report = null;
    try {
      report = await api.applyProfile(serial, selected);
      cache.invalidateAll();
      await snapshotStore.refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      applying = false;
    }
  }

  async function rollback(id: string) {
    if (!serial) return;
    if (!confirm(i18n.t('Restore this snapshot? It reverts the changes captured at that point.'))) return;
    rollbackBusy = id; rollbackReport = null; error = null;
    try {
      rollbackReport = await snapshotStore.rollback(serial, id);
      cache.invalidateAll();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      rollbackBusy = null;
    }
  }

  const okCount = $derived(report ? report.outcomes.filter(o => o.success).length : 0);
  const failCount = $derived(report ? report.outcomes.length - okCount : 0);
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Profiles & Snapshots')}</h1>
    <p class="muted">{i18n.t('One-click optimization with an automatic safety net. Every profile saves a snapshot first, so you can always undo.')}</p>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else if !ready}
  <div class="card empty"><p class="muted">{i18n.t('Device is offline or unauthorized.')}</p></div>
{:else}
  {#if error}<div class="error">{error}</div>{/if}

  <h3 class="sec">{i18n.t('One-click optimize')}</h3>
  <div class="presets">
    {#each PRESETS as p (p.id)}
      <button class="preset" class:active={selected === p.id} data-tone={p.tone} onclick={() => doPreview(p.id)}>
        <div class="preset-top">
          <span class="preset-dot"></span>
          <span class="preset-name">{i18n.t(p.name)}</span>
        </div>
        <p class="preset-desc">{i18n.t(p.desc)}</p>
      </button>
    {/each}
  </div>

  {#if previewing}
    <div class="card"><p class="muted">{i18n.t('Calculating impact…')}</p></div>
  {:else if preview}
    <div class="card preview-card">
      <div class="preview-head">
        <h3>{i18n.t('Impact of the {{name}} profile', { name: preview.profile })}</h3>
        <button class="primary" onclick={apply} disabled={applying}>
          {applying ? i18n.t('Applying…') : i18n.t('Apply {{count}} actions', { count: preview.summary.total_actions })}
        </button>
      </div>
      <div class="metrics">
        <div class="metric"><div class="mval">{preview.summary.apps_restricted}</div><div class="mlab">{i18n.t('Apps restricted')}</div></div>
        <div class="metric"><div class="mval">{preview.summary.bloatware_disabled}</div><div class="mlab">{i18n.t('Bloatware disabled')}</div></div>
        <div class="metric"><div class="mval">{preview.summary.wakelocks_revoked}</div><div class="mlab">{i18n.t('Wakelocks revoked')}</div></div>
        <div class="metric"><div class="mval">{preview.summary.doze_whitelist_cleaned}</div><div class="mlab">{i18n.t('Doze whitelist cleaned')}</div></div>
        <div class="metric"><div class="mval">{preview.summary.packages_excluded}</div><div class="mlab">{i18n.t('Protected (excluded)')}</div></div>
      </div>
      {#if preview.excluded_packages.length > 0}
        <button class="link-btn" onclick={() => showExcluded = !showExcluded}>
          {showExcluded ? i18n.t('Hide protected apps') : i18n.t('Show {{count}} protected apps', { count: preview.excluded_packages.length })}
        </button>
        {#if showExcluded}
          <ul class="excluded">
            {#each preview.excluded_packages as [pkg, reason] (pkg)}
              <li><span class="mono">{pkg}</span> <span class="muted">— {reason}</span></li>
            {/each}
          </ul>
        {/if}
      {/if}
    </div>
  {/if}

  {#if report}
    <div class="card">
      <h3>{i18n.t('Applied')} · <span class="badge ok">{okCount} {i18n.t('OK')}</span> {#if failCount > 0}<span class="badge critical">{failCount} {i18n.t('failed')}</span>{/if}</h3>
      <p class="muted">{i18n.t('Snapshot saved:')} <code class="mono">{report.snapshot_id.slice(0, 12)}…</code> — {i18n.t('use the list below to undo.')}</p>
    </div>
  {/if}

  <h3 class="sec">{i18n.t('Snapshots & rollback')}</h3>
  {#if rollbackReport}
    <div class="card">
      <p><span class="badge ok">{rollbackReport.applied} {i18n.t('restored')}</span>
        {#if rollbackReport.failed.length > 0}<span class="badge critical">{rollbackReport.failed.length} {i18n.t('failed')}</span>{/if}
      </p>
    </div>
  {/if}
  <div class="card snap-card">
    {#if snapshotStore.loading}
      <p class="muted">{i18n.t('Loading…')}</p>
    {:else if snapshotStore.list.length === 0}
      <p class="muted">{i18n.t('No snapshots yet. Apply a profile and a restore point appears here automatically.')}</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>{i18n.t('When')}</th>
            <th>{i18n.t('Label')}</th>
            <th>{i18n.t('Apps')}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each snapshotStore.list as s (s.id)}
            <tr>
              <td>{formatTimestamp(s.created_at)}</td>
              <td>{s.label ?? i18n.t('Auto snapshot')}</td>
              <td class="mono">{s.packages}</td>
              <td class="right">
                <button class="ghost small" onclick={() => rollback(s.id)} disabled={rollbackBusy === s.id}>
                  {rollbackBusy === s.id ? i18n.t('Restoring…') : i18n.t('Undo')}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
{/if}

<style>
  .sec { margin: 1.75rem 0 0.85rem; }
  .presets { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 1rem; }
  .preset {
    text-align: left;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 1rem 1.15rem;
    cursor: pointer;
    transition: border-color var(--t-fast), background var(--t-fast), transform var(--t-fast);
  }
  .preset:hover { border-color: var(--border-strong); }
  .preset.active { border-color: var(--accent); background: var(--accent-soft); }
  .preset-top { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.4rem; }
  .preset-dot { width: 9px; height: 9px; border-radius: 50%; flex-shrink: 0; }
  .preset[data-tone="good"]   .preset-dot { background: var(--good); }
  .preset[data-tone="accent"] .preset-dot { background: var(--accent); }
  .preset[data-tone="warn"]   .preset-dot { background: var(--warn); }
  .preset[data-tone="bad"]    .preset-dot { background: var(--bad); }
  .preset-name { font-family: var(--font-display); font-weight: 600; font-size: 1rem; color: var(--fg-0); }
  .preset-desc { margin: 0; color: var(--fg-2); font-size: 12.5px; line-height: 1.45; }

  .preview-card { margin-top: 1rem; }
  .preview-head { display: flex; align-items: center; justify-content: space-between; gap: 1rem; margin-bottom: 1rem; flex-wrap: wrap; }
  .preview-head h3 { margin: 0; }
  .metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 0.75rem; }
  .metric { background: var(--bg-1); border: 1px solid var(--border); border-radius: var(--radius); padding: 0.85rem 1rem; }
  .mval { font-family: var(--font-mono); font-size: 24px; font-weight: 600; color: var(--fg-0); }
  .mlab { font-size: 11.5px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--fg-3); margin-top: 0.25rem; }
  .link-btn { background: none; border: none; color: var(--accent); padding: 0.6rem 0 0; cursor: pointer; font-size: 13px; }
  .excluded { margin: 0.5rem 0 0; padding-left: 1.1rem; font-size: 12.5px; color: var(--fg-1); max-height: 220px; overflow-y: auto; }
  .excluded li { margin: 0.15rem 0; }

  .snap-card { padding: 0.5rem 0.5rem; }
  .right { text-align: right; }
  button.small { padding: 0.35rem 0.8rem; font-size: 12.5px; }
</style>
