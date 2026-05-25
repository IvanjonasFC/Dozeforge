<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import Skeleton from '$components/Skeleton.svelte';

  let cwd = $state('/sdcard');
  let resolvedCwd = $state('/sdcard'); // actual resolved path on device
  let files = $state<Array<{ name: string; is_dir: boolean; size: number; date: string }>>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let actionBusy = $state(false);

  async function loadDir(path: string) {
    if (!deviceStore.selected) return;
    loading = true; error = null;
    try {
      const result = await api.listFiles(deviceStore.selected.serial, path);
      // Sort: folders first, then files alphabetically
      result.sort((a, b) => {
        if (a.is_dir && !b.is_dir) return -1;
        if (!a.is_dir && b.is_dir) return 1;
        return a.name.localeCompare(b.name);
      });
      files = result;
      cwd = path;
      resolvedCwd = path; // will be updated by backend resolve
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') loadDir(cwd);
  });

  function goUp() {
    // Remove trailing slash, split and pop
    const normalized = cwd.replace(/\/$/, '');
    if (normalized === '' || normalized === '/') return;
    const parts = normalized.split('/');
    parts.pop();
    const parent = parts.length <= 1 ? '/' : parts.join('/');
    loadDir(parent);
  }

  function navigate(f: { name: string; is_dir: boolean }) {
    if (!f.is_dir) return;
    const base = cwd.replace(/\/$/, '');
    loadDir(`${base}/${f.name}`);
  }

  async function uploadFile() {
    if (!deviceStore.selected) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ multiple: false });
      if (!selected || Array.isArray(selected)) return;
      
      const fileName = (selected as string).split(/[\\/]/).pop();
      const remotePath = cwd.endsWith('/') ? cwd + fileName : cwd + '/' + fileName;
      
      actionBusy = true; error = null;
      await api.pushFile(deviceStore.selected.serial, selected as string, remotePath);
      await loadDir(cwd);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      actionBusy = false;
    }
  }

  async function downloadFile(f: any) {
    if (!deviceStore.selected || f.is_dir) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const savePath = await save({ defaultPath: f.name });
      if (!savePath) return;
      
      const remotePath = cwd.endsWith('/') ? cwd + f.name : cwd + '/' + f.name;
      
      actionBusy = true; error = null;
      await api.pullFile(deviceStore.selected.serial, remotePath, savePath);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      actionBusy = false;
    }
  }

  async function createFolder() {
    if (!deviceStore.selected) return;
    const folderName = prompt("Enter new folder name:");
    if (!folderName) return;
    try {
      const remotePath = cwd.endsWith('/') ? cwd + folderName : cwd + '/' + folderName;
      actionBusy = true; error = null;
      await api.createDirectory(deviceStore.selected.serial, remotePath);
      await loadDir(cwd);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      actionBusy = false;
    }
  }

  async function deleteItem(f: any) {
    if (!deviceStore.selected) return;
    if (!confirm(`Are you sure you want to delete ${f.name}? This action cannot be undone.`)) return;
    try {
      const remotePath = cwd.endsWith('/') ? cwd + f.name : cwd + '/' + f.name;
      actionBusy = true; error = null;
      await api.deleteFile(deviceStore.selected.serial, remotePath);
      await loadDir(cwd);
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      actionBusy = false;
    }
  }
</script>

<header class="page-head">
  <div>
    <h1>File Manager</h1>
    <p class="muted">Explore internal storage, transfer files quickly via ADB.</p>
  </div>
  <div class="actions">
    <button class="btn outline" onclick={createFolder} disabled={actionBusy || !deviceStore.selected}>
      New Folder
    </button>
    <button class="primary" onclick={uploadFile} disabled={actionBusy || !deviceStore.selected}>
      {actionBusy ? 'Uploading...' : 'Upload File Here'}
    </button>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected.</p></div>
{:else}
  <div class="card" style="padding: 0;">
    <div class="path-bar">
      <button class="btn icon-btn" onclick={goUp} disabled={cwd === '/' || cwd === '/sdcard'}>↑ Up</button>
      <input type="text" value={cwd} readonly class="path-input" />
      <button class="btn" onclick={() => loadDir(cwd)} disabled={loading}>↻ Refresh</button>
    </div>
    
    {#if error}
      <div class="error" style="margin: 1rem;">{error}</div>
    {/if}

    <div class="file-list">
      {#if loading}
        <div style="padding: 1rem;"><Skeleton lines={10} /></div>
      {:else if files.length === 0}
        <div class="empty-state muted">Directory is empty or permission denied.</div>
      {:else}
        <table>
          <thead>
            <tr>
              <th style="width: 40px;"></th>
              <th>Name</th>
              <th>Size</th>
              <th>Date</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each files as f}
              <tr class="file-row" class:clickable={f.is_dir} onclick={() => navigate(f)}>
                <td class="icon">{f.is_dir ? '📁' : '📄'}</td>
                <td class="name">{f.name}</td>
                <td class="mono small muted">
                  {#if f.is_dir}
                    —
                  {:else if f.size >= 1048576}
                    {(f.size / 1048576).toFixed(1)} MB
                  {:else if f.size >= 1024}
                    {(f.size / 1024).toFixed(1)} KB
                  {:else}
                    {f.size} B
                  {/if}
                </td>
                <td class="mono small muted">{f.date}</td>
                <td class="actions-td" onclick={(e) => e.stopPropagation()}>
                  <div style="display: flex; gap: 0.5rem; justify-content: flex-end; align-items: center;">
                    {#if !f.is_dir}
                      <button class="btn small outline" onclick={() => downloadFile(f)} disabled={actionBusy}>↓ Download</button>
                    {:else}
                      <span class="muted small">Open →</span>
                    {/if}
                    <button class="btn small outline" style="color: var(--error);" onclick={() => deleteItem(f)} disabled={actionBusy}>Delete</button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>
{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; }
  
  .path-bar {
    display: flex; gap: 0.5rem; padding: 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-2);
    align-items: center;
  }
  .path-input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--fg-1);
    background: var(--bg-1);
    border: 1px solid var(--border);
    padding: 0.4rem 0.75rem;
    border-radius: var(--radius-sm);
  }
  
  .file-list {
    max-height: 65vh;
    overflow-y: auto;
  }
  
  table { width: 100%; border-collapse: collapse; }
  th { text-align: left; font-size: 11px; text-transform: uppercase; color: var(--fg-3); padding: 0.75rem 1rem; border-bottom: 1px solid var(--border); position: sticky; top: 0; background: var(--bg-1); z-index: 10; }
  td { padding: 0.75rem 1rem; border-bottom: 1px solid var(--border-soft); vertical-align: middle; }
  
  .file-row { cursor: pointer; transition: background 0.1s; }
  .file-row:hover { background: var(--bg-2); }
  .icon { font-size: 1.2rem; }
  .name { font-weight: 500; color: var(--fg-0); }
  
  .empty-state { padding: 2rem; text-align: center; }
  .actions-td { width: 100px; text-align: right; }
</style>
