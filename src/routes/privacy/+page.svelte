<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import AppName from '$components/AppName.svelte';
  import type {
    DnsPreset,
    InstalledPackage,
    PrivacyAppEntry,
    PrivacyState,
    PrivateDnsMode,
    SystemTweaks,
    PerformanceSettings
  } from '$types';

  type Tab = 'firewall' | 'dns' | 'clipboard' | 'data_saver' | 'permissions';
  let tab: Tab = $state('firewall');

  // ---- Shared state ----
  let privacyState: PrivacyState | null = $state(null);
  let packages: InstalledPackage[] = $state([]);
  let presets: DnsPreset[] = $state([]);
  let tweaks: SystemTweaks | null = $state(null);
  let perfSettings: PerformanceSettings | null = $state(null);
  let dangerousPermissions: Array<{ package: string, permissions: Record<string, string> }> = $state([]);
  let captiveBusy = $state(false);
  let dataSaverBusy = $state(false);
  let loading = $state(false);
  let busy = $state(false);
  let error: string | null = $state(null);
  let success: string | null = $state(null);

  // ---- DNS controls ----
  let dnsMode: PrivateDnsMode = $state('opportunistic');
  let dnsHostname = $state('');

  // ---- App selection (shared between firewall + clipboard tabs) ----
  let appFilter = $state('');
  let selected: Set<string> = $state(new Set());
  let showSystem = $state(false);
  let showUserOnly = $state(true);

  async function refresh() {
    if (!deviceStore.selected) return;
    loading = true;
    error = null;
    try {
      const [s, ps, pkgs, tw, perf, dp] = await Promise.all([
        cache.getOrFetch('privacy:' + deviceStore.selected.serial, TTL.medium, () => api.getPrivacyState(deviceStore.selected!.serial)),
        cache.getOrFetch('dns-presets', TTL.long, () => api.listDnsPresets()),
        cache.getOrFetch('packages:' + deviceStore.selected.serial, TTL.medium, () => api.listPackages(deviceStore.selected!.serial)),
        cache.getOrFetch('tweaks:' + deviceStore.selected.serial, TTL.medium, () => api.getSystemTweaks(deviceStore.selected!.serial)),
        cache.getOrFetch('perf:' + deviceStore.selected.serial, TTL.medium, () => api.getPerformanceSettings(deviceStore.selected!.serial)),
        cache.getOrFetch('dangerous_perms:' + deviceStore.selected.serial, TTL.medium, () => api.getDangerousPermissions(deviceStore.selected!.serial))
      ]);
      privacyState = s;
      presets = ps;
      packages = pkgs;
      tweaks = tw;
      perfSettings = perf;
      dangerousPermissions = dp;
      dnsMode = s.dns.mode;
      dnsHostname = s.dns.hostname ?? '';
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') refresh();
  });

  function toggle(pkg: string) {
    if (selected.has(pkg)) selected.delete(pkg); else selected.add(pkg);
    selected = new Set(selected);
  }
  function clearSelection() { selected = new Set(); }
  function selectVisible(list: InstalledPackage[]) {
    const next = new Set(selected);
    for (const p of list) next.add(p.name.toString());
    selected = next;
  }

  async function applyDns() {
    if (!deviceStore.selected) return;
    busy = true; error = null; success = null;
    try {
      await api.setPrivateDns(
        deviceStore.selected.serial,
        dnsMode,
        dnsMode === 'hostname' ? dnsHostname.trim() : null
      );
      success = `Private DNS set to ${dnsMode}${dnsMode === 'hostname' ? ` → ${dnsHostname}` : ''}.`;
      cache.invalidatePrefix('privacy:');
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
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

  function pickPreset(p: DnsPreset) {
    dnsMode = 'hostname';
    dnsHostname = p.hostname;
  }

  async function toggleCaptivePortal() {
    if (!deviceStore.selected || !tweaks) return;
    captiveBusy = true; error = null; success = null;
    try {
      const currentlySuppressed = tweaks.captive_portal_mode === 0;
      const nextDisabled = !currentlySuppressed;
      await api.setCaptivePortalMode(deviceStore.selected.serial, nextDisabled);
      cache.invalidatePrefix('tweaks:');
      success = nextDisabled
        ? 'Captive portal pings blocked. Wi-Fi networks now connect without phoning Google.'
        : 'Captive portal pings restored to Android default.';
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { captiveBusy = false; }
  }

  async function toggleDataSaver() {
    if (!deviceStore.selected || !perfSettings) return;
    dataSaverBusy = true; error = null; success = null;
    try {
      const target = !perfSettings.restrict_background_data;
      await api.setDataSaver(deviceStore.selected.serial, target);
      cache.invalidatePrefix('perf:');
      perfSettings.restrict_background_data = target;
      success = target ? 'Data Saver ENABLED.' : 'Data Saver DISABLED.';
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { dataSaverBusy = false; }
  }

  async function applyAction(kind: 'firewall' | 'clipboard', block: boolean) {
    if (!deviceStore.selected || selected.size === 0) return;
    const pkgs: string[] = Array.from(selected);
    const verb = block ? 'block' : 'unblock';
    if (!confirm(`${verb.toUpperCase()} ${kind} on ${pkgs.length} app(s)?`)) return;
    busy = true; error = null; success = null;
    try {
      if (kind === 'firewall') {
        await api.applyFirewall(deviceStore.selected.serial, pkgs, block);
        cache.invalidatePrefix('privacy:');
      } else {
        await api.applyClipboardGuard(deviceStore.selected.serial, pkgs, block);
        cache.invalidatePrefix('privacy:');
      }
      success = `${block ? 'Blocked' : 'Restored'} ${kind} on ${pkgs.length} app(s).`;
      clearSelection();
      await refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally { busy = false; }
  }

  // Map: package -> existing privacy entry from the scan
  const entryByPkg: Map<string, PrivacyAppEntry> = $derived.by(() => {
    const m = new Map<string, PrivacyAppEntry>();
    if (privacyState) for (const e of privacyState.scan.apps) m.set(e.package, e);
    return m;
  });

  // Combined list = installed packages augmented with privacy state
  const visibleApps: InstalledPackage[] = $derived.by(() => {
    const filtered = packages.filter((p: InstalledPackage) => {
      if (showUserOnly && p.is_system) return false;
      if (!showSystem && p.is_system) return false;
      if (appFilter) {
        const q = appFilter.toLowerCase();
        const name = p.name.toString().toLowerCase();
        const label = (p.label ?? '').toLowerCase();
        if (!name.includes(q) && !label.includes(q)) return false;
      }
      return true;
    });
    return filtered.slice(0, 500);
  });

  // Counts for header summary
  const firewallCount: number = $derived.by(() => {
    if (!privacyState) return 0;
    return privacyState.scan.apps.filter((a) => a.firewall_active).length;
  });
  const clipboardCount: number = $derived.by(() => {
    if (!privacyState) return 0;
    return privacyState.scan.apps.filter((a) => a.clipboard_blocked).length;
  });

  function isFirewalled(pkg: string): boolean {
    return entryByPkg.get(pkg)?.firewall_active ?? false;
  }
  function isClipboardBlocked(pkg: string): boolean {
    return entryByPkg.get(pkg)?.clipboard_blocked ?? false;
  }
</script>

<header class="page-head">
  <div>
    <h1>Privacy</h1>
    <p class="muted">
      System-wide DNS, per-app background firewall, and clipboard guard.
      Bypasses the need for a local VPN.
    </p>
  </div>
  <div class="head-actions">
    {#if privacyState}
      <div class="kpi-row">
        <span class="kpi"><strong>{firewallCount}</strong> firewalled</span>
        <span class="kpi"><strong>{clipboardCount}</strong> clipboard-blocked</span>
      </div>
    {/if}
    <button class="primary" onclick={refresh} disabled={loading || !deviceStore.selected}>
      {loading ? 'RefreshingÃ¢â‚¬Â¦' : 'Refresh'}
    </button>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected.</p></div>
{:else}
  <div class="seg" role="tablist">
    <button class:active={tab === 'firewall'} onclick={() => tab = 'firewall'} role="tab">
      Firewall <span class="seg-count">{firewallCount}</span>
    </button>
    <button class:active={tab === 'dns'} onclick={() => tab = 'dns'} role="tab">
      Private DNS
    </button>
    <button class:active={tab === 'clipboard'} onclick={() => tab = 'clipboard'} role="tab">
      Clipboard <span class="seg-count">{clipboardCount}</span>
    </button>
    <button class:active={tab === 'data_saver'} onclick={() => tab = 'data_saver'} role="tab">
      Data Saver
    </button>
    <button class:active={tab === 'permissions'} onclick={() => tab = 'permissions'} role="tab">
      Permissions Audit
    </button>
  </div>

  {#if error}<div class="error">{error}</div>{/if}
  {#if success}<div class="success">{success}</div>{/if}

  <!-- ===== DNS TAB ===== -->
  {#if tab === 'dns'}
    <div class="card">
      <h3>Current Private DNS</h3>
      {#if !privacyState}
        <Skeleton lines={3} />
      {:else}
        <div class="dns-current">
          <div>
            <span class="muted small">Mode:</span>
            <code class="mono pill" data-mode={privacyState.dns.mode}>{privacyState.dns.mode}</code>
          </div>
          {#if privacyState.dns.hostname}
            <div>
              <span class="muted small">Hostname:</span>
              <code class="mono">{privacyState.dns.hostname}</code>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <div class="card" style="margin-top: 1rem;">
      <h3>Set DNS</h3>
      <div class="form-grid">
        <label>
          Mode
          <select bind:value={dnsMode}>
            <option value="off">Off Ã¢â‚¬â€ no DoT, plain DNS</option>
            <option value="opportunistic">Opportunistic (Android default)</option>
            <option value="hostname">Hostname Ã¢â‚¬â€ force a specific DoT server</option>
          </select>
        </label>
        {#if dnsMode === 'hostname'}
          <label>
            Hostname (DNS-over-TLS endpoint)
            <input
              type="text"
              bind:value={dnsHostname}
              placeholder="dns.adguard-dns.com"
              spellcheck="false"
              autocomplete="off"
            />
          </label>
        {/if}
      </div>
      <button class="primary" onclick={applyDns} disabled={busy} style="margin-top: 0.85rem;">
        {busy ? 'ApplyingÃ¢â‚¬Â¦' : 'Apply DNS'}
      </button>
    </div>

    {#if presets.length > 0}
      <div class="card" style="margin-top: 1rem;">
        <h3>Presets</h3>
        <p class="muted footnote">Tap a preset to fill the hostname field above.</p>
        <div class="preset-grid">
          {#each presets as p (p.hostname)}
            <button class="preset-card" onclick={() => pickPreset(p)}>
              <div class="preset-label">{p.label}</div>
              <code class="mono preset-host">{p.hostname}</code>
              <div class="preset-flags">
                {#if p.blocks_ads}<span class="badge ok">ads</span>{/if}
                {#if p.blocks_trackers}<span class="badge ok">trackers</span>{/if}
              </div>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="card" style="margin-top: 1rem;">
      <div class="captive-row">
        <div>
          <h3 style="margin: 0 0 0.4rem 0;">Captive portal pings</h3>
          <p class="muted" style="margin: 0;">
            Android pings <code class="mono">connectivitycheck.gstatic.com</code> on every Wi-Fi connect.
            Disabling stops the probe.
            {#if tweaks}
              Currently:
              <code class="mono pill" data-state={tweaks.captive_portal_mode === 0 ? 'off' : 'on'}>
                {tweaks.captive_portal_mode === 0 ? 'BLOCKED' : (tweaks.captive_portal_mode === null ? 'default' : 'enabled')}
              </code>
            {/if}
          </p>
        </div>
        <button
          class={tweaks?.captive_portal_mode === 0 ? 'primary' : 'danger'}
          onclick={toggleCaptivePortal}
          disabled={captiveBusy || !tweaks}
        >
          {captiveBusy ? '…' : (tweaks?.captive_portal_mode === 0 ? 'Re-enable' : 'Block')}
        </button>
      </div>
    </div>

  <!-- ===== DATA SAVER TAB ===== -->
  {:else if tab === 'data_saver'}
    <div class="card flat banner">
      <p>Data Saver reduces data usage by preventing most apps and services from sending or receiving data in the background.</p>
    </div>
    <div class="card" style="padding: 1.25rem;">
      <h3 style="margin: 0 0 0.75rem;">Global Data Saver</h3>
      <p class="muted small" style="margin: 0 0 0.85rem;">
        When enabled, Android restricts background data. This significantly reduces idle battery drain and network traffic.
        Currently: 
        {#if perfSettings}
          <code class="mono pill" data-state={perfSettings.restrict_background_data ? 'on' : 'off'}>
            {perfSettings.restrict_background_data ? 'ENABLED' : 'DISABLED'}
          </code>
        {:else}
          <code class="mono pill">...</code>
        {/if}
      </p>
      {#if perfSettings}
        <button
          class={perfSettings.restrict_background_data ? 'danger' : 'primary'}
          onclick={toggleDataSaver}
          disabled={dataSaverBusy}
        >
          {dataSaverBusy ? '…' : (perfSettings.restrict_background_data ? 'Disable Data Saver' : 'Enable Data Saver')}
        </button>
      {/if}
    </div>

  <!-- ===== FIREWALL & CLIPBOARD TABS ===== -->
  {:else if tab === 'firewall' || tab === 'clipboard'}
    {@const isFirewall = tab === 'firewall'}
    {@const description = isFirewall
      ? 'Block background data/CPU work via RUN_ANY_IN_BACKGROUND + RUN_IN_BACKGROUND ops. Foreground use is unaffected.'
      : 'Deny READ_CLIPBOARD to selected apps. Apps that need clipboard for legitimate paste will silently get empty.'}

    <div class="card flat banner">
      <p>{description}</p>
    </div>

    <div class="card filter-bar">
      <input
        type="search"
        placeholder="Filter by package or labelÃ¢â‚¬Â¦"
        bind:value={appFilter}
      />
      <label class="inline">
        <input type="checkbox" bind:checked={showUserOnly} />
        User apps only
      </label>
      <label class="inline">
        <input type="checkbox" bind:checked={showSystem} disabled={showUserOnly} />
        Show system
      </label>
      <div class="filter-actions">
        <button onclick={() => selectVisible(visibleApps)} disabled={visibleApps.length === 0}>
          Select visible ({visibleApps.length})
        </button>
        <button onclick={clearSelection} disabled={selected.size === 0}>
          Clear ({selected.size})
        </button>
      </div>
    </div>

    {#if loading}
      <div class="card"><Skeleton lines={8} /></div>
    {:else}
      <div class="card table-card">
        <div class="scroll-y" style="max-height: 55vh;">
          <table>
            <thead>
              <tr>
                <th style="width: 30px;"></th>
                <th>Package</th>
                <th>UID</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {#each visibleApps as pkg (pkg.name)}
                {@const pname = pkg.name.toString()}
                {@const blocked = isFirewall ? isFirewalled(pname) : isClipboardBlocked(pname)}
                <tr class:blocked onclick={() => toggle(pname)}>
                  <td>
                    <input
                      type="checkbox"
                      checked={selected.has(pname)}
                      onclick={(e) => e.stopPropagation()}
                      onchange={() => toggle(pname)}
                      aria-label="select"
                    />
                  </td>
                  <td>
                    <AppName package={pname} />
                  </td>
                  <td class="mono small">{pkg.uid}</td>
                  <td>
                    {#if blocked}
                      <span class="badge critical">BLOCKED</span>
                    {:else}
                      <span class="muted small">default</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        {#if packages.length > visibleApps.length}
          <p class="muted footnote">
            Showing {visibleApps.length} of {packages.length}. Narrow with the filter.
          </p>
        {/if}
      </div>

      <div class="action-bar" class:active={selected.size > 0}>
        <span class="muted">{selected.size} selected</span>
        <div class="action-buttons">
          <button class="danger" onclick={() => applyAction(tab as 'firewall' | 'clipboard', true)} disabled={busy || selected.size === 0}>
            {busy ? 'Ã¢â‚¬Â¦' : (isFirewall ? 'Firewall selected' : 'Block clipboard')}
          </button>
          <button onclick={() => applyAction(tab as 'firewall' | 'clipboard', false)} disabled={busy || selected.size === 0}>
            Restore selected
          </button>
        </div>
      </div>
    {/if}
  {/if}

  <!-- ===== PERMISSIONS AUDIT TAB ===== -->
  {#if tab === 'permissions'}
    <div class="card">
      <h3>Permissions Audit</h3>
      <p class="muted small">Review and revoke dangerous permissions (Camera, Location, Mic) held by third-party apps.</p>
      {#if !dangerousPermissions || dangerousPermissions.length === 0}
        <div class="empty-state">No dangerous permissions found.</div>
      {:else}
        <div class="table-wrap" style="margin-top: 1rem;">
          <table class="data-table">
            <thead>
              <tr>
                <th>App</th>
                <th>Granted Permissions</th>
                <th style="width: 100px;">Actions</th>
              </tr>
            </thead>
            <tbody>
              {#each dangerousPermissions as dp}
                <tr>
                  <td><AppName pkg={dp.package} /></td>
                  <td>
                    <div style="display: flex; gap: 0.5rem; flex-wrap: wrap;">
                      {#each Object.entries(dp.permissions) as [perm, mode]}
                        <span class="pill outline {mode === 'foreground' ? 'warn' : ''}" style="display: flex; align-items: center; gap: 0.25rem;">
                          {perm} ({mode})
                          <button class="btn outline error" style="padding: 0.1rem 0.25rem; min-width: auto; font-size: 10px;" onclick={() => revokePermission(dp.package, perm)} disabled={busy}>Revoke</button>
                        </span>
                      {/each}
                    </div>
                  </td>
                  <td>
                    <button class="btn small outline" onclick={() => api.openAppSettings(deviceStore.selected!.serial, dp.package)}>OS Settings</button>
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
  .page-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    margin-bottom: 1.5rem;
    gap: 1rem;
  }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 540px; }
  .head-actions { display: flex; align-items: center; gap: 0.85rem; }
  .kpi-row { display: flex; gap: 0.75rem; }
  .kpi {
    font-size: var(--font-size-xs);
    color: var(--fg-2);
    background: var(--bg-2);
    padding: 0.35rem 0.7rem;
    border-radius: 99px;
    border: 1px solid var(--border);
  }
  .kpi strong { color: var(--fg-0); font-family: var(--font-mono); margin-right: 4px; }

  /* Segmented control */
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

  /* DNS tab */
  .dns-current {
    display: flex;
    gap: 1.5rem;
    flex-wrap: wrap;
    margin-top: 0.5rem;
  }
  .dns-current .small { font-size: var(--font-size-xs); }
  .pill {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 99px;
    background: var(--bg-3);
    font-size: var(--font-size-xs);
  }
  .pill[data-mode="off"]          { color: var(--bad); }
  .pill[data-mode="opportunistic"]{ color: var(--fg-2); }
  .pill[data-mode="hostname"]     { color: var(--good); background: rgba(16, 185, 129, 0.1); }

  .form-grid { display: grid; gap: 1rem; }
  .form-grid label { display: flex; flex-direction: column; gap: 4px; font-size: var(--font-size-sm); color: var(--fg-2); }

  .preset-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 0.65rem;
    margin-top: 0.85rem;
  }
  .preset-card {
    background: var(--bg-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.7rem 0.85rem;
    text-align: left;
    cursor: pointer;
    transition: border-color var(--t-fast), transform var(--t-fast);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .preset-card:hover { border-color: var(--accent); transform: translateY(-1px); }
  .preset-label { font-weight: 600; color: var(--fg-0); font-size: var(--font-size-sm); }
  .preset-host { font-size: var(--font-size-xs); color: var(--fg-2); word-break: break-all; }
  .preset-flags { display: flex; gap: 4px; margin-top: 4px; }

  /* Filter bar */
  .filter-bar {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 0.85rem;
    padding: 0.6rem 0.85rem;
  }
  .filter-bar input[type="search"] { flex: 1; min-width: 240px; max-width: 360px; }
  .filter-bar label.inline {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: var(--font-size-sm);
    color: var(--fg-2);
    cursor: pointer;
  }
  .filter-bar input[type="checkbox"] { width: auto; cursor: pointer; }
  .filter-actions { margin-left: auto; display: flex; gap: 0.4rem; }
  .filter-actions button { font-size: var(--font-size-xs); padding: 0.35rem 0.75rem; }

  /* Tables */
  .table-card { padding: 0.5rem; }
  table { width: 100%; font-size: 12.5px; }
  th { background: var(--bg-1); position: sticky; top: 0; z-index: 1; padding: 0.55rem 0.75rem; }
  tbody tr { cursor: pointer; transition: background var(--t-fast); }
  tbody tr:hover { background: var(--bg-3); }
  tbody tr.blocked { background: rgba(239, 68, 68, 0.03); }
  tbody td { padding: 0.5rem 0.75rem; }
  .pkg-cell { display: flex; flex-direction: column; gap: 2px; }
  .pkg-cell .small { font-size: 11px; }

  /* Action bar */
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

  .banner {
    padding: 0.65rem 1rem;
    border-color: rgba(56, 189, 248, 0.2);
    background: rgba(56, 189, 248, 0.04);
    margin-bottom: 1rem;
  }
  .banner p { margin: 0; font-size: var(--font-size-sm); color: var(--fg-1); }

  .footnote { font-size: var(--font-size-xs); margin: 0.5rem 0 0; }
  .small { font-size: var(--font-size-xs); }
  .captive-row {
    display: flex; justify-content: space-between; align-items: center;
    gap: 1rem; padding: 0.25rem 0;
  }
  .pill[data-state="off"] { color: var(--good); background: rgba(16, 185, 129, 0.1); }
  .pill[data-state="on"]  { color: var(--fg-2); }
</style>
