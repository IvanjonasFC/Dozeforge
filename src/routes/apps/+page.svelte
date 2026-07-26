<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import AppName from '$components/AppName.svelte';
  import RiskBadge from '$components/RiskBadge.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import DebloatWizard from '$components/DebloatWizard.svelte';

  let wizardOpen = $state(false);
  import { appModalStore } from '$stores/appModal.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import type {
    InstalledPackage,
    PrivacyState,
    BloatwareRecommendation,
    Recommendation,
    BloatwareReport,
    OptimizationReport,
    RiskTier
  } from '$types';

  // Map a bloatware recommendation to a risk badge (tier drives the color).
  function bloatBadge(rec: Recommendation): { tier: RiskTier; label: string } {
    switch (rec) {
      case 'do_not_touch':         return { tier: 'critical', label: i18n.t('Keep') };
      case 'system_use_with_care': return { tier: 'elevated', label: i18n.t('Care') };
      case 'preinstalled_bloat':   return { tier: 'moderate', label: i18n.t('Bloat') };
      case 'safe_to_disable':      return { tier: 'moderate', label: i18n.t('Safe') };
    }
  }

  type SubTab = 'manager' | 'permissions';
  let tab: SubTab = $state(page.url.searchParams.get('tab') === 'permissions' ? 'permissions' : 'manager');

  interface AppModel {
    package: string;
    uid: number;
    isSystem: boolean;
    disabled: boolean;
    bloatRec: Recommendation | 'unknown';
    bloatTier: string;
    communityVerified: boolean;
    bloatNotes: string;
    firewalled: boolean;
    clipboardBlocked: boolean;
    perms: Record<string, string>;
  }

  let packages: InstalledPackage[] = $state([]);
  let privacyState: PrivacyState | null = $state(null);
  let bloatRecs: BloatwareRecommendation[] = $state([]);
  let dangerousPermissions: Array<{ package: string, permissions: Record<string, string> }> = $state([]);
  let disabledSet: Set<string> = $state(new Set());

  let loading = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);
  let sideloadBusy = $state(false);

  let filter = $state('');
  let showUserOnly = $state(true);
  let showSystem = $state(false);
  
  let selected = $state<Set<string>>(new Set());

  async function refresh() {
    if (!deviceStore.selected) return;
    loading = true; error = null; success = null;
    try {
      const serial = deviceStore.selected.serial;
      const [pkgs, priv, recs, dp, prof] = await Promise.all([
        cache.getOrFetch('packages:' + serial, TTL.medium, () => api.listPackages(serial)),
        cache.getOrFetch('privacy:' + serial, TTL.medium, () => api.getPrivacyState(serial)),
        cache.getOrFetch('bloat:' + serial, TTL.long, () => api.bloatwareRecommendations(serial)),
        cache.getOrFetch('dangerous_perms:' + serial, TTL.medium, () => api.getDangerousPermissions(serial)),
        cache.getOrFetch('profile:' + serial, TTL.short, () => api.exportNativeProfile(serial))
      ]);
      packages = pkgs;
      privacyState = priv;
      bloatRecs = recs;
      dangerousPermissions = dp;
      disabledSet = new Set(prof.disabled_packages);
      selected = new Set();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') refresh();
  });

  const appModels: AppModel[] = $derived.by(() => {
    const models: AppModel[] = [];
    if (!privacyState) return models;

    const privMap = new Map(privacyState.scan.apps.map(a => [a.package, a]));
    const recMap = new Map(bloatRecs.map(r => [r.package, r]));
    const permMap = new Map(dangerousPermissions.map(d => [d.package, d.permissions]));

    for (const p of packages) {
      const pname = p.name.toString();
      const priv = privMap.get(pname);
      const rec = recMap.get(pname);
      models.push({
        package: pname,
        uid: p.uid,
        isSystem: p.is_system,
        disabled: disabledSet.has(pname),
        bloatRec: rec?.recommendation ?? 'unknown',
        bloatTier: rec?.tier ?? 'unknown',
        communityVerified: rec?.community_verified ?? false,
        bloatNotes: rec?.notes ?? '',
        firewalled: priv?.firewall_active ?? false,
        clipboardBlocked: priv?.clipboard_blocked ?? false,
        perms: permMap.get(pname) ?? {}
      });
    }
    return models;
  });

  const visibleApps = $derived.by(() => {
    const serial = deviceStore.selected?.serial ?? null;
    const needle = filter.toLowerCase();
    
    return appModels.filter(m => {
      if (showUserOnly && m.isSystem) return false;
      if (!showSystem && m.isSystem) return false;
      
      if (needle) {
        const label = labelStore.labelFor(serial, m.package).toLowerCase();
        if (!m.package.toLowerCase().includes(needle) && !label.includes(needle)) return false;
      }
      return true;
    }).sort((a, b) => a.package.localeCompare(b.package));
  });

  const kpis = $derived.by(() => {
    let disabled = 0;
    let firewalled = 0;
    let clipboard = 0;
    for (const m of appModels) {
      if (m.disabled) disabled++;
      if (m.firewalled) firewalled++;
      if (m.clipboardBlocked) clipboard++;
    }
    return { disabled, firewalled, clipboard };
  });

  function toggle(pkg: string) {
    const next = new Set(selected);
    if (next.has(pkg)) next.delete(pkg);
    else next.add(pkg);
    selected = next;
  }
  function selectVisible() {
    const next = new Set(selected);
    for (const m of visibleApps) next.add(m.package);
    selected = next;
  }
  function clearSelection() { selected = new Set(); }

  async function batchAction(action: 'disable' | 'enable' | 'firewall_on' | 'firewall_off' | 'clip_on' | 'clip_off') {
    if (!deviceStore.selected || selected.size === 0) return;
    const serial = deviceStore.selected.serial;
    const pkgs = Array.from(selected);
    
    // Safety check for disable
    if (action === 'disable') {
      const unsafe = pkgs.filter(p => {
        const m = appModels.find(x => x.package === p);
        return m?.bloatRec === 'do_not_touch';
      });
      if (unsafe.length > 0) {
        if (!confirm(`WARNING: You selected ${unsafe.length} critical system apps to disable. This may cause bootloops. Continue?`)) return;
      }
    }

    busy = true; error = null; success = null;
    try {
      if (action === 'disable') {
        await api.disableBloatware(serial, pkgs);
        pkgs.forEach(p => disabledSet.add(p));
        disabledSet = new Set(disabledSet); // Reactivity
        cache.invalidatePrefix('profile:');
        success = `Disabled ${pkgs.length} apps.`;
      } else if (action === 'enable') {
        await api.enableBloatware(serial, pkgs);
        pkgs.forEach(p => disabledSet.delete(p));
        disabledSet = new Set(disabledSet);
        cache.invalidatePrefix('profile:');
        success = `Enabled ${pkgs.length} apps.`;
      } else if (action === 'firewall_on' || action === 'firewall_off') {
        const block = action === 'firewall_on';
        await api.applyFirewall(serial, pkgs, block);
        cache.invalidatePrefix('privacy:');
        success = `${block ? 'Blocked' : 'Restored'} background data/CPU for ${pkgs.length} apps.`;
        await refresh();
      } else if (action === 'clip_on' || action === 'clip_off') {
        const block = action === 'clip_on';
        await api.applyClipboardGuard(serial, pkgs, block);
        cache.invalidatePrefix('privacy:');
        success = `${block ? 'Blocked' : 'Restored'} clipboard access for ${pkgs.length} apps.`;
        await refresh();
      }
      clearSelection();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      busy = false;
    }
  }

  async function revokePermission(pkg: string, permission: string) {
    if (!deviceStore.selected) return;
    busy = true; error = null; success = null;
    try {
      await api.setAppOps(deviceStore.selected.serial, pkg, permission, 'ignore');
      success = `Revoked ${permission} for ${pkg}.`;
      cache.invalidatePrefix('dangerous_perms:');
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }

  function getRecBadge(rec: string): string {
    switch (rec) {
      case 'safe_to_disable': return 'rec-safe';
      case 'preinstalled_bloat': return 'rec-bloat';
      case 'system_use_with_care': return 'rec-careful';
      case 'do_not_touch': return 'rec-critical';
      default: return '';
    }
  }

  async function sideloadApk() {
    if (!deviceStore.selected) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({
      title: 'Select APK to install',
      filters: [{ name: 'Android Package', extensions: ['apk'] }],
      multiple: false
    });
    if (!path) return;
    sideloadBusy = true; error = null; success = null;
    try {
      const result = await api.sideloadApk(deviceStore.selected.serial, path as string);
      success = `APK installed successfully. ${result}`;
      cache.invalidatePrefix('packages:');
      await refresh();
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { sideloadBusy = false; }
  }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('App Manager')}</h1>
    <p class="muted">
      {i18n.t('Unified control over bloatware, background firewalls, clipboard restrictions, and permissions.')}
    </p>
  </div>
  <div class="head-actions">
    {#if !loading}
      <div class="kpi-row">
        <span class="kpi"><strong>{kpis.disabled}</strong> {i18n.t('disabled')}</span>
        <span class="kpi"><strong>{kpis.firewalled}</strong> {i18n.t('firewalled')}</span>
        <span class="kpi"><strong>{kpis.clipboard}</strong> {i18n.t('clip-blocked')}</span>
      </div>
    {/if}
    <button class="primary" onclick={refresh} disabled={loading || !deviceStore.selected}>
      {loading ? i18n.t('Loading...') : i18n.t('Refresh')}
    </button>
    <button class="outline" onclick={() => wizardOpen = true} disabled={!deviceStore.selected}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align:-2px; margin-right:4px;"><path d="M15 4V2M15 16v-2M8 9h2M20 9h2M17.8 11.8 19 13M15 9h.01M17.8 6.2 19 5M3 21l9-9M12.2 6.2 11 5"/></svg>
      {i18n.t('Debloat Wizard')}
    </button>
    <button class="outline" onclick={sideloadApk} disabled={sideloadBusy || !deviceStore.selected}>
      {sideloadBusy ? i18n.t('Loading...') : i18n.t('Sideload APK')}
    </button>
  </div>
</header>

<DebloatWizard bind:open={wizardOpen} onApplied={refresh} />

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else}
  <div class="seg" role="tablist">
    <button class:active={tab === 'manager'} onclick={() => tab = 'manager'} role="tab">{i18n.t('App Control')}</button>
    <button class:active={tab === 'permissions'} onclick={() => tab = 'permissions'} role="tab">{i18n.t('Permissions Audit')}</button>
  </div>

  {#if error}<div class="error">{error}</div>{/if}
  {#if success}<div class="success">{success}</div>{/if}

  {#if tab === 'manager'}
    <!-- App Control Filters -->
    <div class="card filter-bar">
      <input type="search" placeholder={i18n.t('Filter by package or label...')} bind:value={filter} />
      <label class="inline">
        <input type="checkbox" bind:checked={showUserOnly} /> {i18n.t('User apps only')}
      </label>
      <label class="inline">
        <input type="checkbox" bind:checked={showSystem} disabled={showUserOnly} /> {i18n.t('Show system')}
      </label>
      <div class="filter-actions">
        <button onclick={selectVisible} disabled={visibleApps.length === 0}>{i18n.t('Select visible')} ({visibleApps.length})</button>
        <button onclick={clearSelection} disabled={selected.size === 0}>{i18n.t('Clear')} ({selected.size})</button>
      </div>
    </div>

    {#if loading}
      <div class="card"><Skeleton lines={8} /></div>
    {:else}
      <div class="card table-card">
        <div class="scroll-y" style="max-height: 50vh;">
          <table>
            <thead>
              <tr>
                <th style="width:40px;"></th>
                <th>{i18n.t('PACKAGE')}</th>
                <th class="center">{i18n.t('STATUS')}</th>
                <th class="center">{i18n.t('FIREWALL')}</th>
                <th class="center">{i18n.t('CLIPBOARD')}</th>
                <th class="right">{i18n.t('SAFETY')}</th>
              </tr>
            </thead>
            <tbody>
              {#each visibleApps as app (app.package)}
                <tr class:selected={selected.has(app.package)} class:disabled={app.disabled} onclick={() => appModalStore.open(app.package)}>
                  <td>
                    <input type="checkbox" checked={selected.has(app.package)} onclick={(e) => e.stopPropagation()} onchange={() => toggle(app.package)} />
                  </td>
                  <td>
                    <AppName package={app.package} />
                  </td>
                  <td>
                    {#if app.disabled}
                      <span class="badge" style="background: rgba(239, 68, 68, 0.1); color: var(--danger);">{i18n.t('DISABLED')}</span>
                    {:else}
                      <span class="badge" style="background: rgba(16, 185, 129, 0.1); color: var(--good);">{i18n.t('ENABLED')}</span>
                    {/if}
                  </td>
                  <td>
                    {#if app.firewalled}
                      <span class="badge critical">{i18n.t('Blocked')}</span>
                    {:else}
                      <span class="muted small">{i18n.t('Allowed')}</span>
                    {/if}
                  </td>
                  <td>
                    {#if app.clipboardBlocked}
                      <span class="badge critical">{i18n.t('Blocked')}</span>
                    {:else}
                      <span class="muted small">{i18n.t('Allowed')}</span>
                    {/if}
                  </td>
                  <td>
                    {#if app.bloatRec !== 'unknown'}
                      {@const bb = bloatBadge(app.bloatRec)}
                      <RiskBadge tier={bb.tier} label={bb.label} />
                      {#if app.communityVerified}
                        <span class="uad-badge" title={app.bloatNotes}>UAD‑NG</span>
                      {/if}
                    {:else if app.isSystem}
                      <span class="muted" style="font-size:10px;">{i18n.t('System')}</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

      <div class="action-bar" class:active={selected.size > 0}>
        <span class="muted">{selected.size} {i18n.t('apps selected')}</span>
        <div class="action-buttons">
          <button class="danger outline" onclick={() => batchAction('disable')} disabled={busy || selected.size === 0}>{i18n.t('Disable (Freeze)')}</button>
          <button class="good outline" onclick={() => batchAction('enable')} disabled={busy || selected.size === 0}>{i18n.t('Enable')}</button>
          <div style="width: 1px; background: var(--border); margin: 0 0.5rem;"></div>
          <button class="danger outline" onclick={() => batchAction('firewall_on')} disabled={busy || selected.size === 0}>{i18n.t('Block Firewall')}</button>
          <button class="outline" onclick={() => batchAction('firewall_off')} disabled={busy || selected.size === 0}>{i18n.t('Allow Firewall')}</button>
          <div style="width: 1px; background: var(--border); margin: 0 0.5rem;"></div>
          <button class="danger outline" onclick={() => batchAction('clip_on')} disabled={busy || selected.size === 0}>{i18n.t('Block Clipboard')}</button>
          <button class="outline" onclick={() => batchAction('clip_off')} disabled={busy || selected.size === 0}>{i18n.t('Allow Clipboard')}</button>
        </div>
      </div>
    {/if}
  {:else if tab === 'permissions'}
    <div class="card">
      <h3>{i18n.t('Permissions Audit')}</h3>
      <p class="muted small">{i18n.t('Review and revoke dangerous permissions (Camera, Location, Mic) held by third-party apps.')}</p>
      {#if dangerousPermissions.length === 0}
        <div class="empty-state">{i18n.t('No dangerous permissions found.')}</div>
      {:else}
        <div class="table-wrap" style="margin-top: 1rem; max-height: 50vh; overflow-y: auto;">
          <table class="data-table">
            <thead>
              <tr>
                <th>{i18n.t('PACKAGE')}</th>
                <th>{i18n.t('Granted Permissions')}</th>
                <th style="width: 100px;">{i18n.t('Actions')}</th>
              </tr>
            </thead>
            <tbody>
              {#each dangerousPermissions as dp}
                <tr>
                  <td><AppName package={dp.package} /></td>
                  <td>
                    <div style="display: flex; gap: 0.5rem; flex-wrap: wrap;">
                      {#each Object.entries(dp.permissions) as [perm, mode]}
                        <span class="pill outline {mode === 'foreground' ? 'warn' : ''}" style="display: flex; align-items: center; gap: 0.25rem;">
                          {perm} ({mode})
                          <button class="btn outline error" style="padding: 0.1rem 0.25rem; min-width: auto; font-size: 10px;" onclick={() => revokePermission(dp.package, perm)} disabled={busy}>{i18n.t('Revoke')}</button>
                        </span>
                      {/each}
                    </div>
                  </td>
                  <td>
                    <button class="btn small outline" onclick={() => api.openAppSettings(deviceStore.selected!.serial, dp.package)}>{i18n.t('OS Settings')}</button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {/if}
{/if}

<style>
  .uad-badge {
    display: inline-block;
    margin-left: 4px;
    padding: 1px 5px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.03em;
    border-radius: 4px;
    vertical-align: middle;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    cursor: help;
  }
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 540px; }
  .head-actions { display: flex; align-items: center; gap: 0.85rem; }
  .kpi-row { display: flex; gap: 0.75rem; }
  .kpi { font-size: var(--font-size-xs); color: var(--fg-2); background: var(--bg-2); padding: 0.35rem 0.7rem; border-radius: 99px; border: 1px solid var(--border); }
  .kpi strong { color: var(--fg-0); font-family: var(--font-mono); margin-right: 4px; }
  .seg { display: inline-flex; gap: 2px; padding: 3px; background: var(--control-bg); border: 1px solid var(--border); border-radius: 99px; margin-bottom: 1rem; }
  .seg button { padding: 0.45rem 1rem; border-radius: 99px; background: transparent; border: none; color: var(--fg-2); font-size: var(--font-size-sm); font-weight: 500; }
  .seg button.active { background: var(--bg-4); color: var(--fg-0); box-shadow: inset 0 0 0 1px var(--border-strong); }
  .success { padding: 0.65rem 1rem; background: rgba(16, 185, 129, 0.1); border-left: 3px solid var(--good); border-radius: var(--radius); color: var(--good); margin-bottom: 1rem; font-size: var(--font-size-sm); }
  .filter-bar { display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; margin-bottom: 0.85rem; padding: 0.6rem 0.85rem; }
  .filter-bar input[type="search"] { flex: 1; min-width: 240px; max-width: 360px; }
  .filter-bar label.inline { display: flex; align-items: center; gap: 0.4rem; font-size: var(--font-size-sm); color: var(--fg-2); cursor: pointer; }
  .filter-bar input[type="checkbox"] { width: auto; cursor: pointer; }
  .filter-actions { margin-left: auto; display: flex; gap: 0.4rem; }
  .filter-actions button { font-size: var(--font-size-xs); padding: 0.35rem 0.75rem; }
  .table-card { padding: 0.5rem; }
  table { width: 100%; font-size: 12.5px; }
  th { background: var(--bg-1); position: sticky; top: 0; z-index: 1; padding: 0.55rem 0.75rem; text-align: left; }
  tbody tr { cursor: pointer; transition: background var(--t-fast); }
  tbody tr:hover { background: var(--bg-3); }
  tbody tr.selected { background: rgba(255, 107, 0, 0.05); }
  tbody td { padding: 0.5rem 0.75rem; }
  .action-bar { margin-top: 1rem; padding: 0.85rem 1.15rem; background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius-lg); display: flex; justify-content: space-between; align-items: center; transition: all var(--t-base); }
  .action-bar.active { background: var(--bg-3); box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2); }
  .action-buttons { display: flex; gap: 0.4rem; }
  .outline { background: transparent; border: 1px solid var(--border); }
  .outline.danger { color: var(--danger); border-color: rgba(239, 68, 68, 0.3); }
  .outline.good { color: var(--good); border-color: rgba(16, 185, 129, 0.3); }
  .outline:hover:not(:disabled) { background: var(--bg-3); }
</style>
