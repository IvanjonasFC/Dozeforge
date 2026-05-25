<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import type { SystemTweaks } from '$types';
  import { deviceStore } from '$stores/device.svelte';
  import { cache, TTL } from '$stores/cache.svelte';
  import { formatTimestamp } from '$utils/format';
  import type { ActionLogEntry, OptimizationAction } from '$types';

  let tweaks: SystemTweaks | null = $state(null);

  async function loadTweaks() {
    if (!deviceStore.selected) return;
    try { tweaks = await cache.getOrFetch('tweaks:' + deviceStore.selected.serial, TTL.medium, () => api.getSystemTweaks(deviceStore.selected!.serial)); }
    catch (e) { /* silent */ }
  }

  onMount(() => { if (deviceStore.selected?.state === 'device') loadTweaks(); });

  let exportPath = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let log = $state<ActionLogEntry[]>([]);

  async function exportShellScript() {
    if (!deviceStore.selected) return;
    busy = true; error = null; exportPath = null;
    try {
      const actions: OptimizationAction[] = [];
      const label = deviceStore.selected.model ?? deviceStore.selected.serial;
      exportPath = await api.exportShellScript(actions, label);
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  async function exportNativeProfile() {
    if (!deviceStore.selected) return;
    busy = true; error = null; exportPath = null;
    try {
      const profile = await api.exportNativeProfile(deviceStore.selected.serial);
      const { save } = await import('@tauri-apps/plugin-dialog');
      const { writeTextFile } = await import('@tauri-apps/plugin-fs');
      const savePath = await save({ defaultPath: 'profile.dozeprofile', filters: [{ name: 'Doze Profile', extensions: ['dozeprofile'] }] });
      if (savePath) {
        await writeTextFile(savePath, JSON.stringify(profile, null, 2));
        exportPath = savePath;
      }
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  async function importNativeProfile() {
    if (!deviceStore.selected) return;
    busy = true; error = null; exportPath = null;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const { readTextFile } = await import('@tauri-apps/plugin-fs');
      const selected = await open({ filters: [{ name: 'Doze Profile', extensions: ['dozeprofile'] }] });
      if (selected && !Array.isArray(selected)) {
        const content = await readTextFile(selected);
        const profile = JSON.parse(content);
        if (!confirm(`Import profile with ${profile.disabled_packages?.length || 0} disabled apps?`)) return;
        await api.importNativeProfile(deviceStore.selected.serial, profile);
        alert('Profile imported successfully!');
      }
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  async function refreshLog() {
    try {
      log = await api.readActionLog(100);
    } catch (e) {
      error = (e as DozeForgeError).message;
    }
  }

  onMount(() => { refreshLog(); });
</script>

<header class="page-head">
  <div>
    <h1>Automation</h1>
    <p class="muted">
      Make optimizations persistent and portable. Export to Termux/Shizuku scripts
      so your rules survive factory resets.
    </p>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected.</p></div>
{:else}
  <div class="grid two-grid">
    <div class="card">
      <h3>Export profile as shell script</h3>
      <p class="muted">
        Generates a SHA-256 verified <code>.sh</code> runnable under Termux + Shizuku
        or any host with ADB access. The script self-verifies before executing.
      </p>
      <button class="primary" onclick={exportShellScript} disabled={busy} style="margin-top: 0.85rem;">
        {busy ? 'Exporting.' : 'Export to Shell Script'}
      </button>
      <button class="outline" onclick={exportNativeProfile} disabled={busy} style="margin-top: 0.85rem; margin-left: 0.5rem;">
        Export Native Profile (.dozeprofile)
      </button>
      <button class="outline" onclick={importNativeProfile} disabled={busy} style="margin-top: 0.85rem; margin-left: 0.5rem;">
        Import Native Profile
      </button>
      {#if exportPath}
        <p class="export-result">
          Saved to <code class="mono">{exportPath}</code>
        </p>
      {/if}
    </div>
  </div>

  {#if error}<div class="error" style="margin-top: 1rem;">{error}</div>{/if}

  <!-- Action log -->
  <div class="card" style="margin-top: 1.5rem;">
    <div class="row" style="justify-content: space-between; align-items: flex-end;">
      <div>
        <h3 style="margin: 0;">Action history</h3>
        <p class="muted footnote">Persistent log of every action applied by DozeForge.</p>
      </div>
      <button onclick={refreshLog}>Refresh</button>
    </div>
    {#if log.length === 0}
      <p class="muted" style="margin-top: 1rem;">No actions logged yet.</p>
    {:else}
      <div class="scroll-y" style="max-height: 50vh; margin-top: 0.85rem;">
        <table>
          <thead>
            <tr>
              <th>When</th>
              <th>Device</th>
              <th>Action</th>
              <th>Result</th>
            </tr>
          </thead>
          <tbody>
            {#each log as entry, i (i)}
              <tr>
                <td>{formatTimestamp(entry.ts)}</td>
                <td class="mono small">{entry.device_serial}</td>
                <td class="mono">
                  {entry.action.kind}{('package' in entry.action) ? ' Â· ' + entry.action.package : ''}
                </td>
                <td>
                  <span class="badge" class:ok={entry.success} class:critical={!entry.success}>
                    {entry.success ? 'OK' : 'FAIL'}
                  </span>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <div class="card flat about">
    <h4>About</h4>
    <p class="muted">
      <strong>DozeForge</strong> v0.5.0 — Android 12+ ADB power auditor.
      Telemetry: opt-in, local-only. Set <code>DOZEFORGE_NO_LOG=1</code> to disable file logging.
    </p>
  </div>
{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; }

  .two-grid { grid-template-columns: 1fr 1fr; }
  @media (max-width: 980px) { .two-grid { grid-template-columns: 1fr; } }

  .footnote { font-size: var(--font-size-xs); margin: 0.25rem 0 0 0; }
  .small { font-size: var(--font-size-xs); }

  .export-result {
    margin-top: 1rem;
    padding: 0.65rem 0.85rem;
    background: var(--good-soft);
    border-left: 3px solid var(--good);
    border-radius: var(--radius);
    font-size: var(--font-size-sm);
  }
  .export-result code { display: inline-block; word-break: break-all; }

  .card.flat { background: transparent; border: 1px solid var(--border); }
  .about { margin-top: 2rem; padding: 1rem 1.25rem; }
  .about h4 { margin: 0 0 0.35rem 0; color: var(--fg-2); }
</style>
