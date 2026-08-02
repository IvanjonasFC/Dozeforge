<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import AppName from '$components/AppName.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import type { InstalledPackage } from '$types';

  // Seed from cache so tab revisits render instantly (stale-while-revalidate).
  const _seedPkgs = cache.peek<InstalledPackage[]>('packages:' + (deviceStore.selected?.serial ?? ''));
  let packages = $state<InstalledPackage[]>(_seedPkgs ? _seedPkgs.filter((p) => !p.is_system) : []);
  let loadingApps = $state(false);
  let filter = $state('');
  let selectedPkg = $state<string | null>(null);
  let selected = $state<Set<string>>(new Set());
  let includeData = $state(false);

  let backupBusy = $state(false);
  let restoreBusy = $state(false);
  let message = $state<string | null>(null);
  let error = $state<string | null>(null);

  const ready = $derived(deviceStore.selected?.state === 'device');

  onMount(() => { if (ready) loadApps(); });
  $effect(() => { if (ready && packages.length === 0 && !loadingApps) loadApps(); });

  async function loadApps() {
    if (!deviceStore.selected) return;
    const serial = deviceStore.selected.serial;
    loadingApps = cache.peek('packages:' + serial) === null; // skeleton only if nothing cached
    error = null;
    try {
      const pkgs = await cache.getOrFetch('packages:' + serial, TTL.medium, () => api.listPackages(serial));
      packages = pkgs.filter((p) => !p.is_system);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loadingApps = false;
    }
  }

  const visibleApps = $derived.by(() => {
    const serial = deviceStore.selected?.serial ?? null;
    const needle = filter.trim().toLowerCase();
    return packages
      .filter((p) => {
        if (!needle) return true;
        const label = labelStore.labelFor(serial, p.name.toString()).toLowerCase();
        return p.name.toString().toLowerCase().includes(needle) || label.includes(needle);
      })
      .sort((a, b) => a.name.toString().localeCompare(b.name.toString()))
      .slice(0, 300);
  });

  async function runBackup() {
    if (!deviceStore.selected || !selectedPkg) return;
    message = null; error = null;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const path = await save({ defaultPath: `${selectedPkg}.ab`, filters: [{ name: 'Android backup', extensions: ['ab'] }] });
      if (!path) return;
      backupBusy = true;
      message = i18n.t('Confirm the backup on your phone (and set a password to encrypt it). Waiting…');
      const res = await api.backupAppData(deviceStore.selected.serial, selectedPkg, path);
      message = `${i18n.t('Backup saved to')} ${path}. ${res}`.trim();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      backupBusy = false;
    }
  }

  async function runApkBackup() {
    if (!deviceStore.selected || !selectedPkg) return;
    message = null; error = null;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const path = await save({ defaultPath: `${selectedPkg}.zip`, filters: [{ name: 'APK bundle', extensions: ['zip', 'apk'] }] });
      if (!path) return;
      backupBusy = true;
      const res = await api.extractApk(deviceStore.selected.serial, selectedPkg, path);
      message = `${res}`.trim() || `${i18n.t('APK(s) saved to')} ${path}`;
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      backupBusy = false;
    }
  }

  async function runRestore() {
    if (!deviceStore.selected) return;
    message = null; error = null;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const sel = await open({ filters: [{ name: 'Android backup', extensions: ['ab'] }], multiple: false });
      if (!sel || Array.isArray(sel)) return;
      restoreBusy = true;
      message = i18n.t('Confirm the restore on your phone (enter the password if the backup is encrypted). Waiting…');
      const res = await api.restoreBackup(deviceStore.selected.serial, sel as string);
      message = `${i18n.t('Restore finished.')} ${res}`.trim();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      restoreBusy = false;
    }
  }

  function toggle(pkg: string) {
    const s = new Set(selected);
    if (s.has(pkg)) s.delete(pkg); else s.add(pkg);
    selected = s;
    selectedPkg = pkg;
  }
  function selectAllVisible() {
    const s = new Set(selected);
    for (const p of visibleApps) s.add(p.name.toString());
    selected = s;
    if (visibleApps[0]) selectedPkg = visibleApps[0].name.toString();
  }
  function clearSelection() { selected = new Set(); }

  // Batch APK backup: one .zip (base + splits + install_me.bat) per app into a folder.
  async function backupApksBatch(pkgs: string[]) {
    if (!deviceStore.selected || pkgs.length === 0) return;
    message = null; error = null;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const folder = await open({ directory: true, multiple: false });
    if (!folder || Array.isArray(folder)) return;
    const base = String(folder);
    const sep = base.includes('\\') ? '\\' : '/';
    backupBusy = true;
    const serial = deviceStore.selected.serial;
    let ok = 0; const fails: string[] = [];
    for (const pkg of pkgs) {
      message = i18n.t('Backing up {{n}}/{{total}}: {{pkg}}…', { n: String(ok + fails.length + 1), total: String(pkgs.length), pkg });
      try {
        await api.extractApk(serial, pkg, `${base}${sep}${pkg}.zip`);
        if (includeData) { try { await api.backupExternalData(serial, pkg, `${base}${sep}${pkg}_data`); } catch { /* scoped storage may block it */ } }
        ok++;
      } catch { fails.push(pkg); }
    }
    backupBusy = false;
    message = i18n.t('APK backup done: {{ok}} saved, {{fail}} failed. Folder: {{folder}}', { ok: String(ok), fail: String(fails.length), folder: base });
    if (fails.length) error = i18n.t('Could not back up: {{list}}', { list: fails.slice(0, 8).join(', ') });
  }

  // Whole-phone content backup: pulls all of /sdcard (photos, videos, docs, media). No root.
  async function runSdcardBackup() {
    if (!deviceStore.selected) return;
    message = null; error = null;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const folder = await open({ directory: true, multiple: false });
    if (!folder || Array.isArray(folder)) return;
    backupBusy = true;
    message = i18n.t('Backing up internal storage — this can take a while for large storage…');
    try {
      const res = await api.backupSdcard(deviceStore.selected.serial, String(folder));
      message = `${i18n.t('Internal storage backed up to {{folder}}', { folder: String(folder) })} ${res}`.trim();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      backupBusy = false;
    }
  }

