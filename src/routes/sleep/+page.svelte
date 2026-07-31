<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { appModalStore } from '$stores/appModal.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import AppName from '$components/AppName.svelte';
  import OemNote from '$components/OemNote.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import type {
    AuditReport,
    MiscategorizedApp,
    SleepScore,
    SleepTimeline,
    KernelWakelock,
    WakeupSources,
    PerformanceSettings,
    DozeState,
    OptimizationAction
  } from '$types';
  import { formatDuration } from '$utils/format';

  let score = $state<SleepScore | null>(null);
  let audit = $state<AuditReport | null>(null);
  let wakeup = $state<WakeupSources | null>(null);
  let misc = $state<MiscategorizedApp[]>([]);
  let timeline = $state<SleepTimeline | null>(null);
  let kernelWl = $state<KernelWakelock[]>([]);
  let perfSettings = $state<PerformanceSettings | null>(null);
  let dozeState = $state<DozeState | null>(null);
  let loading = $state(false);
  let perfBusy = $state(false);
  let motionBusy = $state(false);
  let motionDisabled = $state(false);
  let heartbeatBusy = $state(false);
  let heartbeatInterval = $state(900000);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);
  let showAdvanced = $state(false);
  let filter = $state('');

  let appRestrictions = $state<Record<string, any>>({});

  async function analyze() {
    if (!deviceStore.selected) return;
    loading = true;
    error = null;
    success = null;
    try {
      const [sc, a, w, m, t, kw, p, ds] = await Promise.all([
        api.sleepScore(deviceStore.selected.serial),
        api.auditDevice(deviceStore.selected.serial),
        api.listWakeupSources(deviceStore.selected.serial),
        api.miscategorizedApps(deviceStore.selected.serial).catch(() => []),
        api.sleepTimeline(deviceStore.selected.serial).catch(() => null),
        api.kernelWakelocks(deviceStore.selected.serial).catch(() => []),
        api.getPerformanceSettings(deviceStore.selected.serial).catch(() => null),
        api.getDozeState(deviceStore.selected.serial).catch(() => null)
      ]);
      score = sc;
      audit = a;
      wakeup = w;
      misc = m;
      timeline = t;
      kernelWl = kw;
      perfSettings = p;
      dozeState = ds;
      
      if (audit?.culprits) {
        const pkgs = audit.culprits.map(c => c.package).slice(0, 30);
        api.getAppRestrictionsBatch(deviceStore.selected.serial, pkgs)
           .then(res => { appRestrictions = res; })
           .catch(e => console.warn(e));
      }
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    if (deviceStore.selected?.state === 'device') {
      analyze();
      loadBuckets();
    }
  });

  function gotoActions(pkg: string) {
    // Open the in-place app modal instead of navigating away from the page;
    // the modal already picks up the "battery" context from the URL (`/sleep`)
    // and surfaces the relevant wakelock / standby / background controls.
    appModalStore.open(pkg);
  }

  async function toggleDoze() {
    if (!deviceStore.selected || !perfSettings) return;
    perfBusy = true; error = null;
    try {
      await api.setAggressiveDoze(deviceStore.selected.serial, !perfSettings.aggressive_doze_enabled);
      perfSettings.aggressive_doze_enabled = !perfSettings.aggressive_doze_enabled;
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { perfBusy = false; }
  }

  async function toggleBgScan() {
    if (!deviceStore.selected || !perfSettings) return;
    perfBusy = true; error = null;
    try {
      const target = !(perfSettings.wifi_scan_always_enabled || perfSettings.ble_scan_always_enabled);
      await api.setBackgroundScan(deviceStore.selected.serial, target, target);
      perfSettings.wifi_scan_always_enabled = target;
      perfSettings.ble_scan_always_enabled = target;
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { perfBusy = false; }
  }

  async function toggleDozeMotion() {
    if (!deviceStore.selected) return;
    motionBusy = true; error = null; success = null;
    try {
      const next = !motionDisabled;
      await api.disableDozeMotion(deviceStore.selected.serial, next);
      motionDisabled = next;
      success = next ? i18n.t("Motion sensor detection in Doze DISABLED. Deep sleep will engage even while moving.") : i18n.t("Motion sensor detection in Doze RESTORED to default.");
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { motionBusy = false; }
  }

  async function applyHeartbeat() {
    if (!deviceStore.selected) return;
    heartbeatBusy = true; error = null; success = null;
    try {
      await api.tuneGmsHeartbeat(deviceStore.selected.serial, heartbeatInterval);
      success = i18n.t('GMS heartbeat interval set to {{min}} minutes.', { min: Math.round(heartbeatInterval / 60000) });
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { heartbeatBusy = false; }
  }

  async function simulateUnplug() {
    if (!deviceStore.selected) return;
    loading = true; error = null;
    try {
      const s = deviceStore.selected.serial;
      // 1) Pretend the device is on battery so Doze is allowed to run.
      await api.simulateUnplug(s, true);
      // 2) Advance the deep-idle state machine one phase, so each click visibly
      //    steps ACTIVE → INACTIVE → … → IDLE (otherwise Doze only progresses on
      //    its own after minutes with the screen off — nothing to see here live).
      await api.runShell(s, 'dumpsys deviceidle step deep');
      // 3) Re-read the new state.
      await analyze();
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { loading = false; }
  }


  let allBuckets = $state<{ package: string, bucket: string }[]>([]);
  let bucketsLoading = $state(false);
  let bucketsFilter = $state('');

  async function loadBuckets() {
    if (!deviceStore.selected) return;
    bucketsLoading = true; error = null;
    try {
      allBuckets = await api.getAllStandbyBuckets(deviceStore.selected.serial);
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { bucketsLoading = false; }
  }

  async function updateBucket(pkg: string, bucket: string) {
    if (!deviceStore.selected) return;
    try {
      await api.setStandbyBucket(deviceStore.selected.serial, pkg, bucket);
      const b = allBuckets.find(x => x.package === pkg);
      if (b) b.bucket = bucket;
      success = i18n.t('Priority of {{pkg}} changed to {{bucket}}', { pkg, bucket });
    } catch (e) { error = (e as DozeForgeError).message; }
  }

  // Color per standby bucket, from most freedom / battery drain (red) to most
  // restricted / battery friendly (green). Drives both the legend and the badges.
  // Green = runs freely (consumes most) → Red = frozen (saves most).
  const BUCKET_META: Record<string, { color: string; note: string }> = {
    exempted:    { color: '#22C55E', note: 'allowlisted' },
    active:      { color: '#84CC16', note: 'no limits' },
    working_set: { color: '#EAB308', note: 'mild limits' },
    frequent:    { color: '#F59E0B', note: 'medium limits' },
    rare:        { color: '#F97316', note: 'strict limits' },
    restricted:  { color: '#EF4444', note: 'frozen' },
    never:       { color: '#991B1B', note: 'never runs' },
  };
  const BUCKET_ORDER = ['active', 'working_set', 'frequent', 'rare', 'restricted', 'exempted'];
  const BUCKET_LABEL: Record<string, string> = {
    active: 'Active', working_set: 'Working Set', frequent: 'Frequent',
    rare: 'Rare', restricted: 'Restricted', exempted: 'Exempted', never: 'Never',
  };
  function bucketColor(b: string): string {
    return BUCKET_META[b.toLowerCase()]?.color ?? 'var(--fg-3)';
  }
  function bucketLabel(k: string): string {
    return BUCKET_LABEL[k.toLowerCase()] ?? k;
  }

  // Custom "change priority" dropdown (styled like the top-bar device picker),
  // shared and fixed-positioned so it never gets clipped by the scroll area.
  // Only the user-actionable buckets are offered (Android internal values in
  // parentheses); Exempted (5) and Never (50) are system/edge states we don't
  // let users set by hand.
  const USER_BUCKETS = [
    { value: 'active', n: 10 },
    { value: 'working_set', n: 20 },
    { value: 'frequent', n: 30 },
    { value: 'rare', n: 40 },
    { value: 'restricted', n: 45 },
  ];
  let menuPkg = $state<string | null>(null);
  let menuPos = $state<{ left: number; top: number }>({ left: 0, top: 0 });
  function openBucketMenu(e: MouseEvent, pkg: string) {
    e.stopPropagation();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menuPos = { left: r.right - 190, top: r.bottom + 4 };
    menuPkg = pkg;
  }
  function chooseBucket(value: string) {
    if (menuPkg) updateBucket(menuPkg, value);
    menuPkg = null;
  }
  $effect(() => {
    if (!menuPkg) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') menuPkg = null; };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  // Some kernel wakelocks map to a real, safe toggle. Return an action for those,
  // null for the purely diagnostic ones (radio/suspend/etc. can't be "fixed").
  function kernelQuickFix(name: string): { label: string; run: (s: string) => Promise<unknown> } | null {
    const n = name.toLowerCase();
    if (n.includes('nfc')) {
      return { label: i18n.t('Disable NFC'), run: (s) => api.runShell(s, 'svc nfc disable') };
    }
    if (n.includes('wlan_scan') || n.includes('wifi_scan')) {
      return { label: i18n.t('Stop background scan'), run: (s) => api.setBackgroundScan(s, false, false) };
    }
    return null;
  }
  let kernelFixBusy = $state<string | null>(null);
  async function runKernelFix(name: string) {
    if (!deviceStore.selected) return;
    const fix = kernelQuickFix(name);
    if (!fix) return;
    kernelFixBusy = name;
    try {
      await fix.run(deviceStore.selected.serial);
      success = i18n.t('Applied: {{label}}', { label: fix.label });
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { kernelFixBusy = null; }
  }

  async function removeWhitelist(pkg: string) {
    if (!deviceStore.selected) return;
    loading = true; error = null;
    try {
      await api.setDozeWhitelist(deviceStore.selected.serial, pkg, false);
      await analyze();
    } catch (e) { error = (e as DozeForgeError).message; }
    finally { loading = false; }
  }

  // Set of packages with live wakelocks, for joining into the culprit table.
  const liveWakelockSet = $derived(
    new Set(
      (wakeup?.live_wakelocks ?? [])
        .map((wl) => wl.package)
        .filter((p): p is string => !!p)
    )
  );

  // Set of packages currently holding sensor handles.
  const sensorMap = $derived(
    new Map((wakeup?.sensors ?? []).map((s) => [s.package, s.sensors.length]))
  );

  // Top culprits with extra columns derived from cross-references.
  const culprits = $derived.by(() => {
    const list = audit?.culprits ?? [];
    const f = filter.trim().toLowerCase();
    return list
      .filter((c) => {
        if (!f) return true;
        if (c.package.toLowerCase().includes(f)) return true;
        const lbl = labelStore.labelFor(deviceStore.selected?.serial ?? null, c.package).toLowerCase();
        return lbl.includes(f);
      })
      .slice(0, 30);
  });

  /** Heuristic legitimacy verdict for the culprit table. */
  function verdictOf(pkg: string, wakelockMs: number, sensorCount: number) {
    const isMedia = /spotify|youtube|music|podcast|netflix|deezer|tidal|amazon\.mp3|soundcloud/i.test(
      pkg
    );
    if (isMedia && wakelockMs > 5 * 60_000) return { tier: 'ok', label: i18n.t("Likely legitimate (media)") };
    if (sensorCount > 0 && wakelockMs > 30 * 60_000) return { tier: 'warn', label: i18n.t("Sensor-bound — review") };
    if (wakelockMs > 30 * 60_000) return { tier: 'bad', label: i18n.t("Background hog") };
    if (liveWakelockSet.has(pkg)) return { tier: 'bad', label: i18n.t("Holding wakelock now") };
    if (wakelockMs > 5 * 60_000) return { tier: 'warn', label: i18n.t("Moderate drain") };
    return { tier: 'ok', label: i18n.t("Minor") };
  }

  /** Pixel-width for the timeline bars relative to on_battery_realtime. */
  function pctOf(ms: number, base: number): number {
    if (!base || base === 0) return 0;
    return Math.min(100, (ms / base) * 100);
  }

  function fmtRatio(r: number): string {
    return `${(r * 100).toFixed(1)}%`;
  }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Sleep Analyzer')}</h1>
    <p class="muted">
      {i18n.t('Why the device is not sleeping when the screen is off. Three layers: global timeline, per-app culprits, and (optionally) kernel-level wakelocks.')}
    </p>
  </div>
  <button class="primary" onclick={analyze} disabled={loading || !deviceStore.selected}>
    {loading ? i18n.t('Analyzing…') : i18n.t('Re-analyze')}
  </button>
</header>

<OemNote />

{#if !deviceStore.selected}
  <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>
{:else}
  {#if success}<div class="success">{success}</div>{/if}
  {#if error}<div class="error">{error}</div>{/if}
  <!-- ============================================================ -->
  <!-- Layer 1 — Sleep Efficiency Timeline (macro context)           -->
  <!-- ============================================================ -->
  <div class="card" style="margin-bottom: 1rem;">
    <div class="row" style="justify-content: space-between; align-items: flex-end; margin-bottom: 0.75rem;">
      <div>
        <h3 style="margin: 0 0 0.25rem 0;">{i18n.t('Sleep efficiency timeline')}</h3>
        <p class="muted footnote" style="margin: 0;">
          {i18n.t('Hours the screen was off vs. how much of that time the CPU actually slept. Awake-with-screen-off is the leak you can fix.')}
        </p>
      </div>
      {#if timeline}
        <div class="ratio-pill" data-tier={timeline.tier}>
          <span class="ratio-value">{fmtRatio(timeline.efficiency_ratio)}</span>
          <span class="ratio-label">{timeline.tier}</span>
        </div>
      {/if}
    </div>

    {#if !timeline}
      {#if loading}<Skeleton lines={4} />{:else}
        <p class="muted">{i18n.t('Timeline not available — device may not have been on battery long enough.')}</p>
        <p class="muted small">{i18n.t('It needs recent time on battery. Over a USB cable the phone is always charging, so this stays empty — enable Wi-Fi ADB (device picker → Enable TCP/IP), unplug the cable, use the phone for a while, then re-analyze.')}</p>
      {/if}
    {:else}
      {@const base = Math.max(timeline.on_battery_realtime_ms, timeline.screen_off_realtime_ms, 1)}
      <div class="timeline">
        <!-- Screen off realtime -->
        <div class="tl-row">
          <span class="tl-label" title={i18n.t("Wall-clock hours the screen was off, while on battery.")}>{i18n.t('Screen off')}</span>
          <div class="tl-bar-wrap">
            <div class="tl-bar tl-screen-off" style="width: {pctOf(timeline.screen_off_realtime_ms, base)}%"></div>
          </div>
          <span class="tl-value mono">{formatDuration(timeline.screen_off_realtime_ms)}</span>
        </div>
        <!-- Deep sleep -->
        <div class="tl-row">
          <span class="tl-label" title={i18n.t("Time the CPU was actually suspended with screen off. Higher is better.")}>{i18n.t('Deep sleep')}</span>
          <div class="tl-bar-wrap">
            <div class="tl-bar tl-deep" style="width: {pctOf(timeline.deep_sleep_ms, base)}%"></div>
          </div>
          <span class="tl-value mono">{formatDuration(timeline.deep_sleep_ms)}</span>
        </div>
        <!-- Awake (the leak) -->
        <div class="tl-row">
          <span class="tl-label" title={i18n.t("Screen was off, but the CPU stayed awake. Every minute here drains battery.")}>{i18n.t('Awake (screen off)')}</span>
          <div class="tl-bar-wrap">
            <div class="tl-bar tl-awake" style="width: {pctOf(timeline.screen_off_uptime_ms, base)}%"></div>
          </div>
          <span class="tl-value mono">{formatDuration(timeline.screen_off_uptime_ms)}</span>
        </div>
      </div>
      <p class="muted footnote" style="margin-top: 0.85rem;">
        {@html i18n.t('Healthy reference: efficiency ≥ 85%, awake-with-screen-off &lt; 15% of screen-off time.')}
      </p>
    {/if}
  </div>

  <!-- ============================================================ -->
  <!-- Layer 2 — Score and Miscategorized                            -->
  <!-- ============================================================ -->
  <div class="grid two-grid">
    <div class="card score-card">
      {#if !score}
        <Skeleton lines={5} />
      {:else}
        <div class="score-inner">
          <div class="score-number" data-tier={score.tier}>{score.score}</div>
          <div class="score-meta">
            <div class="score-tier" data-tier={score.tier}>{score.tier}</div>
            <div class="muted">{i18n.t('out of 100')}</div>
          </div>
        </div>
        <div class="penalties">
          {#if score.penalties.length === 0}
            <p class="muted">{i18n.t('No penalties — device sleeps well.')}</p>
          {:else}
            {#each score.penalties as p, i (i)}
              <div class="penalty">
                <span class="penalty-label">{p.label}</span>
                <span class="penalty-points mono">{p.points}</span>
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </div>

    <div class="card">
      <h3>{i18n.t('Miscategorized apps')}</h3>
      <p class="muted footnote">
        {i18n.t('Android keeps these in privileged buckets, but you have not used them recently.')}
      </p>
      {#if loading}
        <Skeleton lines={4} />
      {:else if misc.length === 0}
        <p class="muted">{i18n.t('None detected — buckets are well calibrated.')}</p>
      {:else}
        <div class="scroll-y" style="max-height: 230px;">
          <table>
            <thead><tr><th>{i18n.t('Package')}</th><th>{i18n.t('Current')}</th><th>{i18n.t('Suggest')}</th><th></th></tr></thead>
            <tbody>
              {#each misc.slice(0, 12) as m (m.package)}
                <tr class="app-row" onclick={() => gotoActions(m.package)} style="cursor: pointer;" title="Open optimization options">
                  <td><AppName package={m.package} size="sm" hidePackage inline /></td>
                  <td><span class="badge moderate">{i18n.t(m.current_bucket)}</span></td>
                  <td><span class="badge ok">{i18n.t(m.recommended_bucket)}</span></td>
                  <td><button onclick={() => gotoActions(m.package)}>{i18n.t('Fix')}</button></td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </div>

  <!-- ============================================================ -->
  <!-- Layer 2.5 — Idle Settings & State Machine                     -->
  <!-- ============================================================ -->
  <div class="grid two-grid" style="margin-top: 1rem;">
    {#if dozeState}
      <div class="card" style="grid-column: 1 / -1; padding: 1.25rem;">
        <div class="row" style="justify-content: space-between; align-items: flex-start; margin-bottom: 1rem;">
          <div>
            <h3 style="margin: 0 0 0.5rem;">{i18n.t('Doze State Machine')}</h3>
            <p class="muted small" style="margin: 0;">
              {i18n.t('Real-time state of the device idle controller. The device must pass through sensing phases before deep sleep.')}
            </p>
          </div>
          <button class="secondary" onclick={simulateUnplug} disabled={loading} title={i18n.t('Simulates being on battery and steps the Doze machine one phase forward. Click repeatedly to walk it to deep sleep (IDLE).')}>{i18n.t('Simulate battery')}</button>
        </div>

        <div class="state-machine">
          {#each ['ACTIVE', 'INACTIVE', 'IDLE_PENDING', 'SENSING', 'LOCATING', 'IDLE', 'IDLE_MAINTENANCE'] as st, i}
            <div class="sm-node {dozeState.state.toUpperCase() === st ? 'active' : ''}">
              <div class="sm-circle"></div>
              <span class="sm-label">{st}</span>
            </div>
            {#if i < 6}
              <div class="sm-line {dozeState.state.toUpperCase() === 'IDLE' && i < 5 ? 'past' : ''}"></div>
            {/if}
          {/each}
        </div>
        {#if dozeState.next_alarm_elapsed}
          <p class="muted small" style="margin: 1rem 0 0; text-align: center;">
            {i18n.t('Next alarm/transition in:')} <strong class="mono">{dozeState.next_alarm_elapsed}</strong>
          </p>
        {/if}
      </div>
    {/if}

    {#if perfSettings}
      <div class="card" style="padding: 1.25rem;">
        <h3 style="margin: 0 0 0.75rem;">{i18n.t("Aggressive Doze")}</h3>
        <p class="muted small" style="margin: 0 0 0.85rem;">
          {i18n.t('Forces the device to enter Deep Sleep almost immediately after screen off. Disables motion sensing during doze.')}
        </p>
        <button
          class={perfSettings.aggressive_doze_enabled ? 'danger' : 'primary'}
          onclick={toggleDoze}
          disabled={perfBusy}
        >
          {perfBusy ? '…' : (perfSettings.aggressive_doze_enabled ? i18n.t("Disable") : i18n.t("Enable Naptime-style Doze"))}
        </button>
      </div>
      <div class="card" style="padding: 1.25rem;">
        <h3 style="margin: 0 0 0.75rem;">{i18n.t("Background Scanning")}</h3>
        <p class="muted small" style="margin: 0 0 0.85rem;">
          {i18n.t('Allows apps to scan for Wi-Fi and Bluetooth even when those radios are turned off. Wakes up the device constantly.')}
        </p>
        <button
          class={(perfSettings.wifi_scan_always_enabled || perfSettings.ble_scan_always_enabled) ? 'danger' : 'primary'}
          onclick={toggleBgScan}
          disabled={perfBusy}
        >
          {perfBusy ? '…' : ((perfSettings.wifi_scan_always_enabled || perfSettings.ble_scan_always_enabled) ? i18n.t("Disable Background Scan") : i18n.t("Re-enable Background Scan"))}
        </button>
      </div>
      <div class="card" style="padding: 1.25rem;">
        <h3 style="margin: 0 0 0.75rem;">{i18n.t("Motion Sensor Override")}</h3>
        <p class="muted small" style="margin: 0 0 0.85rem;">
          {i18n.t('Prevents motion detection from waking the device during Doze. Inspired by Naptime. The device will enter deep sleep even while in your pocket or bag.')}
        </p>
        <button
          class={motionDisabled ? 'danger' : 'primary'}
          onclick={toggleDozeMotion}
          disabled={motionBusy}
        >
          {motionBusy ? '...' : (motionDisabled ? i18n.t("Restore Motion Detection") : i18n.t("Disable Motion in Doze"))}
        </button>
      </div>
      <div class="card" style="padding: 1.25rem;">
        <h3 style="margin: 0 0 0.75rem;">{i18n.t("GMS Heartbeat Interval")}</h3>
        <p class="muted small" style="margin: 0 0 0.85rem;">
          {i18n.t('Controls how often Google Play Services wakes the device for Firebase Cloud Messaging. Higher intervals save battery but may delay push notifications.')}
        </p>
        <div style="display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 0.85rem;">
          <button class="outline" class:active={heartbeatInterval === 300000} onclick={() => heartbeatInterval = 300000}>{i18n.t("5 min")}</button>
          <button class="outline" class:active={heartbeatInterval === 900000} onclick={() => heartbeatInterval = 900000}>{i18n.t("15 min (default)")}</button>
          <button class="outline" class:active={heartbeatInterval === 1800000} onclick={() => heartbeatInterval = 1800000}>{i18n.t("30 min")}</button>
          <button class="outline" class:active={heartbeatInterval === 3600000} onclick={() => heartbeatInterval = 3600000}>{i18n.t("60 min")}</button>
        </div>
        <button class="primary" onclick={applyHeartbeat} disabled={heartbeatBusy}>
          {heartbeatBusy ? '...' : i18n.t("Apply Heartbeat Interval")}
        </button>
      </div>
    {/if}
  </div>

  <!-- ============================================================ -->
  <!-- Layer 3 — Per-app culprits (richer table)                     -->
  <!-- ============================================================ -->
  <div class="card" style="margin-top: 1rem;">
    <div class="row" style="justify-content: space-between; margin-bottom: 0.85rem; gap: 0.75rem;">
      <h3 style="margin: 0;">{i18n.t("Per-app culprits")}</h3>
      <input
        type="text"
        placeholder={i18n.t('Filter by package…')}
        bind:value={filter}
        class="filter-input"
      />
    </div>
    {#if culprits.length === 0}
      <p class="muted">{i18n.t('No significant offenders found.')}</p>
    {:else}
      <div class="scroll-y" style="max-height: 460px;">
        <table>
          <thead>
            <tr>
              <th title={i18n.t("Application package name (com.example.app).")}>{i18n.t("Package")}</th>
              <th title={i18n.t("Total time the app held a partial wakelock since last unplug. The bigger, the more it kept the CPU running.")}>{i18n.t("Wakelock")}</th>
              <th title={i18n.t("Number of times the app woke the device from deep sleep via an alarm.")}>{i18n.t("Wakeups")}</th>
              <th title={i18n.t("Background jobs scheduled by the app.")}>{i18n.t("Jobs")}</th>
              <th title={i18n.t("Number of sensor handles held by the app (GPS, accelerometer, etc.).")}>{i18n.t("Sensors")}</th>
              <th title={i18n.t("If the app reaches a privileged service through GMS, the culprit is reattributed here.")}>{i18n.t("Proxy")}</th>
              <th title={i18n.t("Composite score — higher means worse impact.")}>{i18n.t("Score")}</th>
              <th title={i18n.t("Human-readable verdict so non-technical users can act.")}>{i18n.t("Verdict")}</th>
            </tr>
          </thead>
          <tbody>
            {#each culprits as c (c.package)}
              {@const sensorCount = sensorMap.get(c.package) ?? 0}
              {@const v = verdictOf(c.package, c.wakelock_ms, sensorCount)}
              <tr class:live={liveWakelockSet.has(c.package)} class="app-row" onclick={() => gotoActions(c.package)} style="cursor: pointer;" title="Click to open optimization options">
                <td><AppName package={c.package} size="sm" hidePackage inline /></td>
                <td class="mono">{formatDuration(c.wakelock_ms)}</td>
                <td class="mono">{c.wakeup_count}</td>
                <td class="mono">{c.job_count}</td>
                <td class="mono">{sensorCount}</td>
                <td>
                  {#if c.redirected_from_proxy}
                    <span class="badge ok" title={i18n.t('reattributed from {{pkg}}', { pkg: c.redirected_from_proxy })}>
                      {i18n.t('via')} {c.redirected_from_proxy.split('.').slice(-1)[0]}
                    </span>
                  {:else}
                    <span class="muted">—</span>
                  {/if}
                </td>
                <td>
                  <div style="display: flex; flex-direction: column; gap: 4px; align-items: flex-start;">
                    <strong class="mono">{c.score.toFixed(1)}</strong>
                    {#if appRestrictions[c.package]}
                      {#if appRestrictions[c.package].wake_lock_ignored}
                        <span class="badge danger" style="font-size: 9px; padding: 2px 4px;">{i18n.t("Wake Blocked")}</span>
                      {/if}
                      {#if appRestrictions[c.package].run_in_background_ignored}
                        <span class="badge danger" style="font-size: 9px; padding: 2px 4px;">{i18n.t("Bg Blocked")}</span>
                      {/if}
                      {#if appRestrictions[c.package].standby_bucket === 'restricted'}
                        <span class="badge ok" style="font-size: 9px; padding: 2px 4px;">{i18n.t("Restricted")}</span>
                      {/if}
                    {/if}
                  </div>
                </td>
                <td><span class="badge {v.tier}">{v.label}</span></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <!-- ============================================================ -->
  <!-- Layer 3.5 — Standby Buckets Manager                           -->
  <!-- ============================================================ -->
  <div class="card" style="margin-top: 1rem;">
    <div class="row" style="justify-content: space-between; align-items: flex-start; margin-bottom: 0.85rem;">
      <div>
        <h3 style="margin: 0 0 0.5rem;">{i18n.t("Standby Buckets Manager")}</h3>
        <p class="muted small" style="margin: 0 0 0.6rem; max-width: 620px;">
          {i18n.t('Android assigns each app a priority (bucket). The lower the priority, the more its background activity is throttled — saving battery. Colors go from green (runs freely) to red (frozen).')}
        </p>
        <div class="bucket-legend">
          {#each BUCKET_ORDER as key}
            <span class="bk-chip" style="--bk: {BUCKET_META[key].color}">
              {i18n.t(bucketLabel(key))}<em>{i18n.t(BUCKET_META[key].note)}</em>
            </span>
          {/each}
        </div>
      </div>
      <button class="secondary" onclick={loadBuckets} disabled={bucketsLoading}>
        {bucketsLoading ? i18n.t("Loading…") : (allBuckets.length ? i18n.t("Refresh Buckets") : i18n.t("Load Standby Buckets"))}
      </button>
    </div>

    {#if allBuckets.length > 0}
      <input type="text" placeholder={i18n.t('Filter apps…')} bind:value={bucketsFilter} class="filter-input" style="margin-bottom: 1rem; width: 100%; max-width: 300px;" />
      <div class="scroll-y" style="max-height: 400px;">
        <table>
          <thead>
            <tr>
              <th>{i18n.t("Application")}</th>
              <th>{i18n.t("Current Bucket")}</th>
              <th>{i18n.t("Override Bucket")}</th>
            </tr>
          </thead>
          <tbody>
            {#each allBuckets.filter(b => b.package.toLowerCase().includes(bucketsFilter.toLowerCase())) as b (b.package)}
              <tr class="app-row">
                <td><AppName package={b.package} size="sm" hidePackage inline /></td>
                <td>
                  <span class="bk-badge" style="--bk: {bucketColor(b.bucket)}">{i18n.t(bucketLabel(b.bucket))}</span>
                </td>
                <td>
                  <button class="bk-change" class:open={menuPkg === b.package} onclick={(e) => openBucketMenu(e, b.package)}>
                    {i18n.t("Change...")}
                    <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M3 4.5l3 3 3-3"/></svg>
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    {#if menuPkg}
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div class="bk-menu-backdrop" onclick={() => (menuPkg = null)} role="presentation"></div>
      <div class="bk-menu" style="left:{menuPos.left}px; top:{menuPos.top}px" role="menu">
        <div class="bk-menu-head">{i18n.t('Set priority to')}</div>
        {#each USER_BUCKETS as opt}
          <button class="bk-menu-item" title={`bucket ${opt.n}`} onclick={() => chooseBucket(opt.value)}>
            <span class="bk-dot" style="background:{BUCKET_META[opt.value].color}"></span>
            <span class="bk-name">{i18n.t(bucketLabel(opt.value))}</span>
            <span class="bk-note">{i18n.t(BUCKET_META[opt.value].note)}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- ============================================================ -->
  <!-- Doze whitelist + Live wakelocks                               -->
  <!-- ============================================================ -->
  {#if wakeup}
    <div class="grid two-grid" style="margin-top: 1rem;">
      <div class="card">
        <h3>{i18n.t("Doze whitelist")}</h3>
        <p class="muted footnote">
          {i18n.t('Apps allowed to bypass deep sleep. Third-party entries are the biggest single drain you can fix.')}
        </p>
        {#if wakeup.doze_whitelist.user_whitelisted.length === 0}
          <p class="muted">{i18n.t('Clean — no user-whitelisted apps bypassing Doze.')}</p>
        {:else}
          <ul class="ul-list">
            {#each wakeup.doze_whitelist.user_whitelisted as pkg (pkg)}
              <li>
                <AppName package={pkg} size="sm" hidePackage inline />
                <button onclick={() => removeWhitelist(pkg)}>{i18n.t('Remove')}</button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="card">
        <h3>{i18n.t('Currently held wakelocks')}</h3>
        <p class="muted footnote">
          {i18n.t('Wakelocks active at the moment of analysis. Empty here is a good sign.')}
        </p>
        {#if wakeup.live_wakelocks.length === 0}
          <p class="muted">{i18n.t('None right now — good sign.')}</p>
        {:else}
          <ul class="ul-list">
            {#each wakeup.live_wakelocks as wl, i (i)}
              <li>
                <span class="badge moderate">{wl.flags}</span>
                <code class="mono">{wl.tag}</code>
                {#if wl.package}<AppName package={wl.package} size="sm" hidePackage inline />{/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  {/if}

  <!-- ============================================================ -->
  <!-- Advanced — Kernel wakelocks (Ring 0)                          -->
  <!-- ============================================================ -->
  <div class="card" style="margin-top: 1rem;">
    <button class="toggle-advanced" onclick={() => (showAdvanced = !showAdvanced)}>
      <span>{showAdvanced ? '▾' : '▸'}</span>
      {i18n.t('Advanced — Kernel wakelocks')} ({kernelWl.length})
    </button>
    {#if showAdvanced}
      <p class="muted footnote" style="margin: 0.75rem 0;">
        {i18n.t('Hardware-level wakelocks held by drivers (Wi-Fi, modem, NFC, display). These are invisible to per-app restrictions: if your worst row here is wlan_rx_wake, restricting Spotify will not help — the fix is in your router or carrier signal.')}
      </p>
      {#if kernelWl.length === 0}
        <div class="card flat info-banner" style="margin-top: 0.75rem; display: flex; align-items: center; justify-content: center; min-height: 120px; text-align: center;">
          <div>
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="var(--fg-2)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="margin-bottom: 0.5rem;"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>
            <p style="margin: 0; color: var(--fg-1); font-weight: 500;">{i18n.t('No Kernel Wakelocks Detected')}</p>
            <p class="muted small" style="margin: 0.25rem 0 0; max-width: 400px;">
              {i18n.t("Either the device hasn't been off-charger long enough, or this kernel doesn't expose standard wakelocks via dumpsys.")}
            </p>
          </div>
        </div>
      {:else}
        <div class="scroll-y" style="max-height: 380px;">
          <table>
            <thead>
              <tr>
                <th>{i18n.t('Name')}</th>
                <th>{i18n.t('Total held')}</th>
                <th>{i18n.t('Count')}</th>
                <th>{i18n.t('Severity')}</th>
                <th>{i18n.t('What it usually means')}</th>
                <th>{i18n.t('Fix')}</th>
              </tr>
            </thead>
            <tbody>
              {#each kernelWl.slice(0, 25) as k (k.name)}
                {@const fix = kernelQuickFix(k.name)}
                <tr>
                  <td class="mono">{k.name}</td>
                  <td class="mono">{formatDuration(k.total_ms)}</td>
                  <td class="mono">{k.count}</td>
                  <td><span class="badge {k.severity === 'critical' ? 'bad' : k.severity === 'high' ? 'bad' : k.severity === 'moderate' ? 'warn' : 'ok'}">{k.severity}</span></td>
                  <td class="small">{k.explanation}</td>
                  <td>
                    {#if fix}
                      <button class="btn outline small" disabled={kernelFixBusy === k.name} onclick={() => runKernelFix(k.name)}>
                        {kernelFixBusy === k.name ? i18n.t('Applying…') : fix.label}
                      </button>
                    {:else}
                      <span class="muted" style="font-size: 11px;">—</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/if}
  </div>

  <!-- ============================================================ -->
  <!-- Layer 4 — Pending Alarms and Jobs                             -->
  <!-- ============================================================ -->
  {#if wakeup}
    <div class="grid two-grid" style="margin-top: 1rem;">
      <div class="card">
        <h3>{i18n.t('Pending Alarms')}</h3>
        <p class="muted footnote">
          {i18n.t('Apps that have scheduled alarms to wake the device.')}
        </p>
        {#if wakeup.alarms.length === 0}
          <p class="muted">{i18n.t('No pending alarms found.')}</p>
        {:else}
          <div class="scroll-y" style="max-height: 380px;">
            <table>
              <thead>
                <tr>
                  <th>{i18n.t('Application')}</th>
                  <th>{i18n.t('Wake Type')}</th>
                  <th>{i18n.t('Count')}</th>
                </tr>
              </thead>
              <tbody>
                {#each wakeup.alarms.slice(0, 30) as a (a.triggering_package + a.kind)}
                  <tr class="app-row" onclick={() => gotoActions(a.triggering_package)}>
                    <td><AppName package={a.triggering_package} size="sm" hidePackage inline /></td>
                    <td><span class="badge {a.kind.includes('wakeup') ? 'warn' : 'moderate'}">{a.kind}</span></td>
                    <td class="mono">{a.wake_count}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>

      <div class="card">
        <h3>{i18n.t('Pending Jobs')}</h3>
        <p class="muted footnote">
          {i18n.t('Apps with background tasks scheduled in the JobScheduler.')}
        </p>
        {#if wakeup.jobs.length === 0}
          <p class="muted">{i18n.t('No pending jobs found.')}</p>
        {:else}
          <div class="scroll-y" style="max-height: 380px;">
            <table>
              <thead>
                <tr>
                  <th>{i18n.t('Application')}</th>
                  <th>{i18n.t('Total Jobs')}</th>
                  <th>{i18n.t('Periodic')}</th>
                </tr>
              </thead>
              <tbody>
                {#each wakeup.jobs.slice(0, 30) as j (j.package)}
                  <tr class="app-row" onclick={() => gotoActions(j.package)}>
                    <td><AppName package={j.package} size="sm" hidePackage inline /></td>
                    <td class="mono">{j.job_count}</td>
                    <td class="mono">{j.periodic_count}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    </div>
  {/if}
{/if}

<style>
  /* Standby bucket colour legend + badges */
  .bucket-legend { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .bk-chip {
    display: inline-flex; align-items: baseline; gap: 0.35rem;
    padding: 3px 9px; border-radius: 999px; font-size: 11px; font-weight: 600;
    color: var(--bk);
    background: color-mix(in srgb, var(--bk) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--bk) 40%, transparent);
  }
  .bk-chip em { font-style: normal; font-weight: 500; font-size: 10px; color: var(--fg-3); }
  .bk-badge {
    display: inline-block; padding: 2px 10px; border-radius: 999px;
    font-size: 11px; font-weight: 700; letter-spacing: 0.02em;
    color: var(--bk);
    background: color-mix(in srgb, var(--bk) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--bk) 45%, transparent);
  }
  /* Custom "change priority" dropdown (device-picker style) */
  .bk-change {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 4px 10px; font-size: 12px; font-weight: 600;
    background: var(--control-bg); border: 1px solid var(--border);
    border-radius: 8px; color: var(--fg-2); cursor: pointer;
  }
  .bk-change:hover, .bk-change.open { border-color: var(--accent); color: var(--fg-0); }
  .bk-menu-backdrop { position: fixed; inset: 0; z-index: 1000; }
  .bk-menu {
    position: fixed; z-index: 1001; width: 190px;
    background: var(--bg-2); border: 1px solid var(--border-strong);
    border-radius: 12px; padding: 5px; box-shadow: 0 12px 40px rgba(0,0,0,0.5);
  }
  .bk-menu-head { font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--fg-3); padding: 4px 8px 6px; }
  .bk-menu-item {
    display: flex; align-items: center; gap: 0.5rem; width: 100%;
    padding: 7px 8px; background: none; border: none; border-radius: 8px;
    color: var(--fg-1); cursor: pointer; text-align: left; font-size: 13px;
  }
  .bk-menu-item:hover { background: var(--bg-3); }
  .bk-dot { width: 9px; height: 9px; border-radius: 50%; flex-shrink: 0; }
  .bk-name { font-weight: 600; }
  .bk-note { margin-left: auto; font-size: 10px; color: var(--fg-3); }
  .page-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 1.5rem; gap: 1rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 60ch; }

  .two-grid { grid-template-columns: 1fr 1fr; gap: 1rem; }
  @media (max-width: 980px) { .two-grid { grid-template-columns: 1fr; } }

  /* ---------- Timeline ---------- */
  .timeline { display: flex; flex-direction: column; gap: 0.65rem; }
  .tl-row { display: grid; grid-template-columns: 160px 1fr 110px; align-items: center; gap: 0.85rem; }
  .tl-label { font-size: var(--font-size-sm); color: var(--fg-1); cursor: help; }
  .tl-bar-wrap { background: var(--bg-3); border-radius: var(--radius-sm); height: 22px; overflow: hidden; }
  .tl-bar { height: 100%; border-radius: var(--radius-sm); transition: width 250ms ease-out; }
  .tl-screen-off { background: linear-gradient(90deg, var(--accent) 0%, var(--accent) 100%); opacity: 0.55; }
  .tl-deep       { background: linear-gradient(90deg, var(--good) 0%, var(--good) 100%); }
  .tl-awake      { background: linear-gradient(90deg, var(--bad) 0%, var(--warn) 100%); }
  .tl-value { text-align: right; font-size: var(--font-size-sm); color: var(--fg-0); }

  .ratio-pill {
    display: flex; flex-direction: column; align-items: flex-end;
    padding: 0.45rem 0.85rem; border-radius: var(--radius);
    background: var(--bg-3); border: 1px solid var(--border);
  }
  .ratio-value { font-family: var(--font-mono); font-size: 22px; font-weight: 700; line-height: 1; }
  .ratio-label { font-size: 10px; text-transform: uppercase; letter-spacing: 0.1em; margin-top: 2px; }
  .ratio-pill[data-tier="excellent"] .ratio-value { color: var(--good); }
  .ratio-pill[data-tier="excellent"] .ratio-label { color: var(--good); }
  .ratio-pill[data-tier="good"]      .ratio-value { color: var(--accent); }
  .ratio-pill[data-tier="good"]      .ratio-label { color: var(--accent); }
  .ratio-pill[data-tier="mediocre"]  .ratio-value { color: var(--warn); }
  .ratio-pill[data-tier="mediocre"]  .ratio-label { color: var(--warn); }
  .ratio-pill[data-tier="bad"]       .ratio-value { color: var(--bad); }
  .ratio-pill[data-tier="bad"]       .ratio-label { color: var(--bad); }

  /* ---------- Score ---------- */
  .score-card { display: flex; flex-direction: column; }
  .score-inner { display: flex; align-items: center; gap: 1.5rem; padding-bottom: 1rem; border-bottom: 1px solid var(--border); }
  .score-number {
    font-family: var(--font-mono);
    font-size: 72px;
    font-weight: 700;
    letter-spacing: -0.03em;
    line-height: 1;
  }
  .score-number[data-tier="excellent"] { color: var(--good); }
  .score-number[data-tier="good"]      { color: var(--accent); }
  .score-number[data-tier="mediocre"]  { color: var(--warn); }
  .score-number[data-tier="bad"]       { color: var(--bad); }
  .score-meta { display: flex; flex-direction: column; gap: 4px; }
  .score-tier {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 600;
  }
  .score-tier[data-tier="excellent"] { color: var(--good); }
  .score-tier[data-tier="good"]      { color: var(--accent); }
  .score-tier[data-tier="mediocre"]  { color: var(--warn); }
  .score-tier[data-tier="bad"]       { color: var(--bad); }

  .penalties { margin-top: 1rem; display: flex; flex-direction: column; gap: 8px; }
  .penalty {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.5rem 0.75rem;
    background: var(--bg-3); border-radius: var(--radius);
    font-size: var(--font-size-sm);
  }
  .penalty-points { color: var(--bad); font-weight: 600; }

  /* ---------- Filter + table ---------- */
  .filter-input {
    padding: 0.4rem 0.75rem;
    background: var(--bg-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--fg-0);
    font-size: var(--font-size-sm);
    min-width: 240px;
  }
  .filter-input::placeholder { color: var(--fg-3); }

  tr.live td:first-child::before {
    content: '';
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--bad);
    margin-right: 6px;
    vertical-align: middle;
    animation: pulse 1.6s infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  /* ---------- Advanced toggle ---------- */
  .toggle-advanced {
    background: none; border: none; color: var(--fg-0); cursor: pointer;
    font-size: var(--font-size-md); font-weight: 600;
    padding: 0; display: flex; align-items: center; gap: 8px;
  }
  .toggle-advanced:hover { color: var(--accent); }

  .footnote { font-size: var(--font-size-xs); margin-top: -0.25rem; margin-bottom: 0.85rem; max-width: 72ch; }

  .ul-list { list-style: none; padding: 0; margin: 0.5rem 0 0; display: flex; flex-direction: column; gap: 4px; }
  
  .action-dropdown {
    padding: 0.35rem;
    font-size: var(--font-size-xs);
    background: var(--bg-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--fg-1);
    cursor: pointer;
  }
  .action-dropdown:hover {
    border-color: var(--accent);
  }
  .success {
    padding: 0.65rem 1rem;
    background: rgba(16, 185, 129, 0.1);
    border-left: 3px solid var(--good);
    border-radius: var(--radius);
    color: var(--good);
    margin-bottom: 1rem;
    font-weight: 500;
  }
  .ul-list li {
    display: flex; align-items: center; gap: 0.65rem;
    padding: 0.45rem 0.75rem;
    background: var(--bg-3); border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
  }
  .ul-list li button { margin-left: auto; padding: 0.25rem 0.65rem; font-size: var(--font-size-xs); }
  .small { font-size: var(--font-size-xs); }

  .info-banner {
    padding: 0.85rem 1.15rem;
    border-color: rgba(255, 107, 0, 0.2);
    background: rgba(255, 107, 0, 0.04);
  }
  .info-banner p { margin: 0 0 0.5rem; font-size: var(--font-size-sm); color: var(--fg-1); }
  .info-banner p:last-child { margin-bottom: 0; }

  /* ---------- State Machine ---------- */
  .state-machine {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 1.5rem;
    padding: 1rem 0;
    overflow-x: auto;
  }
  .sm-node {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    opacity: 0.5;
    transition: all 0.3s ease;
  }
  .sm-node.active {
    opacity: 1;
    transform: scale(1.1);
  }
  .sm-circle {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--bg-3);
    border: 2px solid var(--border);
    transition: all 0.3s ease;
  }
  .sm-node.active .sm-circle {
    background: var(--accent);
    border-color: var(--accent);
    box-shadow: 0 0 12px var(--accent-dim);
  }
  .sm-label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: var(--fg-1);
    white-space: nowrap;
  }
  .sm-node.active .sm-label {
    color: var(--accent);
  }
  .sm-line {
    flex-grow: 1;
    height: 2px;
    background: var(--border);
    margin: 0 10px;
    margin-bottom: 20px;
    transition: all 0.3s ease;
  }
  .sm-line.past {
    background: var(--accent-dim);
  }
</style>
