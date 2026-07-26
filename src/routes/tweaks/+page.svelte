<script lang="ts">
  import { goto } from '$app/navigation';
  import { deviceStore } from '$stores/device.svelte';
  import { api, DozeForgeError } from '$lib/tauri/api';

  let error = $state<string | null>(null);
  let success = $state<string | null>(null);

  let selectedScale = $state('0.5');
  let animationLoading = $state(false);
  let ramPlusLoading = $state(false);
  let compileLoading = $state(false);
  let headsUpLoading = $state(false);
  let phantomLimit = $state(128);
  let phantomLoading = $state(false);
  let hotwordLoading = $state(false);
  let loggingLoading = $state(false);
  let adaptiveConnLoading = $state(false);
  let stayAwakeLoading = $state(false);
  let darkModeLoading = $state(false);
  let sensorsOffLoading = $state(false);

  let displaySize = $state('');
  let displayDensity = $state('');
  let displayLoading = $state(false);

  async function run<T>(fn: () => Promise<T>, setLoading: (v: boolean) => void, msg: string) {
    setLoading(true); error = null; success = null;
    try { await fn(); success = msg; }
    catch (e) { error = (e as DozeForgeError).message; }
    finally { setLoading(false); }
  }

  const serial = () => deviceStore.selected!.serial;
</script>

<header class="page-head">
  <div>
    <h1>Advanced Tweaks</h1>
    <p class="muted">Non-root system optimizations. Doze and Background Scan controls live in the Sleep tab to avoid duplication.</p>
  </div>
