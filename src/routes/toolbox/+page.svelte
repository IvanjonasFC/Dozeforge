<script lang="ts">
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';

  let actionLoading = $state(false);
  let actionSuccess = $state<string | null>(null);
  let actionError = $state<string | null>(null);

  // APK tool options
  let downgradeOpt = $state(false);
  let keepDataOpt = $state(true);
  let extractPkg = $state('');

  const ready = $derived(deviceStore.selected?.state === 'device');

  async function rebootDevice(mode: string) {
    if (!deviceStore.selected) return;
    actionLoading = true; actionSuccess = null; actionError = null;
    try {
      await api.rebootDevice(deviceStore.selected.serial, mode);
      actionSuccess = i18n.t('Rebooting to {{mode}}…', { mode });
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  async function resetDisplay() {
    if (!deviceStore.selected) return;
    actionLoading = true; actionSuccess = null; actionError = null;
    try {
      await api.resetDisplay(deviceStore.selected.serial);
      actionSuccess = i18n.t('Display metrics restored to factory defaults.');
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  async function launchScrcpy() {
    if (!deviceStore.selected) return;
    actionError = null; actionSuccess = null;
    try { await api.launchScrcpy(deviceStore.selected.serial); }
    catch (e) { actionError = (e as DozeForgeError).message; }
  }

  async function takeScreenshotToPc() {
    if (!deviceStore.selected) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const savePath = await save({ filters: [{ name: 'PNG Image', extensions: ['png'] }] });
      if (!savePath) return;
      actionLoading = true; actionSuccess = null; actionError = null;
      await api.captureScreenshot(deviceStore.selected.serial, savePath);
      actionSuccess = i18n.t('Screenshot saved to {{path}}', { path: savePath });
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  // F11 — immersive mode / hide system bars.
  async function immersive(mode: 'full' | 'status' | 'navigation' | 'off') {
    if (!deviceStore.selected) return;
    actionLoading = true; actionError = null; actionSuccess = null;
    try {
      await api.setImmersiveMode(deviceStore.selected.serial, mode);
      actionSuccess = mode === 'off' ? i18n.t('Immersive mode disabled.') : i18n.t('Immersive mode applied.');
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  // F10 — install a single APK with options.
  async function installApkAdvanced() {
    if (!deviceStore.selected) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const sel = await open({ filters: [{ name: 'APK', extensions: ['apk'] }], multiple: false });
      if (!sel || Array.isArray(sel)) return;
      actionLoading = true; actionSuccess = null; actionError = null;
      const res = await api.installApk(deviceStore.selected.serial, sel as string, downgradeOpt, keepDataOpt);
      actionSuccess = `${i18n.t('Installed:')} ${res}`;
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  // F10 — install split APKs (base + config splits).
  async function installSplitApks() {
    if (!deviceStore.selected) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const sel = await open({ filters: [{ name: 'APK', extensions: ['apk'] }], multiple: true });
      if (!sel || !Array.isArray(sel) || sel.length === 0) return;
      actionLoading = true; actionSuccess = null; actionError = null;
      const res = await api.installApksMultiple(deviceStore.selected.serial, sel as string[], downgradeOpt, keepDataOpt);
      actionSuccess = `${i18n.t('Installed')} ${sel.length} ${i18n.t('APKs.')} ${res}`;
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  // F10 — extract all APKs (base + splits) of a package to a .zip.
  async function extractApks() {
    if (!deviceStore.selected || !extractPkg.trim()) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const path = await save({ defaultPath: `${extractPkg.trim()}.zip`, filters: [{ name: 'APK bundle', extensions: ['zip', 'apk'] }] });
      if (!path) return;
      actionLoading = true; actionSuccess = null; actionError = null;
      const res = await api.extractApk(deviceStore.selected.serial, extractPkg.trim(), path);
      actionSuccess = res;
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  async function sideloadApk() {
    if (!deviceStore.selected) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const sel = await open({ filters: [{ name: 'APK', extensions: ['apk'] }], multiple: false });
      if (!sel || Array.isArray(sel)) return;
      actionLoading = true; actionSuccess = null; actionError = null;
      const res = await api.sideloadApk(deviceStore.selected.serial, sel as string);
      actionSuccess = `${i18n.t('Installed:')} ${res}`;
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Toolbox')}</h1>
    <p class="muted">{i18n.t('Power-user device actions: reboot, mirroring, APK install/extract, immersive mode.')}</p>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else if !ready}
  <div class="card empty"><p class="muted">{i18n.t('Device is offline or unauthorized.')}</p></div>
{:else}
  <div class="tb-grid">
    <div class="card">
      <h3>{i18n.t('Power State')}</h3>
      <div class="stack">
        <button class="btn outline" onclick={() => rebootDevice('system')} disabled={actionLoading}>{i18n.t('Reboot System')}</button>
        <button class="btn outline warn" onclick={() => rebootDevice('recovery')} disabled={actionLoading}>{i18n.t('Reboot Recovery')}</button>
        <button class="btn outline warn" onclick={() => rebootDevice('bootloader')} disabled={actionLoading}>{i18n.t('Reboot Fastboot')}</button>
      </div>
    </div>

    <div class="card">
      <h3>{i18n.t('Utilities')}</h3>
      <div class="stack">
        <button class="primary" onclick={launchScrcpy}>{i18n.t('Screen Mirror (Scrcpy)')}</button>
        <button class="btn outline" onclick={takeScreenshotToPc} disabled={actionLoading}>{i18n.t('Screenshot to PC')}</button>
        <button class="btn outline" onclick={sideloadApk} disabled={actionLoading}>{i18n.t('Sideload APK')}</button>
      </div>
    </div>

    <div class="card">
      <h3>{i18n.t('Immersive Mode')}</h3>
      <p class="muted small">{i18n.t('Hide the status and/or navigation bars system-wide (policy_control).')}</p>
      <div class="stack" style="margin-top: 0.85rem;">
        <button class="btn outline" onclick={() => immersive('full')} disabled={actionLoading}>{i18n.t('Hide both bars')}</button>
        <button class="btn outline" onclick={() => immersive('status')} disabled={actionLoading}>{i18n.t('Hide status bar')}</button>
        <button class="btn outline" onclick={() => immersive('navigation')} disabled={actionLoading}>{i18n.t('Hide nav bar')}</button>
        <button class="primary" onclick={() => immersive('off')} disabled={actionLoading}>{i18n.t('Restore bars')}</button>
      </div>
    </div>

    <div class="card">
      <h3>{i18n.t('APK Tools')}</h3>
      <div class="opts">
        <label><input type="checkbox" bind:checked={downgradeOpt} /> <span>{i18n.t('Allow downgrade (-d)')}</span></label>
        <label><input type="checkbox" bind:checked={keepDataOpt} /> <span>{i18n.t('Keep app data (-r)')}</span></label>
      </div>
      <div class="stack" style="margin-top: 0.85rem;">
        <button class="btn outline" onclick={installApkAdvanced} disabled={actionLoading}>{i18n.t('Install APK…')}</button>
        <button class="btn outline" onclick={installSplitApks} disabled={actionLoading}>{i18n.t('Install split APKs…')}</button>
      </div>
      <div class="extract-row">
        <input bind:value={extractPkg} placeholder="com.package.name" spellcheck="false" autocomplete="off" />
        <button class="btn outline" onclick={extractApks} disabled={actionLoading || !extractPkg.trim()}>{i18n.t('Extract all APKs…')}</button>
      </div>
    </div>

    <div class="card">
      <h3>{i18n.t('Dangerous Fixes')}</h3>
      <div class="stack">
        <button class="btn outline error" onclick={resetDisplay} disabled={actionLoading}>{i18n.t('Reset Display Size/DPI')}</button>
      </div>
    </div>
  </div>

  {#if actionSuccess}<div class="success" style="margin-top: 1rem;">{actionSuccess}</div>{/if}
  {#if actionError}<div class="error" style="margin-top: 1rem;">{actionError}</div>{/if}
{/if}

<style>
  .tb-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1rem; align-items: start; max-width: 980px; }
  @media (max-width: 760px) { .tb-grid { grid-template-columns: 1fr; } }
  .stack { display: flex; flex-direction: column; gap: 0.5rem; margin-top: 1rem; }
  .stack button { width: 100%; text-align: left; }
  .opts { display: flex; flex-direction: column; gap: 0.5rem; margin-top: 0.75rem; }
  .opts label { display: flex; align-items: center; gap: 0.5rem; color: var(--fg-1); font-size: 13.5px; cursor: pointer; }
  .opts label input { width: auto; margin: 0; }
  .extract-row { display: flex; gap: 0.5rem; margin-top: 0.75rem; }
  .extract-row input { flex: 1; }
  .success { padding: 0.65rem 1rem; background: rgba(16, 185, 129, 0.1); border-left: 3px solid var(--good); border-radius: var(--radius); color: var(--good); }
  .error { padding: 0.65rem 1rem; background: rgba(239, 68, 68, 0.1); border-left: 3px solid var(--bad); border-radius: var(--radius); color: var(--bad); }
</style>
