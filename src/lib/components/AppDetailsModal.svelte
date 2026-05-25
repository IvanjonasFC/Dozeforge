<script lang="ts">
  import { appModalStore } from '$stores/appModal.svelte';
  import { deviceStore } from '$stores/device.svelte';
  import { api } from '$lib/tauri/api';
  import AppName from './AppName.svelte';
  import { page } from '$app/stores';
  import { save } from '@tauri-apps/plugin-dialog';

  let details = $state<any>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);

  const context = $derived.by(() => {
    if ($page.url.pathname.includes('/bloatware')) return 'bloatware';
    if ($page.url.pathname.includes('/sleep') || $page.url.pathname.includes('/battery')) return 'battery';
    return 'general';
  });

  $effect(() => {
    if (appModalStore.selectedPackage && deviceStore.selected) {
      loadDetails(appModalStore.selectedPackage);
    } else {
      details = null;
      error = null;
      success = null;
    }
  });

  async function loadDetails(pkg: string) {
    loading = true;
    error = null;
    try {
      details = await api.getSingleAppDetails(deviceStore.selected!.serial, pkg, deviceStore.rootMode);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  async function executeAction(action: string) {
    if (!deviceStore.selected || !appModalStore.selectedPackage) return;
    const pkg = appModalStore.selectedPackage;
    loading = true;
    error = null;
    success = null;
    try {
      if (action === 'clear_cache') await api.clearAppCache(deviceStore.selected.serial, [pkg]);
      else if (action === 'force_stop') await api.forceStopPackage(deviceStore.selected.serial, pkg);
      else if (action === 'ignore_wakelocks') await api.setAppOps(deviceStore.selected.serial, pkg, 'WAKE_LOCK', 'ignore');
      else if (action === 'ignore_background') await api.setAppOps(deviceStore.selected.serial, pkg, 'RUN_ANY_IN_BACKGROUND', 'ignore');
      else if (action === 'block_exact_alarms') await api.setAppOps(deviceStore.selected.serial, pkg, 'SCHEDULE_EXACT_ALARM', 'ignore');
      else if (action === 'block_sensors') await api.setAppOps(deviceStore.selected.serial, pkg, 'SENSOR', 'ignore');
      else if (action === 'force_restricted') await api.setStandbyBucket(deviceStore.selected.serial, pkg, 'restricted');
      else if (action === 'clear_data') await api.clearAppData(deviceStore.selected.serial, pkg);
      else if (action === 'uninstall') await api.uninstallPackage(deviceStore.selected.serial, pkg);
      else if (action === 'settings') await api.openAppSettings(deviceStore.selected.serial, pkg);
      else if (action === 'disable') await api.disableBloatware(deviceStore.selected.serial, [pkg]);
      else if (action === 'enable') await api.enableBloatware(deviceStore.selected.serial, [pkg]);
      else if (action === 'extract_apk') {
        const savePath = await save({
          filters: [{ name: 'App Bundle (ZIP)', extensions: ['zip'] }],
          defaultPath: `${pkg}_bundle.zip`
        });
        if (savePath) {
          success = await api.extractApk(deviceStore.selected.serial, pkg, savePath);
          loading = false;
          return;
        }
      }
      else if (action === 'copy_tasker_intent') {
        const script = `# Tasker / Shell script to restrict ${pkg}
cmd appops set ${pkg} WAKE_LOCK ignore
cmd appops set ${pkg} RUN_ANY_IN_BACKGROUND ignore
cmd appops set ${pkg} SCHEDULE_EXACT_ALARM ignore
am set-standby-bucket ${pkg} restricted`;
        await navigator.clipboard.writeText(script);
        success = 'Tasker / Shell script copied to clipboard!';
      }

      if (action === 'settings') {
        success = 'Opened on device';
      } else if (action === 'uninstall') {
        success = 'App uninstalled';
        setTimeout(() => appModalStore.close(), 1500);
      } else if (action !== 'copy_tasker_intent') {
        success = `Action applied successfully`;
        await loadDetails(pkg);
      }
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  function formatBytes(bytes: number | null) {
    if (bytes === null || bytes === 0) return 'Root Required';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }
</script>

{#if appModalStore.selectedPackage}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => appModalStore.close()}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <div style="display: flex; flex-direction: column; gap: 0.25rem;">
          <AppName package={appModalStore.selectedPackage} size="lg" />
          {#if details}
            <div style="display: flex; gap: 0.5rem; align-items: center;">
              {#if details.version_name}
                <span class="muted small mono" style="font-size: 10px;">v{details.version_name}</span>
              {/if}
              {#if details.is_system}
                <span class="badge danger" style="font-size: 9px; padding: 2px 4px;">SYSTEM APP</span>
              {/if}
            </div>
          {/if}
        </div>
        <button class="close-btn" onclick={() => appModalStore.close()}>×</button>
      </div>

      <div class="modal-body">
        {#if success}<div class="success">{success}</div>{/if}
        {#if error}<div class="error">{error}</div>{/if}

        {#if loading && !details}
          <div class="spinner">Loading...</div>
        {:else if details}
          <div class="grid two-cols">
            <div class="card stat-card">
              <span class="muted small">Cache Size</span>
              <strong class="mono">{formatBytes(details.cache_bytes)}</strong>
            </div>
            <div class="card stat-card">
              <span class="muted small">Data Size</span>
              <strong class="mono">{formatBytes(details.data_bytes)}</strong>
            </div>
            <div class="card stat-card">
              <span class="muted small">APK Size</span>
              <strong class="mono">{formatBytes(details.apk_bytes)}</strong>
            </div>
            <div class="card stat-card">
              <span class="muted small">Standby Bucket</span>
              <strong class="mono" style="text-transform: capitalize;">{details.restrictions.standby_bucket}</strong>
            </div>
          </div>

          <div class="restrictions">
            <h4>Current Restrictions</h4>
            <div class="tags">
              {#if details.restrictions.wake_lock_ignored}
                <span class="badge danger">Wakelocks Blocked</span>
              {:else}
                <span class="badge ok">Wakelocks Allowed</span>
              {/if}
              {#if details.restrictions.run_in_background_ignored}
                <span class="badge danger">Background Blocked</span>
              {:else}
                <span class="badge ok">Background Allowed</span>
              {/if}
            </div>
          </div>

          <div class="actions-grid">
            {#if context === 'bloatware'}
              <button class="action-btn danger" onclick={() => executeAction('disable')}>Disable App</button>
              <button class="action-btn" onclick={() => executeAction('enable')}>Enable App</button>
              {#if !details?.is_system}
                <button class="action-btn danger" onclick={() => { if(confirm('Uninstall this app?')) executeAction('uninstall'); }}>Uninstall App</button>
              {:else}
                <button class="action-btn" disabled style="opacity:0.5; cursor:not-allowed;">System App</button>
              {/if}
              <button class="action-btn" onclick={() => executeAction('settings')}>Open on Phone</button>
            {:else if context === 'battery'}
              <button class="action-btn" onclick={() => executeAction('ignore_wakelocks')}>Block Wakelocks</button>
              <button class="action-btn" onclick={() => executeAction('ignore_background')}>Block Background</button>
              <button class="action-btn" onclick={() => executeAction('block_exact_alarms')}>Block Exact Alarms</button>
              <button class="action-btn" onclick={() => executeAction('block_sensors')}>Block Sensors</button>
              <button class="action-btn" onclick={() => executeAction('force_restricted')}>Force Restricted</button>
              <button class="action-btn" onclick={() => executeAction('force_stop')}>Force Stop</button>
            {:else}
              <button class="action-btn" onclick={() => executeAction('clear_cache')}>Clear Cache</button>
              <button class="action-btn danger" onclick={() => { if(confirm('Clear ALL app data?')) executeAction('clear_data'); }}>Clear Data</button>
              <button class="action-btn" onclick={() => executeAction('force_stop')}>Force Stop</button>
              {#if !details?.is_system}
                <button class="action-btn danger" onclick={() => { if(confirm('Uninstall this app?')) executeAction('uninstall'); }}>Uninstall App</button>
              {:else}
                <button class="action-btn" disabled style="opacity:0.5; cursor:not-allowed;">System App</button>
              {/if}
              <button class="action-btn" onclick={() => executeAction('ignore_wakelocks')}>Block Wakelocks</button>
              <button class="action-btn" onclick={() => executeAction('ignore_background')}>Block Background</button>
              <button class="action-btn" onclick={() => executeAction('block_exact_alarms')}>Block Exact Alarms</button>
              <button class="action-btn" onclick={() => executeAction('block_sensors')}>Block Sensors</button>
              <button class="action-btn" onclick={() => executeAction('extract_apk')}>Extract APK to PC</button>
              <button class="action-btn" onclick={() => executeAction('copy_tasker_intent')}>Copy Tasker Script</button>
              <button class="action-btn" onclick={() => executeAction('settings')}>Open on Phone</button>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed; top: 0; left: 0; width: 100vw; height: 100vh;
    background: rgba(0,0,0,0.6);
    backdrop-filter: blur(4px);
    display: flex; align-items: center; justify-content: center;
    z-index: 9999;
  }
  .modal {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    width: 480px; max-width: 90vw;
    box-shadow: 0 10px 40px rgba(0,0,0,0.5);
    overflow: hidden;
  }
  .modal-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 1.5rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-2);
  }
  .close-btn {
    background: none; border: none; font-size: 1.5rem; color: var(--fg-3);
    cursor: pointer; padding: 0.5rem; line-height: 0.5;
  }
  .close-btn:hover { color: var(--fg-0); }
  .modal-body {
    padding: 1.5rem;
  }
  .spinner { text-align: center; color: var(--fg-3); padding: 2rem; }
  .two-cols { grid-template-columns: 1fr 1fr; gap: 0.75rem; margin-bottom: 1.5rem; }
  .stat-card {
    display: flex; flex-direction: column; gap: 0.25rem;
    padding: 0.75rem;
  }
  .restrictions { margin-bottom: 1.5rem; }
  .restrictions h4 { margin-top: 0; margin-bottom: 0.5rem; font-size: var(--font-size-sm); color: var(--fg-2); }
  .tags { display: flex; gap: 0.5rem; flex-wrap: wrap; }
  .actions-grid {
    display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem;
  }
  .action-btn {
    padding: 0.65rem; background: var(--bg-2); border: 1px solid var(--border);
    color: var(--fg-1); border-radius: var(--radius-sm); cursor: pointer;
    font-size: var(--font-size-sm);
  }
  .action-btn:hover { background: var(--bg-3); border-color: var(--accent); color: var(--fg-0); }
  .action-btn.danger:hover { background: rgba(239, 68, 68, 0.1); border-color: var(--bad); color: var(--bad); }
  .success {
    padding: 0.65rem 1rem; background: rgba(16, 185, 129, 0.1);
    border-left: 3px solid var(--good); border-radius: var(--radius);
    color: var(--good); margin-bottom: 1rem; font-weight: 500;
  }
</style>