</header>

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">No device connected.</p></div>
{:else}
  {#if error}<div class="card error-banner"><p>{error}</p></div>{/if}
  {#if success}<div class="card success-banner"><p>{success}</p></div>{/if}

  <div class="section-label">Performance</div>
  <div class="grid">

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>System Animations</h3>
          <p class="muted small">Reduce animation duration for instant UI response.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/></svg>
      </div>
      <div class="tweak-actions">
        <select bind:value={selectedScale}>
          <option value="1.0">1.0x — Default</option>
          <option value="0.5">0.5x — Fast</option>
          <option value="0.0">0.0x — Instant</option>
        </select>
        <button class="btn" onclick={() => run(() => api.setAnimationScales(serial(), parseFloat(selectedScale)), v => animationLoading = v, `Animation scales set to ${selectedScale}x.`)} disabled={animationLoading}>
          {animationLoading ? 'Applying...' : 'Apply'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>AOT App Compilation</h3>
          <p class="muted small">Pre-compiles all apps Ahead-Of-Time. Faster launches, lower runtime CPU. Takes 5+ minutes.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--warn)" stroke-width="2"><path d="M12 2v20M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn warn" onclick={() => run(() => api.compileAllApps(serial()), v => compileLoading = v, 'All apps compiled with AOT.')} disabled={compileLoading}>
          {compileLoading ? 'Compiling... (wait)' : 'Force Compile All Apps'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Phantom Process Limit</h3>
          <p class="muted small">Android 12+ kills background CLI processes (Termux, shell scripts). Set a higher limit to preserve them.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--purple)" stroke-width="2"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>
      </div>
      <div class="tweak-actions">
        <select bind:value={phantomLimit}>
          <option value={32}>32 — Strict</option>
          <option value={64}>64 — Conservative</option>
          <option value={128}>128 — Balanced</option>
          <option value={256}>256 — Generous</option>
          <option value={2147483647}>Unlimited</option>
        </select>
        <button class="btn" onclick={() => run(() => api.setPhantomProcessLimit(serial(), phantomLimit), v => phantomLoading = v, `Phantom limit set to ${phantomLimit === 2147483647 ? 'Unlimited' : phantomLimit}.`)} disabled={phantomLoading}>
          {phantomLoading ? 'Applying...' : 'Set Limit'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Disable Virtual RAM (RAM Plus)</h3>
          <p class="muted small">Stops the device using slow flash storage as swap RAM. Recommended for Samsung and Xiaomi.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--good)" stroke-width="2"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn outline" onclick={() => run(() => api.disableRamPlus(serial()), v => ramPlusLoading = v, 'RAM Plus disabled. Reboot may be required.')} disabled={ramPlusLoading}>
          {ramPlusLoading ? 'Applying...' : 'Disable RAM Plus'}
        </button>
      </div>
    </div>

  </div>

  <div class="section-label" style="margin-top: 2rem;">Display Overclocking</div>
  <div class="grid">
    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Custom Resolution & Density</h3>
          <p class="muted small">Force custom pixel dimensions (WxH) or DPI. Incorrect values may make the UI unusable. Use Reset if stuck.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
      </div>
      <div class="tweak-actions" style="flex-direction: column; align-items: stretch; gap: 0.5rem;">
        <div style="display: flex; gap: 0.5rem;">
          <input type="text" placeholder="1080x2400" bind:value={displaySize} style="flex: 1;" />
          <button class="btn outline" onclick={() => run(() => api.setDisplaySize(serial(), displaySize), v => displayLoading = v, 'Resolution applied.')} disabled={displayLoading || !displaySize}>Apply Size</button>
        </div>
        <div style="display: flex; gap: 0.5rem;">
          <input type="text" placeholder="420" bind:value={displayDensity} style="flex: 1;" />
          <button class="btn outline" onclick={() => run(() => api.setDisplayDensity(serial(), displayDensity), v => displayLoading = v, 'Density applied.')} disabled={displayLoading || !displayDensity}>Apply DPI</button>
        </div>
        <button class="btn danger" style="margin-top: 0.5rem;" onclick={() => run(() => api.resetDisplay(serial()), v => displayLoading = v, 'Display reset to factory defaults.')} disabled={displayLoading}>
          Reset Display to Factory Defaults
        </button>
      </div>
    </div>
  </div>

  <div class="section-label" style="margin-top: 2rem;">System Behavior</div>
  <div class="grid">

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Disable Heads-Up Banners</h3>
          <p class="muted small">Stops notification banners from dropping over full-screen content. Useful for gaming and focus sessions.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--blue)" stroke-width="2"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn outline" onclick={() => run(() => api.setHeadsUpNotifications(serial(), true), v => headsUpLoading = v, 'Heads-Up notifications restored.')} disabled={headsUpLoading}>Restore</button>
        <button class="btn" onclick={() => run(() => api.setHeadsUpNotifications(serial(), false), v => headsUpLoading = v, 'Heads-Up banners disabled.')} disabled={headsUpLoading}>
          {headsUpLoading ? 'Applying...' : 'Disable Banners'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Force Dark Mode</h3>
          <p class="muted small">Forces dark mode globally across all apps, including those without an official dark theme.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--fg-2)" stroke-width="2"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn outline" onclick={() => run(() => api.setDarkMode(serial(), false), v => darkModeLoading = v, 'Dark mode override removed.')} disabled={darkModeLoading}>Restore</button>
        <button class="btn" onclick={() => run(() => api.setDarkMode(serial(), true), v => darkModeLoading = v, 'Force Dark Mode enabled.')} disabled={darkModeLoading}>
          {darkModeLoading ? 'Applying...' : 'Force Dark Mode'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Stay Awake While Charging</h3>
          <p class="muted small">Keeps the screen on when connected to a charger, USB, or wireless pad.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--warn)" stroke-width="2"><polyline points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn outline" onclick={() => run(() => api.setStayAwake(serial(), false), v => stayAwakeLoading = v, 'Stay-awake while charging disabled.')} disabled={stayAwakeLoading}>Restore</button>
        <button class="btn" onclick={() => run(() => api.setStayAwake(serial(), true), v => stayAwakeLoading = v, 'Screen stays on while charging.')} disabled={stayAwakeLoading}>
          {stayAwakeLoading ? 'Applying...' : 'Enable Stay Awake'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Disable Activity Logging</h3>
          <p class="muted small">Stops Android from logging every screen and process start to storage. Reduces I/O wear and background CPU.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--blue)" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn outline" onclick={() => run(() => api.setActivityLogging(serial(), true), v => loggingLoading = v, 'Activity logging restored.')} disabled={loggingLoading}>Restore</button>
        <button class="btn" onclick={() => run(() => api.setActivityLogging(serial(), false), v => loggingLoading = v, 'Activity logging disabled.')} disabled={loggingLoading}>
          {loggingLoading ? 'Applying...' : 'Disable Logging'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Sensors Off Quick Tile</h3>
          <p class="muted small">Adds a hidden Quick Setting tile to the notification shade that kills ALL sensors (mic, camera, gyroscope, GPS) at the hardware level. Android 10+ only.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--bad)" stroke-width="2"><path d="M1 1l22 22"/><path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55"/><path d="M5 12.55a10.94 10.94 0 0 1 5.17-2.39"/><path d="M10.71 5.05A16 16 0 0 1 22.56 9"/><path d="M1.42 9a15.91 15.91 0 0 1 4.7-2.88"/><path d="M8.53 16.11a6 6 0 0 1 6.95 0"/><line x1="12" y1="20" x2="12.01" y2="20"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn" onclick={() => run(() => api.enableSensorsOffTile(serial()), v => sensorsOffLoading = v, 'Sensors Off tile added to your Quick Settings. Pull down the notification shade to find it.')} disabled={sensorsOffLoading}>
          {sensorsOffLoading ? 'Applying...' : 'Enable Sensors Off Tile'}
        </button>
      </div>
    </div>

  </div>

  <div class="section-label" style="margin-top: 2rem;">Radio & Connectivity</div>
  <div class="grid">

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Disable "Hey Google" Listener</h3>
          <p class="muted small">Stops Google from keeping the microphone active 24/7. Noticeable standby battery improvement.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--bad)" stroke-width="2"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="22"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn outline" onclick={() => run(() => api.setHotwordDetection(serial(), true), v => hotwordLoading = v, 'Hotword listener restored.')} disabled={hotwordLoading}>Restore</button>
        <button class="btn" onclick={() => run(() => api.setHotwordDetection(serial(), false), v => hotwordLoading = v, 'Hotword listener disabled.')} disabled={hotwordLoading}>
          {hotwordLoading ? 'Applying...' : 'Disable Hotword'}
        </button>
      </div>
    </div>

    <div class="card tweak-card">
      <div class="tweak-header">
        <div>
          <h3>Disable Adaptive Connectivity</h3>
          <p class="muted small">Stops constant band-switching between 4G, 5G, and Wi-Fi. Saves battery in areas with weak or variable signal.</p>
        </div>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--purple)" stroke-width="2"><path d="M5 12.55a11 11 0 0 1 14.08 0"/><path d="M1.42 9a16 16 0 0 1 21.16 0"/><path d="M8.53 16.11a6 6 0 0 1 6.95 0"/><line x1="12" y1="20" x2="12.01" y2="20"/></svg>
      </div>
      <div class="tweak-actions">
        <button class="btn outline" onclick={() => run(() => api.setAdaptiveConnectivity(serial(), true), v => adaptiveConnLoading = v, 'Adaptive Connectivity restored.')} disabled={adaptiveConnLoading}>Restore</button>
        <button class="btn" onclick={() => run(() => api.setAdaptiveConnectivity(serial(), false), v => adaptiveConnLoading = v, 'Adaptive Connectivity disabled.')} disabled={adaptiveConnLoading}>
          {adaptiveConnLoading ? 'Applying...' : 'Disable Adaptive'}
        </button>
      </div>
    </div>

  </div>

  <div class="card xref-card" style="margin-top: 2rem;">
    <p class="small muted" style="margin:0 0 0.75rem;">Doze and Background Scan controls are in the Sleep tab for full wakelock context.</p>
    <button class="btn outline" onclick={() => goto('/sleep/')}>Open Sleep Analyzer</button>
  </div>

