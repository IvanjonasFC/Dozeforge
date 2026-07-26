<script lang="ts">
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import AppName from './AppName.svelte';
  import type { BloatPresetDto, BloatPreset } from '$types';

  let { open = $bindable(false), onApplied }: { open?: boolean; onApplied?: () => void } = $props();

  let presets = $state<BloatPresetDto[]>([]);
  let previewPkgs = $state<string[]>([]);
  let checked = $state<Set<string>>(new Set());
  let step = $state<'choose' | 'review' | 'done'>('choose');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let result = $state<{ disabled: number; failed: number } | null>(null);

  $effect(() => {
    if (open && presets.length === 0) loadPresets();
    if (!open) reset();
  });

  function reset() { previewPkgs = []; checked = new Set(); step = 'choose'; result = null; error = null; }

  async function loadPresets() {
    try { presets = await api.listBloatPresets(); }
    catch (e) { error = (e as DozeForgeError).message; }
  }

  async function choosePreset(id: BloatPreset) {
    if (!deviceStore.selected) return;
    busy = true; error = null;
    try {
      previewPkgs = await api.previewBloatPreset(deviceStore.selected.serial, id);
      checked = new Set(previewPkgs);
      step = 'review';
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  function toggle(pkg: string) {
    const n = new Set(checked);
    if (n.has(pkg)) n.delete(pkg); else n.add(pkg);
    checked = n;
  }

  async function apply() {
    if (!deviceStore.selected || checked.size === 0) return;
    busy = true; error = null;
    try {
      const rep = await api.disableBloatware(deviceStore.selected.serial, [...checked]);
      result = { disabled: rep.disabled.length, failed: rep.failed.length };
      step = 'done';
      onApplied?.();
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { busy = false; }
  }
</script>

{#if open}
  <div class="wiz-backdrop" onclick={() => open = false} role="presentation">
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="wiz" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="wiz-head">
        <h3>{i18n.t('Debloat Wizard')}</h3>
        <button class="close" onclick={() => open = false} aria-label="Close">×</button>
      </div>
      <div class="wiz-body">
        {#if error}<div class="error">{error}</div>{/if}

        {#if step === 'choose'}
          <p class="muted small">{i18n.t('Pick a preset. You review the exact packages before anything is disabled — and everything is reversible (re-enable from App Manager).')}</p>
          <div class="presets">
            {#each presets as p (p.id)}
              <button class="preset" onclick={() => choosePreset(p.id)} disabled={busy}>
                <div class="preset-name">{p.label}</div>
                <div class="preset-desc">{p.description}</div>
              </button>
            {/each}
            {#if presets.length === 0}<p class="muted">{i18n.t('Loading…')}</p>{/if}
          </div>

        {:else if step === 'review'}
          <div class="rev-head">
            <span class="mono">{checked.size} / {previewPkgs.length} {i18n.t('selected')}</span>
            <div style="display:flex; gap:0.4rem;">
              <button class="btn outline small" onclick={() => (checked = new Set(previewPkgs))}>{i18n.t('All')}</button>
              <button class="btn outline small" onclick={() => (checked = new Set())}>{i18n.t('None')}</button>
            </div>
          </div>
          <div class="pkg-list">
            {#each previewPkgs as pkg (pkg)}
              <label class="pkg">
                <input type="checkbox" checked={checked.has(pkg)} onchange={() => toggle(pkg)} />
                <AppName package={pkg} size="sm" />
              </label>
            {/each}
            {#if previewPkgs.length === 0}<p class="muted">{i18n.t('This preset matched no packages on your device.')}</p>{/if}
          </div>
          <div class="wiz-actions">
            <button onclick={() => (step = 'choose')}>{i18n.t('Back')}</button>
            <button class="primary" onclick={apply} disabled={busy || checked.size === 0}>
              {busy ? i18n.t('Disabling…') : `${i18n.t('Disable')} ${checked.size}`}
            </button>
          </div>

        {:else if step === 'done' && result}
          <div class="done">
            <span class="badge ok" style="font-size:14px; padding:6px 14px;">{result.disabled} {i18n.t('disabled')}</span>
            {#if result.failed > 0}<span class="badge critical" style="font-size:14px; padding:6px 14px;">{result.failed} {i18n.t('failed')}</span>{/if}
            <p class="muted small" style="margin-top:0.85rem;">{i18n.t('Disabled apps can be restored any time from App Manager (Enable).')}</p>
          </div>
          <div class="wiz-actions"><button class="primary" onclick={() => (open = false)}>{i18n.t('Done')}</button></div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .wiz-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.6); backdrop-filter: blur(4px); display: flex; align-items: center; justify-content: center; z-index: 9999; }
  .wiz { background: var(--bg-1); border: 1px solid var(--border); border-radius: var(--radius-lg); width: 520px; max-width: 92vw; max-height: 85vh; display: flex; flex-direction: column; box-shadow: 0 10px 40px rgba(0,0,0,0.5); overflow: hidden; }
  .wiz-head { display: flex; justify-content: space-between; align-items: center; padding: 1.25rem 1.5rem; border-bottom: 1px solid var(--border); background: var(--bg-2); }
  .wiz-head h3 { margin: 0; }
  .close { background: none; border: none; font-size: 1.5rem; color: var(--fg-3); cursor: pointer; line-height: 0.5; padding: 0.4rem; }
  .close:hover { color: var(--fg-0); }
  .wiz-body { padding: 1.25rem 1.5rem; overflow-y: auto; }
  .presets { display: flex; flex-direction: column; gap: 0.6rem; margin-top: 1rem; }
  .preset { display: block; width: 100%; text-align: left; white-space: normal; background: var(--bg-2); border: 1px solid var(--border); border-radius: var(--radius); padding: 0.85rem 1rem; cursor: pointer; }
  .preset:hover:not(:disabled) { border-color: var(--accent); background: var(--bg-3); }
  .preset-name { font-family: var(--font-display); font-weight: 600; color: var(--fg-0); white-space: normal; }
  .preset-desc { color: var(--fg-2); font-size: 12.5px; margin-top: 0.25rem; white-space: normal; line-height: 1.45; }
  .rev-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; color: var(--fg-2); font-size: 12.5px; }
  .pkg-list { max-height: 40vh; overflow-y: auto; border: 1px solid var(--border); border-radius: var(--radius); }
  .pkg { display: flex; align-items: center; gap: 0.6rem; padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border); cursor: pointer; }
  .pkg:hover { background: var(--bg-2); }
  .pkg input { width: auto; margin: 0; }
  .wiz-actions { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1rem; }
  .done { text-align: center; padding: 1rem 0; }
</style>
