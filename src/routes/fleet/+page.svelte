<script lang="ts">
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import type { Device, Profile } from '$types';

  type RowResult = { serial: string; ok: boolean; message: string };

  let selected = $state<Set<string>>(new Set());
  let profile = $state<Profile>('balanced');
  let shellCmd = $state('');
  let busy = $state(false);
  let action = $state<string | null>(null);
  let results = $state<RowResult[]>([]);

  const devices = $derived(deviceStore.devices.filter((d) => d.state === 'device'));

  function label(d: Device): string {
    if (d.model) return `${d.manufacturer ? d.manufacturer + ' ' : ''}${d.model}`;
    return d.product ?? d.serial;
  }
  function toggle(serial: string) {
    const n = new Set(selected);
    if (n.has(serial)) n.delete(serial); else n.add(serial);
    selected = n;
  }
  function selectAll() { selected = new Set(devices.map((d) => d.serial)); }
  function selectNone() { selected = new Set(); }

  async function runBulk(name: string, fn: (serial: string) => Promise<unknown>) {
    const targets = [...selected];
    if (targets.length === 0) return;
    busy = true; action = name; results = [];
    const out: RowResult[] = [];
    // Sequential to avoid hammering adb; each device is independent.
    for (const serial of targets) {
      try {
        await fn(serial);
        out.push({ serial, ok: true, message: i18n.t('OK') });
      } catch (e) {
        out.push({ serial, ok: false, message: (e as DozeForgeError).message });
      }
      results = [...out];
    }
    busy = false; action = null;
  }

  function applyProfileAll() {
    if (!confirm(i18n.t('Apply the {{name}} profile to {{n}} device(s)? A snapshot is saved on each first.', { name: profile, n: selected.size }))) return;
    runBulk('profile', (serial) => api.applyProfile(serial, profile));
  }
  function rebootAll() {
    if (!confirm(i18n.t('Reboot {{n}} device(s)?', { n: selected.size }))) return;
    runBulk('reboot', (serial) => api.rebootDevice(serial, 'system'));
  }
  function runShellAll() {
    if (!shellCmd.trim()) return;
    runBulk('shell', (serial) => api.runShell(serial, shellCmd.trim()));
  }

  function shortSerial(s: string): string { return s.length > 22 ? s.slice(0, 20) + '…' : s; }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Fleet')}</h1>
    <p class="muted">{i18n.t('Apply the same action to several connected devices at once — profiles, reboots or a shell command.')}</p>
  </div>
</header>

{#if devices.length === 0}
  <div class="card empty"><p class="muted">{i18n.t('No devices connected. Connect one or more phones (USB or wireless) and refresh.')}</p></div>
{:else}
  <div class="card">
    <div class="fleet-head">
      <span class="mono">{selected.size} / {devices.length} {i18n.t('selected')}</span>
      <div style="display:flex; gap:0.4rem;">
        <button class="btn outline small" onclick={selectAll}>{i18n.t('All')}</button>
        <button class="btn outline small" onclick={selectNone}>{i18n.t('None')}</button>
        <button class="btn outline small" onclick={() => deviceStore.refresh()}>{i18n.t('Refresh')}</button>
      </div>
    </div>
    <div class="dev-list">
      {#each devices as d (d.serial)}
        <label class="dev">
          <input type="checkbox" checked={selected.has(d.serial)} onchange={() => toggle(d.serial)} />
          <span class="dot"></span>
          <span class="dev-name">{label(d)}</span>
          <span class="dev-serial mono">{shortSerial(d.serial)}</span>
        </label>
      {/each}
    </div>
  </div>

  <h3 class="sec">{i18n.t('Bulk action')}</h3>
  <div class="grid actions">
    <div class="card">
      <h4>{i18n.t('Apply optimization profile')}</h4>
      <select bind:value={profile} style="margin-top:0.6rem;">
        <option value="conservative">{i18n.t('Conservative')}</option>
        <option value="balanced">{i18n.t('Balanced')}</option>
        <option value="aggressive">{i18n.t('Aggressive')}</option>
        <option value="nuclear">{i18n.t('Nuclear')}</option>
      </select>
      <button class="primary" style="margin-top:0.85rem; width:100%;" onclick={applyProfileAll} disabled={busy || selected.size === 0}>
        {busy && action === 'profile' ? i18n.t('Applying…') : i18n.t('Apply to selected')}
      </button>
    </div>

    <div class="card">
      <h4>{i18n.t('Power')}</h4>
      <p class="muted small">{i18n.t('Reboot every selected device.')}</p>
      <button class="btn outline warn" style="margin-top:0.85rem; width:100%;" onclick={rebootAll} disabled={busy || selected.size === 0}>
        {busy && action === 'reboot' ? i18n.t('Rebooting…') : i18n.t('Reboot selected')}
      </button>
    </div>

    <div class="card">
      <h4>{i18n.t('Shell command')}</h4>
      <input bind:value={shellCmd} placeholder="settings put global …" spellcheck="false" autocomplete="off" style="margin-top:0.6rem;" />
      <button class="btn outline" style="margin-top:0.85rem; width:100%;" onclick={runShellAll} disabled={busy || selected.size === 0 || !shellCmd.trim()}>
        {busy && action === 'shell' ? i18n.t('Running…') : i18n.t('Run on selected')}
      </button>
    </div>
  </div>

  {#if results.length > 0}
    <h3 class="sec">{i18n.t('Results')}</h3>
    <div class="card res-card">
      <table>
        <thead><tr><th>{i18n.t('Device')}</th><th>{i18n.t('Result')}</th></tr></thead>
        <tbody>
          {#each results as r (r.serial)}
            <tr>
              <td class="mono">{shortSerial(r.serial)}</td>
              <td>
                <span class="badge {r.ok ? 'ok' : 'critical'}">{r.ok ? i18n.t('OK') : i18n.t('Failed')}</span>
                {#if !r.ok}<span class="muted small" style="margin-left:0.5rem;">{r.message}</span>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
{/if}

<style>
  .sec { margin: 1.75rem 0 0.85rem; }
  .fleet-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; color: var(--fg-2); font-size: 12.5px; }
  .dev-list { border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; }
  .dev { display: flex; align-items: center; gap: 0.6rem; padding: 0.6rem 0.85rem; border-bottom: 1px solid var(--border); cursor: pointer; }
  .dev:last-child { border-bottom: none; }
  .dev:hover { background: var(--bg-2); }
  .dev input { width: auto; margin: 0; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--good); box-shadow: 0 0 6px var(--good); flex-shrink: 0; }
  .dev-name { font-weight: 500; }
  .dev-serial { margin-left: auto; color: var(--fg-3); font-size: 12px; }
  .grid.actions { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 1rem; align-items: start; }
  .grid.actions h4 { margin: 0; }
  .res-card { padding: 0.5rem 0.75rem; }
</style>
