<script lang="ts">
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { snapshotStore } from '$stores/snapshots.svelte';
  import { cache } from '$stores/cache.svelte';
  import type { OptimizationAction, OptimizationReport, StandbyBucket } from '$types';

  interface Props {
    initialPackage?: string;
  }
  let { initialPackage = '' }: Props = $props();

  let pkg = $state('');
  let bucket = $state<StandbyBucket>('restricted');
  let revokeBackground = $state(true);
  let revokeWakelock = $state(true);
  let killNow = $state(false);
  let busy = $state(false);
  let report = $state<OptimizationReport | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    if (initialPackage) pkg = initialPackage;
  });

  const actions = $derived.by<OptimizationAction[]>(() => {
    if (!pkg) return [];
    const list: OptimizationAction[] = [
      { kind: 'set_standby_bucket', package: pkg, bucket }
    ];
    if (revokeBackground) {
      list.push({ kind: 'set_app_op', package: pkg, op: 'RUN_IN_BACKGROUND', mode: 'ignore' });
      list.push({ kind: 'set_app_op', package: pkg, op: 'RUN_ANY_IN_BACKGROUND', mode: 'ignore' });
    }
    if (revokeWakelock) {
      list.push({ kind: 'set_app_op', package: pkg, op: 'WAKE_LOCK', mode: 'ignore' });
    }
    if (killNow) {
      list.push({ kind: 'kill_package', package: pkg });
    }
    return list;
  });

  async function apply() {
    if (!deviceStore.selected) return;
    busy = true;
    error = null;
    report = null;
    try {
      report = await api.applyOptimization(deviceStore.selected.serial, actions);
      cache.invalidateAll();
      await snapshotStore.refresh();
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="card">
  <h3>Optimisation</h3>
  <p class="muted">A snapshot of the current state is taken automatically before any change.</p>

  <div class="form">
    <label>
      Package
      <input type="text" placeholder="com.example.app" bind:value={pkg} spellcheck="false" autocomplete="off" />
    </label>

    <label>
      Standby bucket
      <select bind:value={bucket}>
        <option value="active">active</option>
        <option value="working_set">working set</option>
        <option value="frequent">frequent</option>
        <option value="rare">rare</option>
        <option value="restricted">restricted</option>
      </select>
    </label>

    <label class="row">
      <input type="checkbox" bind:checked={revokeBackground} />
      Revoke RUN_IN_BACKGROUND
    </label>
    <label class="row">
      <input type="checkbox" bind:checked={revokeWakelock} />
      Revoke WAKE_LOCK
    </label>
    <label class="row">
      <input type="checkbox" bind:checked={killNow} />
      Force-stop now (am kill)
    </label>
  </div>

  {#if actions.length > 0}
    <div class="preview">
      <h4>Will execute:</h4>
      <ul>
        {#each actions as a (a.kind + ('package' in a ? a.package : ''))}
          <li class="mono">{a.kind} -> {'package' in a ? a.package : `value=${a.value}`}</li>
        {/each}
      </ul>
    </div>
  {/if}

  <button class="primary" onclick={apply} disabled={busy || actions.length === 0}>
    {busy ? 'Applying...' : `Apply (${actions.length} step${actions.length === 1 ? '' : 's'})`}
  </button>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  {#if report}
    <div class="report">
      <h4>Report</h4>
      <p>Snapshot: <code class="mono">{report.snapshot_id.slice(0, 12)}...</code></p>
      <table>
        <thead>
          <tr><th>Action</th><th>Result</th><th>Message</th></tr>
        </thead>
        <tbody>
          {#each report.outcomes as outcome, i (i)}
            <tr>
              <td class="mono">{outcome.action.kind}</td>
              <td>
                <span class="badge {outcome.success ? 'moderate' : 'critical'}">
                  {outcome.success ? 'OK' : 'FAIL'}
                </span>
              </td>
              <td class="mono">{outcome.message || '--'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .form { display: grid; gap: 0.75rem; margin-bottom: 1rem; }
  label { display: block; color: var(--fg-2); font-size: 0.85rem; }
  label.row { display: flex; gap: 0.5rem; align-items: center; color: var(--fg-1); }
  label.row input { width: auto; }
  .preview { background: var(--bg-2); border-radius: var(--radius); padding: 0.75rem 1rem; margin-bottom: 1rem; }
  .preview h4 { margin: 0 0 0.5em 0; font-size: 0.8rem; color: var(--fg-2); text-transform: uppercase; }
  .preview ul { margin: 0; padding-left: 1.25rem; color: var(--fg-1); font-size: 0.85rem; }
  .report { margin-top: 1rem; }
</style>