{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(370px, 1fr));
    gap: 1.25rem;
    margin-top: 0.75rem;
  }
  .section-label {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--fg-3);
    margin-bottom: 0.5rem;
    padding-left: 2px;
  }
  .tweak-card {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 1.25rem 1.5rem;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    transition: border-color var(--t-fast);
  }
  .tweak-card:hover { border-color: var(--border-hover); }
  .tweak-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
    margin-bottom: 1.25rem;
  }
  .tweak-header h3 { margin: 0 0 0.2rem; color: var(--fg-0); font-size: 0.9rem; }
  .tweak-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    justify-content: flex-end;
  }
  .tweak-actions select {
    padding: 0.4rem 0.6rem;
    border-radius: var(--radius-sm);
    background: var(--bg-2);
    border: 1px solid var(--border);
    color: var(--fg-0);
    font-size: var(--font-size-sm);
  }
  .btn.warn { background: rgba(234,179,8,0.1); color: var(--warn); border: 1px solid var(--warn); }
  .btn.warn:hover { background: var(--warn); color: var(--bg-0); }
  .error-banner { background: rgba(239,68,68,0.1); border-left: 3px solid var(--bad); padding: 1rem; color: var(--bad); }
  .success-banner { background: rgba(16,185,129,0.1); border-left: 3px solid var(--good); padding: 1rem; color: var(--good); }
  .xref-card { padding: 1.25rem 1.5rem; }
</style>
