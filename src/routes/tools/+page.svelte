<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/state';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { formatTimestamp } from '$utils/format';
  import { i18n } from '$stores/i18n.svelte';
  import type { ActionLogEntry, OptimizationAction } from '$types';

  type ToolsTab = 'logs' | 'bugreport' | 'profiles' | 'console';
  const tabParam = page.url.searchParams.get('tab');
  const initialTab: ToolsTab =
    tabParam === 'bugreport' || tabParam === 'profiles' || tabParam === 'logs' ? tabParam : 'console';
  let activeTab = $state<ToolsTab>(initialTab);

  // Logs
  let logs = $state<string[]>([]);
  let logMode = $state<'logcat' | 'dmesg'>('logcat');
  let logStreaming = $state(false);
  let logUnlisten: UnlistenFn | null = null;
  let logContainer: HTMLElement | undefined = $state();
  let logHeuristics = $state<{ type: string, msg: string, time: string }[]>([]);
  // Incoming lines are buffered and flushed to the DOM on a throttle so a chatty
  // logcat stream can't saturate the main thread (which blocked tab switching).
  let logBuffer: string[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  // F12 — logcat pro: filter displayed lines by severity.
  let logLevel = $state<'all' | 'D' | 'I' | 'W' | 'E'>('all');
  const LEVEL_RANK: Record<string, number> = { D: 0, I: 1, W: 2, E: 3, F: 4 };
  const displayLogs = $derived.by(() => {
    if (logLevel === 'all') return logs;
    const min = LEVEL_RANK[logLevel] ?? 0;
    return logs.filter((l) => {
      const g = l.match(/ ([VDIWEF]) /)?.[1];
      const r = g ? (LEVEL_RANK[g] ?? 0) : 0;
      return r >= min;
    });
  });

  // F5 — in-app ADB console.
  let consoleCmd = $state('');
  let consoleBusy = $state(false);
  let consoleHistory = $state<{ cmd: string; out: string; err: boolean }[]>([]);

  // F13 — send text to the device.
  let sendText = $state('');
  let sendBusy = $state(false);


  // Bugreport
  let bugreportText = $state<string | null>(null);
  let bugreportLoading = $state(false);

  // System Actions State & Logic
  let actionLoading = $state(false);
  let actionSuccess = $state<string | null>(null);
  let actionError = $state<string | null>(null);

  // Profiles
  let exportPath = $state<string | null>(null);
  let actionLog = $state<ActionLogEntry[]>([]);
  
  async function startLogs() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device' || logStreaming) return;
    logs = [];
    logHeuristics = [];
    try {
      logUnlisten = await api.onLogBatch((newLines) => {
        // Cheap scan happens immediately; DOM update is throttled below.
        for (const l of newLines) {
          if (l.includes('FATAL EXCEPTION')) logHeuristics.push({ type: 'Exception', msg: l.substring(0, 100), time: new Date().toLocaleTimeString() });
          else if (l.includes('ANR in ')) logHeuristics.push({ type: 'ANR', msg: l.substring(0, 100), time: new Date().toLocaleTimeString() });
          else if (l.includes('WIN DEATH')) logHeuristics.push({ type: 'Crash', msg: l.substring(0, 100), time: new Date().toLocaleTimeString() });
        }
        if (logHeuristics.length > 5) logHeuristics.shift();

        logBuffer.push(...newLines);
        if (flushTimer) return;
        flushTimer = setTimeout(() => {
          flushTimer = null;
          if (logBuffer.length === 0) return;
          logs.push(...logBuffer);
          logBuffer = [];
          if (logs.length > 800) logs.splice(0, logs.length - 800);
          if (logContainer) {
            const atBottom = logContainer.scrollHeight - logContainer.clientHeight <= logContainer.scrollTop + 80;
            if (atBottom) setTimeout(() => { if (logContainer) logContainer.scrollTop = logContainer.scrollHeight; }, 10);
          }
        }, 250);
      });
      await api.startLogStream(deviceStore.selected.serial, logMode);
      logStreaming = true;
    } catch (e) { console.error(e); }
  }

  async function stopLogs() {
    try { await api.stopLogStream(); } catch {}
    if (logUnlisten) { logUnlisten(); logUnlisten = null; }
    if (flushTimer) { clearTimeout(flushTimer); flushTimer = null; }
    logBuffer = [];
    logStreaming = false;
  }

  async function generateBugreport() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    bugreportLoading = true;
    bugreportText = 'Generating full bugreport (this usually takes 3 to 5 minutes)...';
    try {
      bugreportText = await api.generateBugreport(deviceStore.selected.serial);
    } catch (e) {
      bugreportText = `Error: ${(e as DozeForgeError).message}`;
    } finally { bugreportLoading = false; }
  }

  // One-click diagnostic export: captures the key dumpsys sections into a file
  // the user can share to help harden device support (Samsung/Xiaomi/etc.).
  let diagPath = $state<string | null>(null);
  let diagError = $state<string | null>(null);
  let diagLoading = $state(false);
  async function exportDiag() {
    if (!deviceStore.selected || deviceStore.selected.state !== 'device') return;
    diagLoading = true; diagPath = null; diagError = null;
    try {
      diagPath = await api.exportDiagnostic(deviceStore.selected.serial);
    } catch (e) {
      diagError = (e as DozeForgeError).message;
    } finally { diagLoading = false; }
  }

  async function exportShellScript() {
    if (!deviceStore.selected) return;
    actionLoading = true; actionError = null; exportPath = null;
    try {
      const actions: OptimizationAction[] = [];
      const label = deviceStore.selected.model ?? deviceStore.selected.serial;
      exportPath = await api.exportShellScript(actions, label);
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  async function exportNativeProfile() {
    if (!deviceStore.selected) return;
    actionLoading = true; actionError = null; exportPath = null;
    try {
      const profile = await api.exportNativeProfile(deviceStore.selected.serial);
      const { save } = await import('@tauri-apps/plugin-dialog');
      const { writeTextFile } = await import('@tauri-apps/plugin-fs');
      const savePath = await save({ defaultPath: 'profile.dozeprofile', filters: [{ name: 'Doze Profile', extensions: ['dozeprofile'] }] });
      if (savePath) {
        await writeTextFile(savePath, JSON.stringify(profile, null, 2));
        exportPath = savePath;
      }
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  async function importNativeProfile() {
    if (!deviceStore.selected) return;
    actionLoading = true; actionError = null; exportPath = null;
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
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { actionLoading = false; }
  }

  async function refreshLog() {
    try { actionLog = await api.readActionLog(100); }
    catch (e) { actionError = (e as DozeForgeError).message; }
  }

  onMount(() => { refreshLog(); });
  onDestroy(() => { stopLogs(); });

  $effect(() => {
    if (activeTab === 'logs' && !logStreaming) startLogs();
    if (activeTab !== 'logs' && logStreaming) stopLogs();
  });

  function getLogClass(line: string) {
    if (line.includes(' E ')) return 'log-e';
    if (line.includes(' W ')) return 'log-w';
    if (line.includes(' I ')) return 'log-i';
    if (line.includes(' D ')) return 'log-d';
    if (line.includes(' F ')) return 'log-f';
    return '';
  }

  async function saveLogs() {
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const { writeTextFile } = await import('@tauri-apps/plugin-fs');
      const path = await save({ defaultPath: `logcat-${Date.now()}.txt`, filters: [{ name: 'Log', extensions: ['txt', 'log'] }] });
      if (path) await writeTextFile(path, displayLogs.join('\n'));
    } catch (e) { actionError = (e as DozeForgeError).message; }
  }

  // F5 — run an arbitrary adb shell command and capture output.
  async function runConsole() {
    if (!deviceStore.selected || !consoleCmd.trim()) return;
    const cmd = consoleCmd.trim();
    consoleBusy = true;
    try {
      const out = await api.runShell(deviceStore.selected.serial, cmd);
      consoleHistory.push({ cmd, out: out.trimEnd() || '(no output)', err: false });
    } catch (e) {
      consoleHistory.push({ cmd, out: (e as DozeForgeError).message, err: true });
    } finally {
      consoleBusy = false;
      consoleCmd = '';
      if (consoleHistory.length > 100) consoleHistory.splice(0, consoleHistory.length - 100);
    }
  }

  // F13 — type text on the device via `input text`.
  async function sendTextToDevice() {
    if (!deviceStore.selected || !sendText) return;
    sendBusy = true;
    try {
      const escaped = sendText.replace(/(["\\$`])/g, '\\$1').replace(/ /g, '%s');
      await api.runShell(deviceStore.selected.serial, `input text "${escaped}"`);
      sendText = '';
    } catch (e) { actionError = (e as DozeForgeError).message; }
    finally { sendBusy = false; }
  }

</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Diagnostics & Tools')}</h1>
    <p class="muted">{i18n.t('Live logs, bugreports, and advanced power-user operations.')}</p>
  </div>
</header>

<div class="tabs">
  <button class:active={activeTab === 'console'} onclick={() => activeTab = 'console'}>{i18n.t('Console')}</button>
  <button class:active={activeTab === 'logs'} onclick={() => activeTab = 'logs'}>{i18n.t('Live Logs')}</button>
  <button class:active={activeTab === 'bugreport'} onclick={() => activeTab = 'bugreport'}>{i18n.t('Bugreport')}</button>
  <button class:active={activeTab === 'profiles'} onclick={() => activeTab = 'profiles'}>{i18n.t('Automation Profiles')}</button>
</div>

<div class="tab-content">
  {#if !deviceStore.selected}
    <div class="card p-card"><p class="muted">{i18n.t('No device connected.')}</p></div>
  {:else if deviceStore.selected.state === 'bootloader'}
    <div class="card p-card" style="border-left: 3px solid var(--accent);">
      <h3>{i18n.t('Fastboot Mode Active')}</h3>
      <p class="muted">{i18n.t('Device is currently in bootloader/fastboot mode.')}</p>
      <div style="margin-top: 1rem;">
        <button class="primary" onclick={async () => { if (deviceStore.selected) await api.fastbootReboot(deviceStore.selected.serial); }}>{i18n.t('Reboot to System')}</button>
      </div>
    </div>
  {:else if deviceStore.selected.state !== 'device'}
    <div class="card p-card"><p class="muted">{i18n.t('Device is offline or unauthorized.')}</p></div>
  {:else if activeTab === 'logs'}
    <div class="card p-card" style="display: flex; flex-direction: column; height: 65vh; padding: 0.5rem;">
      <div style="display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; padding: 0.5rem; flex-wrap: wrap;">
        <div style="display: flex; gap: 0.5rem; align-items: center;">
          <select bind:value={logMode} onchange={() => { stopLogs(); startLogs(); }} class="small">
            <option value="logcat">{i18n.t('Logcat (System Logs)')}</option>
            <option value="dmesg">{i18n.t('Dmesg (Kernel Logs)')}</option>
          </select>
          <div class="lvl-seg">
            {#each ['all', 'D', 'I', 'W', 'E'] as lvl (lvl)}
              <button class="lvl" class:active={logLevel === lvl} onclick={() => logLevel = lvl as typeof logLevel}>
                {lvl === 'all' ? i18n.t('All') : lvl}
              </button>
            {/each}
          </div>
        </div>
        <div style="display: flex; gap: 0.5rem;">
          <button class="btn outline small" onclick={saveLogs} disabled={displayLogs.length === 0}>{i18n.t('Save to file')}</button>
          <button class="btn outline small" onclick={() => logs = []}>{i18n.t('Clear Buffer')}</button>
        </div>
      </div>

      {#if logHeuristics.length > 0}
        <div style="display: flex; gap: 0.5rem; flex-wrap: wrap; padding: 0 0.5rem 0.5rem 0.5rem;">
          {#each logHeuristics as h}
            <div class="badge" class:critical={h.type === 'Crash' || h.type === 'ANR'} class:warn={h.type === 'Exception'} title={h.msg}>
              {h.type} at {h.time}
            </div>
          {/each}
        </div>
      {/if}

      <div bind:this={logContainer} class="log-view" style="flex: 1; overflow-y: auto; background: var(--bg-0); padding: 0.5rem; border-radius: 4px; border: 1px solid var(--border);">
        {#each displayLogs as l}
          <div class="log-line {getLogClass(l)}">{l}</div>
        {/each}
        {#if displayLogs.length === 0}
          <div class="muted small" style="text-align: center; margin-top: 2rem;">{logs.length === 0 ? i18n.t('Waiting for logs...') : i18n.t('No lines at this level.')}</div>
        {/if}
      </div>
    </div>
  {:else if activeTab === 'bugreport'}
    <div class="card p-card">
      <h3>{i18n.t('Generate Bugreport')}</h3>
      <p class="muted small">{i18n.t('Extracts full dumpsys state, logs, and traces. Used by Android engineers for deep analysis.')}</p>
      <button class="primary" onclick={generateBugreport} disabled={bugreportLoading} style="margin-top: 1rem;">
        {bugreportLoading ? i18n.t('Generating...') : i18n.t('Start Bugreport')}
      </button>
      
      {#if bugreportText}
        <textarea class="code-area" readonly style="margin-top: 1rem; height: 400px; width: 100%; white-space: pre; font-size: 11px;">{bugreportText}</textarea>
      {/if}
    </div>

    <div class="card p-card" style="margin-top: 1rem;">
      <h3>{i18n.t('Export diagnostic')}</h3>
      <p class="muted small">{i18n.t('One-click capture of battery, storage, Doze and standby dumps into a single file. Share it to help add support for your exact device/ROM. Read-only, no root.')}</p>
      <button class="primary" onclick={exportDiag} disabled={diagLoading} style="margin-top: 1rem;">
        {diagLoading ? i18n.t('Capturing…') : i18n.t('Export diagnostic')}
      </button>
      {#if diagPath}
        <p class="success" style="margin-top: 1rem;">{i18n.t('Saved to')} <code class="mono">{diagPath}</code></p>
      {/if}
      {#if diagError}<p class="error" style="margin-top: 1rem;">{diagError}</p>{/if}
    </div>
  {:else if activeTab === 'profiles'}
    <div class="grid two-grid">
      <div class="card">
        <h3>{i18n.t('Export profile as shell script')}</h3>
        <p class="muted">{i18n.t('Generates a SHA-256 verified .sh runnable under Termux + Shizuku.')}</p>
        <button class="primary" onclick={exportShellScript} disabled={actionLoading} style="margin-top: 0.85rem;">{i18n.t('Export to Shell Script')}</button>
        <button class="outline" onclick={exportNativeProfile} disabled={actionLoading} style="margin-top: 0.85rem; margin-left: 0.5rem;">{i18n.t('Export Native (.dozeprofile)')}</button>
        <button class="outline" onclick={importNativeProfile} disabled={actionLoading} style="margin-top: 0.85rem; margin-left: 0.5rem;">{i18n.t('Import Native')}</button>
        {#if exportPath}<p class="success" style="margin-top: 1rem;">{i18n.t('Saved to')} <code class="mono">{exportPath}</code></p>{/if}
      </div>
    </div>
    <div class="card" style="margin-top: 1.5rem;">
      <div style="display: flex; justify-content: space-between; align-items: flex-end;">
        <div><h3 style="margin: 0;">{i18n.t('Action history')}</h3><p class="muted small">{i18n.t('Persistent log of every action.')}</p></div>
        <button class="btn outline small" onclick={refreshLog}>{i18n.t('Refresh')}</button>
      </div>
      {#if actionLog.length === 0}
        <p class="muted" style="margin-top: 1rem;">{i18n.t('No actions logged yet.')}</p>
      {:else}
        <div class="table-container" style="max-height: 40vh; margin-top: 0.85rem; overflow-y: auto;">
          <table class="data-table">
            <thead style="position: sticky; top: 0; background: var(--bg-1);">
              <tr><th>{i18n.t('When')}</th><th>{i18n.t('Device')}</th><th>{i18n.t('Action')}</th><th>{i18n.t('Result')}</th></tr>
            </thead>
            <tbody>
              {#each actionLog as entry}
                <tr>
                  <td>{formatTimestamp(entry.ts)}</td>
                  <td class="mono small">{entry.device_serial}</td>
                  <td class="mono">{entry.action.kind}{('package' in entry.action) ? ' · ' + entry.action.package : ''}{('command' in entry.action) ? ' · ' + entry.action.command : ''}</td>
                  <td><span class="badge" class:ok={entry.success} class:critical={!entry.success}>{entry.success ? 'OK' : 'FAIL'}</span></td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {:else if activeTab === 'console'}
    <div class="card p-card" style="display:flex; flex-direction:column; height:60vh; padding:0.75rem;">
      <p class="muted small" style="margin:0 0 0.6rem;">{i18n.t('Runs adb shell <command> on the selected device. Power-user tool — commands run with your ADB permissions.')}</p>
      <div class="con-view" style="flex:1; overflow-y:auto; background:var(--bg-0); border:1px solid var(--border); border-radius:6px; padding:0.6rem; font-family:var(--font-mono); font-size:12px;">
        {#if consoleHistory.length === 0}
          <div class="muted small">{i18n.t('Try: getprop ro.product.model · dumpsys battery · pm list packages -3')}</div>
        {/if}
        {#each consoleHistory as h, i (i)}
          <div class="con-cmd mono">$ {h.cmd}</div>
          <div class="con-out" class:err={h.err}>{h.out}</div>
        {/each}
      </div>
      <div style="display:flex; gap:0.5rem; margin-top:0.6rem; align-items:center;">
        <span class="mono" style="color:var(--accent);">$</span>
        <input
          bind:value={consoleCmd}
          onkeydown={(e) => { if (e.key === 'Enter') runConsole(); }}
          placeholder="getprop ro.build.version.release"
          spellcheck="false" autocomplete="off"
          style="flex:1; font-family:var(--font-mono);"
        />
        <button class="primary" onclick={runConsole} disabled={consoleBusy || !consoleCmd.trim()}>
          {consoleBusy ? i18n.t('Running…') : i18n.t('Run')}
        </button>
      </div>
    </div>
    <div class="card" style="margin-top:1rem;">
      <h3>{i18n.t('Send text to device')}</h3>
      <p class="muted small">{i18n.t('Types the text into the focused field on the phone (via input text).')}</p>
      <div style="display:flex; gap:0.5rem; margin-top:0.75rem;">
        <input bind:value={sendText} placeholder={i18n.t('Text to type on the phone…')} style="flex:1;" />
        <button class="primary" onclick={sendTextToDevice} disabled={sendBusy || !sendText}>
          {sendBusy ? i18n.t('Sending…') : i18n.t('Send')}
        </button>
      </div>
    </div>
  {/if}
</div>

{#if actionSuccess}<div class="success" style="margin-top: 1rem;">{actionSuccess}</div>{/if}
{#if actionError}<div class="error" style="margin-top: 1rem;">{actionError}</div>{/if}

<style>
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 540px; }
  
  .tabs { display: flex; gap: 0.5rem; margin-bottom: 1.5rem; border-bottom: 1px solid var(--border); padding-bottom: 0px; }
  .tabs button { background: transparent; border: none; padding: 0.5rem 1rem; color: var(--fg-2); border-bottom: 2px solid transparent; font-weight: 500; cursor: pointer; border-radius: 0; }
  .tabs button:hover { color: var(--fg-0); }
  .tabs button.active { color: var(--accent); border-bottom-color: var(--accent); }
  
  .p-card { min-height: 400px; }
  .two-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  
  .log-line { font-family: var(--font-mono); font-size: 11px; white-space: pre-wrap; word-break: break-all; margin-bottom: 2px; color: var(--fg-1); }
  .log-e { color: var(--danger); font-weight: bold; }
  .log-w { color: var(--warn); }
  .log-i { color: var(--fg-2); }
  .log-d { color: var(--fg-3); }
  .log-f { color: white; background: var(--danger); }
  
  .success { padding: 0.65rem 1rem; background: rgba(16, 185, 129, 0.1); border-left: 3px solid var(--good); border-radius: var(--radius); color: var(--good); }
  .error { padding: 0.65rem 1rem; background: rgba(239, 68, 68, 0.1); border-left: 3px solid var(--danger); border-radius: var(--radius); color: var(--danger); }

  /* Log level filter (F12) */
  .lvl-seg { display: inline-flex; border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; }
  .lvl-seg .lvl { background: transparent; border: none; border-radius: 0; padding: 0.35rem 0.65rem; font-size: 12px; font-family: var(--font-mono); color: var(--fg-2); cursor: pointer; }
  .lvl-seg .lvl:hover { background: var(--bg-3); color: var(--fg-0); }
  .lvl-seg .lvl.active { background: var(--accent-soft); color: var(--accent); }

  /* ADB console (F5) */
  .con-cmd { color: var(--accent); margin-top: 0.5rem; }
  .con-out { color: var(--fg-1); white-space: pre-wrap; word-break: break-all; margin-bottom: 0.35rem; }
  .con-out.err { color: var(--bad); }
</style>
