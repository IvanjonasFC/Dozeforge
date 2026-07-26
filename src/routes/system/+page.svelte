<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import Skeleton from '$components/Skeleton.svelte';

  let activeTab = $state<'props' | 'io'>('props');

  // IO
  let ioStats = $state<any[]>([]);
  let ioLoading = $state(false);
  let ioError = $state<string | null>(null);

  // Props
  let props = $state<Record<string, string> | null>(null);
  let propsError = $state<string | null>(null);
  let propsLoading = $state(false);
  let propsFilter = $state('');

  async function loadIoStats() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    ioLoading = true; ioError = null;
    try {
      ioStats = await api.getIoStats(deviceStore.selected.serial);
    } catch(e) {
      ioError = (e as DozeForgeError).message;
    } finally {
      ioLoading = false;
    }
  }

  async function loadProps() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    propsLoading = true;
    try {
      props = await api.getSystemProperties(deviceStore.selected.serial);
    } catch (e) {
      propsError = (e as DozeForgeError).message;
    } finally {
      propsLoading = false;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') loadProps();
  });

  $effect(() => {
    if (deviceStore.selected?.state === 'device' && activeTab === 'props') {
      if (!props && !propsLoading) loadProps();
    }
    if (deviceStore.selected?.state === 'device' && activeTab === 'io' && ioStats.length === 0 && !ioLoading) {
      loadIoStats();
    }
  });

  const visibleProps = $derived.by(() => {
    if (!props) return [];
    if (!propsFilter) return Object.entries(props);
    const q = propsFilter.toLowerCase();
    return Object.entries(props).filter(([k, v]) =>
      k.toLowerCase().includes(q) || v.toLowerCase().includes(q)
    );
  });

  // Escape HTML so device-controlled getprop keys/values can never inject
  // markup through the {@html} sink below (defense-in-depth against XSS).
  function esc(s: string) {
    return s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function highlight(text: string, search: string) {
    if (!search) return esc(text);
    const s = search.toLowerCase();
    const t = text.toLowerCase();
    const idx = t.indexOf(s);
    if (idx === -1) return esc(text);
    const before = esc(text.slice(0, idx));
    const match = esc(text.slice(idx, idx + search.length));
    const after = esc(text.slice(idx + search.length));
    return `${before}<mark>${match}</mark>${after}`;
  }
  
  async function copyProp(k: string, v: string) {
    try {
      await navigator.clipboard.writeText(`${k}=${v}`);
    } catch {}
  }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('System Tweaks')}</h1>
    <p class="muted">{i18n.t('Build properties and UFS storage degradation monitor.')}</p>
  </div>
</header>

<div class="tabs">
  <button class:active={activeTab === 'props'} onclick={() => activeTab = 'props'}>{i18n.t('Build Props')}</button>
  <button class:active={activeTab === 'io'} onclick={() => activeTab = 'io'}>{i18n.t('Storage I/O')}</button>
</div>

<div class="tab-content">
  {#if !deviceStore.selected}
    <div class="card p-card"><p class="muted">{i18n.t('No device connected.')}</p></div>
  {:else if activeTab === 'io'}
    <div class="card p-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
        <div>
          <h3>{i18n.t('UFS Storage Degradation Monitor')}</h3>
          <p class="muted small">{i18n.t('Shows cumulative read/write bytes per package (requires Root Mode).')}</p>
        </div>
        <button class="btn outline small" onclick={loadIoStats} disabled={ioLoading}>{i18n.t('Refresh')}</button>
      </div>
      {#if ioLoading && ioStats.length === 0}
        <Skeleton lines={5} />
      {:else if ioError}
        <div class="error">{ioError}</div>
      {:else if ioStats.length > 0}
        <div class="table-container" style="max-height: 60vh; overflow-y: auto;">
          <table class="data-table">
            <thead style="position: sticky; top: 0; background: var(--bg-1);">
              <tr>
                <th>{i18n.t('UID')}</th>
                <th>{i18n.t('Foreground Read')}</th>
                <th>{i18n.t('Foreground Write')}</th>
                <th>{i18n.t('Background Read')}</th>
                <th>{i18n.t('Background Write')}</th>
              </tr>
            </thead>
            <tbody>
              {#each ioStats.sort((a,b) => (b.bg_write_bytes + b.fg_write_bytes) - (a.bg_write_bytes + a.fg_write_bytes)).slice(0, 50) as stat}
                <tr>
                  <td class="mono">{stat.uid}</td>
                  <td class="mono">{(stat.fg_read_bytes / 1024 / 1024).toFixed(2)} MB</td>
                  <td class="mono">{(stat.fg_write_bytes / 1024 / 1024).toFixed(2)} MB</td>
                  <td class="mono">{(stat.bg_read_bytes / 1024 / 1024).toFixed(2)} MB</td>
                  <td class="mono" style="color: var(--warn);">{(stat.bg_write_bytes / 1024 / 1024).toFixed(2)} MB</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <div class="card empty">
          <p class="muted">{i18n.t('No I/O data found or Permission Denied.')}</p>
          <p class="muted small">{i18n.t('This feature requires')} <strong>{i18n.t('Root access (Magisk/KernelSU)')}</strong> {i18n.t('and a compatible kernel to read')} <code>/proc/uid_io/stats</code>.</p>
        </div>
      {/if}
    </div>
  {:else if activeTab === 'props'}
    <div class="card p-card">
      <div style="display: flex; gap: 1rem; margin-bottom: 1rem; align-items: center;">
        <input type="search" placeholder={i18n.t('Filter props...')} bind:value={propsFilter} style="flex: 1; max-width: 300px;" />
        <span class="muted small">{visibleProps.length} {i18n.t('props found')}</span>
      </div>
      {#if propsLoading && !props}
        <Skeleton lines={10} />
      {:else if propsError}
        <div class="error">{propsError}</div>
      {:else}
        <div class="table-container" style="max-height: 65vh; overflow-y: auto;">
          <table class="data-table prop-table">
            <thead style="position: sticky; top: 0; background: var(--bg-1); z-index: 10;">
              <tr><th>{i18n.t('Key')}</th><th>{i18n.t('Value')}</th></tr>
            </thead>
            <tbody>
              {#each visibleProps as [k, v]}
                <tr onclick={() => copyProp(k, v)} title="Click to copy">
                  <td class="mono prop-key">{@html highlight(k, propsFilter)}</td>
                  <td class="mono prop-val">{@html highlight(v, propsFilter)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 540px; }
  
  .tabs { display: flex; gap: 0.5rem; margin-bottom: 1.5rem; border-bottom: 1px solid var(--border); padding-bottom: 0px; }
  .tabs button { background: transparent; border: none; padding: 0.5rem 1rem; color: var(--fg-2); border-bottom: 2px solid transparent; font-weight: 500; cursor: pointer; border-radius: 0; }
  .tabs button:hover { color: var(--fg-0); }
  .tabs button.active { color: var(--accent); border-bottom-color: var(--accent); }
  
  .p-card { min-height: 400px; padding: 0.5rem; }
  
  .prop-table { font-size: 11.5px; }
  .prop-table tr:hover { background: var(--bg-hover); cursor: copy; }
  .prop-key { color: var(--fg-1); }
  .prop-val { color: var(--good); max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  :global(mark) { background: rgba(255, 107, 0, 0.3); color: inherit; padding: 0 2px; border-radius: 2px; }
  
  .error { padding: 0.65rem 1rem; background: rgba(239, 68, 68, 0.1); border-left: 3px solid var(--danger); border-radius: var(--radius); color: var(--danger); }
</style>