</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Backup & Restore')}</h1>
    <p class="muted">{i18n.t('Full app backups (APK + data) to an encrypted .ab archive, and restore them back — using Android’s native adb backup.')}</p>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else if !ready}
  <div class="card empty"><p class="muted">{i18n.t('Device is offline or unauthorized.')}</p></div>
{:else}
  {#if message}<div class="success">{message}</div>{/if}
  {#if error}<div class="error">{error}</div>{/if}

  <div class="grid two">
    <!-- Backup -->
    <div class="card">
      <h3>{i18n.t('Back up an app')}</h3>
      <p class="muted small">{i18n.t('Pick a user app, choose where to save the .ab, then confirm on the phone.')}</p>

      <input class="filter" bind:value={filter} placeholder={i18n.t('Filter apps…')} spellcheck="false" autocomplete="off" />
      <div class="sel-bar">
        <span class="muted small">{selected.size} {i18n.t('selected')}</span>
        <div class="sel-actions">
          <button class="mini" onclick={selectAllVisible}>{i18n.t('Select shown')}</button>
          <button class="mini" onclick={clearSelection} disabled={selected.size === 0}>{i18n.t('Clear')}</button>
        </div>
      </div>
      <div class="applist">
        {#if loadingApps}
          <p class="muted small" style="padding: 0.5rem;">{i18n.t('Loading apps…')}</p>
        {:else}
          {#each visibleApps as p (p.name.toString())}
            <button class="app-item" class:selected={selected.has(p.name.toString())} class:active={selectedPkg === p.name.toString()} onclick={() => toggle(p.name.toString())}>
              <span class="chk" class:on={selected.has(p.name.toString())} aria-hidden="true">
                {#if selected.has(p.name.toString())}<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>{/if}
              </span>
              <AppName package={p.name.toString()} size="sm" />
            </button>
          {/each}
          {#if visibleApps.length === 0}<p class="muted small" style="padding: 0.5rem;">{i18n.t('No apps match.')}</p>{/if}
        {/if}
      </div>

      <div class="actions">
        <label class="opt">
          <input type="checkbox" bind:checked={includeData} />
          <span>{i18n.t('Also pull external data (OBB / media) where accessible — no root')}</span>
        </label>
        <div class="act-row">
          <span class="act-label">{i18n.t('Batch (checked apps)')}</span>
          <div class="btn-row">
            <button class="btn outline" onclick={() => backupApksBatch(Array.from(selected))} disabled={backupBusy || selected.size === 0}>
              {i18n.t('Back up selected APKs ({{n}})', { n: String(selected.size) })}
            </button>
            <button class="btn outline" onclick={() => backupApksBatch(packages.map((p) => p.name.toString()))} disabled={backupBusy || packages.length === 0}>
              {i18n.t('All user APKs ({{n}})', { n: String(packages.length) })}
            </button>
          </div>
        </div>
        <div class="act-row">
          <span class="act-label">{i18n.t('Highlighted app')}</span>
          <div class="btn-row">
            <button class="btn outline" onclick={runApkBackup} disabled={backupBusy || !selectedPkg} title={i18n.t('Saves the APK(s) only — works for any app, no root')}>
              {i18n.t('APK only')}
            </button>
            <button class="primary" onclick={runBackup} disabled={backupBusy || !selectedPkg} title={i18n.t('APK + data as encrypted .ab — only apps that allow backup')}>
              {backupBusy ? i18n.t('Backing up…') : i18n.t('Full .ab (APK + data)')}
            </button>
          </div>
        </div>
        <div class="act-row">
          <span class="act-label">{i18n.t('Whole phone content')}</span>
          <div class="btn-row">
            <button class="btn outline" onclick={runSdcardBackup} disabled={backupBusy} title={i18n.t('Pulls all photos, videos, documents and media from /sdcard — no root')}>
              {i18n.t('Back up internal storage (/sdcard)')}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Right column: restore + legend -->
    <div class="side">
      <div class="card">
        <h3>{i18n.t('Restore a backup')}</h3>
        <p class="muted small">{i18n.t('Pick a .ab file from your PC and confirm the restore on the phone. If the archive is encrypted, the phone asks for the password.')}</p>
        <div style="margin-top: 1rem;">
          <button class="btn outline" onclick={runRestore} disabled={restoreBusy}>
            {restoreBusy ? i18n.t('Restoring…') : i18n.t('Choose .ab file & restore')}
          </button>
        </div>
        <ul class="tips">
          <li>{i18n.t('The app is reinstalled from the APK inside the archive if not present.')}</li>
          <li>{i18n.t('Data and granted permissions are restored with it.')}</li>
          <li>{i18n.t('Keep the phone unlocked and screen on during the transfer.')}</li>
        </ul>
      </div>

      <div class="card legend">
        <h4>{i18n.t('What each level captures')}</h4>
        <ul class="legend-list">
          <li><strong>APK</strong> — {i18n.t('always works, no root. Saves the app’s installable APK(s). Best for reinstalling an app; does not include its data.')}</li>
          <li><strong>.ab</strong> — {i18n.t('APK + data, encrypted with the password you set in the on-device prompt. Only works for apps that allow backup (many banking/chat apps opt out).')}</li>
          <li><strong>{i18n.t('External data')}</strong> — {i18n.t('OBB / expansion files and accessible /sdcard/Android data, pulled alongside the APK. Captures game data and media without root.')}</li>
        </ul>
        <p class="muted small">{i18n.t('adb backup was deprecated in Android 12, so app private data is limited unless the app allows it or you use root/Shizuku on the phone.')}</p>
      </div>
    </div>
  </div>
{/if}

<style>
  .grid.two { display: grid; grid-template-columns: 1.35fr 1fr; gap: 1rem; align-items: start; }
  @media (max-width: 900px) { .grid.two { grid-template-columns: 1fr; } }
  .side { display: flex; flex-direction: column; gap: 1rem; }
  .actions { margin-top: 0.9rem; display: flex; flex-direction: column; gap: 0.9rem; }
  .act-label { display: block; font-size: 10px; text-transform: uppercase; letter-spacing: 0.1em; color: var(--fg-3); font-weight: 700; margin-bottom: 0.4rem; }
  .btn-row { display: flex; gap: 0.5rem; }
  .btn-row button { flex: 1; min-width: 0; }
  .legend h4 { margin-bottom: 0.5rem; }
  .legend-list { margin: 0.4rem 0 0.6rem; padding-left: 1.1rem; }
  .legend-list li { margin: 0.3rem 0; color: var(--fg-2); font-size: var(--font-size-sm); }
  .legend-list strong { color: var(--fg-1); }
  .filter { margin-top: 0.85rem; }
  .applist { margin-top: 0.6rem; max-height: 320px; overflow-y: auto; border: 1px solid var(--border); border-radius: var(--radius); }
  .app-item { display: flex; align-items: center; width: 100%; text-align: left; padding: 0.5rem 0.75rem; background: transparent; border: none; border-bottom: 1px solid var(--border); border-radius: 0; cursor: pointer; }
  .app-item:hover { background: var(--bg-2); }
  .app-item { gap: 0.55rem; }
  .app-item.selected { background: var(--accent-soft); }
  .app-item.active { box-shadow: inset 2px 0 0 var(--accent); }
  .chk { width: 16px; height: 16px; flex-shrink: 0; border: 1px solid var(--border-strong); border-radius: 4px; display: inline-flex; align-items: center; justify-content: center; color: var(--on-accent); }
  .chk.on { background: var(--accent); border-color: var(--accent); }
  .sel-bar { display: flex; align-items: center; justify-content: space-between; margin-top: 0.55rem; }
  .sel-actions { display: flex; gap: 0.4rem; }
  .mini { padding: 0.25rem 0.6rem; font-size: var(--font-size-xs); background: var(--bg-2); border: 1px solid var(--hairline); border-radius: 7px; color: var(--fg-1); cursor: pointer; }
  .mini:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
  .opt { display: flex; align-items: center; gap: 0.5rem; font-size: var(--font-size-sm); color: var(--fg-1); cursor: pointer; }
  .opt input { width: auto; }
  .tips { margin: 1rem 0 0; padding-left: 1.1rem; color: var(--fg-2); font-size: 12.5px; }
  .tips li { margin: 0.25rem 0; }
  .success { padding: 0.65rem 1rem; background: rgba(16, 185, 129, 0.1); border-left: 3px solid var(--good); border-radius: var(--radius); color: var(--good); margin-bottom: 1rem; }
  .error { padding: 0.65rem 1rem; background: rgba(239, 68, 68, 0.1); border-left: 3px solid var(--bad); border-radius: var(--radius); color: var(--bad); margin-bottom: 1rem; }
</style>
