<script lang="ts">
  import { onMount } from 'svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import { cache, TTL } from '$stores/cache.svelte';

  type Tab = 'diag' | 'tweaks' | 'root' | 'props' | 'io';
  let activeTab = $state<Tab>('diag');

  const dev = $derived(deviceStore.selected);
  const ready = $derived(dev?.state === 'device');

  let msg = $state<string | null>(null);
  let terr = $state<string | null>(null);
  let busy = $state(false);

  // Props (shared: diagnostics + raw viewer). Seeded from cache so tab revisits
  // render instantly (stale-while-revalidate).
  let props = $state<Record<string, string> | null>(cache.peek<Record<string, string>>('props:' + (deviceStore.selected?.serial ?? '')));
  let propsError = $state<string | null>(null);
  let propsLoading = $state(false);
  let propsFilter = $state('');
  let selinux = $state<string | null>(null);
  let kernel = $state<string | null>(null);

  async function loadProps() {
    if (!ready) return;
    propsLoading = cache.peek('props:' + dev!.serial) === null; // skeleton only if nothing cached
    propsError = null;
    try {
      props = await cache.getOrFetch('props:' + dev!.serial, TTL.medium, () => api.getSystemProperties(dev!.serial));
      try { selinux = (await api.runShell(dev!.serial, 'getenforce')).trim(); } catch { /* optional */ }
      try { kernel = (await api.runShell(dev!.serial, 'uname -r')).trim(); } catch { /* optional */ }
    } catch (e) { propsError = (e as DozeForgeError).message; }
    finally { propsLoading = false; }
  }

  // Interpreted diagnostics
  type Tone = 'good' | 'warn' | 'bad' | 'neutral';
  interface Diag { label: string; value: string; tone: Tone; hint?: string; recovery?: boolean; }
  function p(k: string) { return props?.[k] ?? ''; }
  function yn(v: string) { return (v === 'true' || v === '1') ? i18n.t('Yes') : (v === 'false' || v === '0') ? i18n.t('No') : (v || i18n.t('Unknown')); }
  function monthsSince(date: string): number | null {
    const m = /^(\d{4})-(\d{2})/.exec(date);
    if (!m) return null;
    const then = new Date(Number(m[1]), Number(m[2]) - 1, 1).getTime();
    return Math.floor((Date.now() - then) / (1000 * 60 * 60 * 24 * 30.44));
  }

  const diagnostics = $derived.by<Diag[]>(() => {
    if (!props) return [];
    const rows: Diag[] = [];

    const vbs = p('ro.boot.verifiedbootstate').toLowerCase();
    const locked = (p('ro.boot.flash.locked') || p('ro.boot.vbmeta.device_state')).toLowerCase();
    let btone: Tone = 'neutral'; let bval = i18n.t('Unknown');
    if (vbs === 'green' || locked === '1' || locked === 'locked') { btone = 'good'; bval = i18n.t('Locked & verified'); }
    else if (vbs === 'orange' || vbs === 'yellow' || locked === '0' || locked === 'unlocked') { btone = 'warn'; bval = i18n.t('Bootloader unlocked'); }
    rows.push({ label: i18n.t('Bootloader / Verified Boot'), value: bval, tone: btone,
      hint: btone === 'warn' ? i18n.t('You can flash, but banking apps and Play Integrity may fail.') : undefined });

    const sp = p('ro.build.version.security_patch');
    if (sp) {
      const ms = monthsSince(sp);
      const tone: Tone = ms === null ? 'neutral' : ms > 6 ? 'bad' : ms > 3 ? 'warn' : 'good';
      rows.push({ label: i18n.t('Security patch'), value: sp, tone,
        hint: (ms !== null && ms > 3) ? i18n.t('{{n}} months behind — consider updating.', { n: String(ms) }) : undefined });
    }

    rows.push({ label: i18n.t('Android version'), value: `${p('ro.build.version.release')} · API ${p('ro.build.version.sdk')}`, tone: 'neutral' });

    const ab = p('ro.build.ab_update') === 'true' || !!p('ro.boot.slot_suffix');
    const slot = p('ro.boot.slot_suffix').replace('_', '').toUpperCase();
    rows.push({ label: i18n.t('Seamless A/B updates'), tone: 'neutral',
      value: ab ? `${i18n.t('Yes')}${slot ? ` (${i18n.t('slot')} ${slot})` : ''}` : i18n.t('No (single slot)'),
      hint: ab ? i18n.t('A boot loop after an update is usually fixed by switching slot in Recovery.') : undefined,
      recovery: ab });

    rows.push({ label: i18n.t('Dynamic partitions'), value: yn(p('ro.boot.dynamic_partitions')), tone: 'neutral' });
    rows.push({ label: i18n.t('Project Treble'), value: yn(p('ro.treble.enabled')), tone: 'neutral',
      hint: p('ro.treble.enabled') === 'true' ? i18n.t('Supports generic GSI system images.') : undefined });

    const ct = p('ro.crypto.type');
    rows.push({ label: i18n.t('Storage encryption'), value: ct ? ct.toUpperCase() : i18n.t('Unknown'), tone: ct ? 'good' : 'neutral' });

    if (selinux) rows.push({ label: 'SELinux', value: selinux, tone: /permissive/i.test(selinux) ? 'warn' : 'good',
      hint: /permissive/i.test(selinux) ? i18n.t('Permissive is unusual on a stock, unmodified device.') : undefined });

    const soc = p('ro.soc.model') || p('ro.board.platform');
    if (soc) rows.push({ label: i18n.t('Chipset'), value: soc, tone: 'neutral' });
    if (kernel) rows.push({ label: i18n.t('Kernel'), value: kernel, tone: 'neutral' });

    return rows;
  });

  const deviceName = $derived(props ? `${p('ro.product.manufacturer')} ${p('ro.product.model')}`.trim() : '');
  const fingerprint = $derived(p('ro.build.fingerprint'));

  async function copyText(t: string) { try { await navigator.clipboard.writeText(t); msg = i18n.t('Copied to clipboard.'); terr = null; } catch { /* clipboard blocked */ } }

  // Quick tweaks (no root)
  let fontScale = $state('1.0');
  let showTouches = $state(false);
  let pointerLoc = $state(false);
  let qsTiles = $state<string[]>([]);
  let qsAdd = $state('');

  const FONTS = [
    { v: '0.85', label: i18n.t('Small') },
    { v: '1.0',  label: i18n.t('Default') },
    { v: '1.15', label: i18n.t('Large') },
    { v: '1.30', label: i18n.t('Largest') },
  ];
  const QS_PRESET = ['internet','wifi','bt','flashlight','dnd','rotation','battery','airplane','location','hotspot','saver','dark','screenrecord','night_display','reduce_brightness','onehanded','cast','nfc','qr_code_scanner','font_scaling','color_correction'];

  async function loadTweaks() {
    if (!ready) return;
    try {
      const fs = (await api.runShell(dev!.serial, 'settings get system font_scale')).trim();
      fontScale = (!fs || fs === 'null') ? '1.0' : fs;
      showTouches = (await api.runShell(dev!.serial, 'settings get system show_touches')).trim() === '1';
      pointerLoc = (await api.runShell(dev!.serial, 'settings get system pointer_location')).trim() === '1';
      const tiles = (await api.runShell(dev!.serial, 'settings get secure sysui_qs_tiles')).trim();
      qsTiles = (!tiles || tiles === 'null') ? [] : tiles.split(',').map((s) => s.trim()).filter(Boolean);
    } catch { /* leave defaults */ }
  }

  async function act(cmd: string, ok: string) {
    if (!ready) return;
    busy = true; msg = null; terr = null;
    try { await api.runShell(dev!.serial, cmd); msg = ok; }
    catch (e) { terr = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  function setFont(v: string) { fontScale = v; act(`settings put system font_scale ${v}`, i18n.t('Font scale applied ({{v}}).', { v })); }
  function toggleTouches() { const nv = !showTouches; showTouches = nv; act(`settings put system show_touches ${nv ? 1 : 0}`, i18n.t('Applied.')); }
  function togglePointer() { const nv = !pointerLoc; pointerLoc = nv; act(`settings put system pointer_location ${nv ? 1 : 0}`, i18n.t('Applied.')); }

  async function immersive(mode: 'full' | 'status' | 'navigation' | 'off') {
    if (!ready) return;
    busy = true; msg = null; terr = null;
    try { await api.setImmersiveMode(dev!.serial, mode); msg = i18n.t('Immersive mode: {{m}}.', { m: mode }); }
    catch (e) { terr = (e as DozeForgeError).message; }
    finally { busy = false; }
  }

  function removeTile(t: string) { qsTiles = qsTiles.filter((x) => x !== t); }
  function addTile() { if (qsAdd && !qsTiles.includes(qsAdd)) { qsTiles = [...qsTiles, qsAdd]; qsAdd = ''; } }
  function applyTiles() { act(`settings put secure sysui_qs_tiles "${qsTiles.join(',')}"`, i18n.t('Quick Settings tiles updated.')); }
  function resetTiles() { act('settings delete secure sysui_qs_tiles', i18n.t('Quick Settings tiles reset to default.')); setTimeout(loadTweaks, 500); }

  // ───────────── Performance & cleanup (no root) ─────────────
  let dontKeep = $state(false);
  let wifiScan = $state(true);
  let bleScan = $state(true);
  let screenOff = $state('');
  const SCREEN_OFF = [
    { v: '15000', label: '15 s' }, { v: '30000', label: '30 s' }, { v: '60000', label: '1 min' },
    { v: '120000', label: '2 min' }, { v: '300000', label: '5 min' }, { v: '600000', label: '10 min' }, { v: '1800000', label: '30 min' },
  ];

  async function loadPerf() {
    if (!ready) return;
    try {
      dontKeep = (await api.runShell(dev!.serial, 'settings get global always_finish_activities')).trim() === '1';
      wifiScan = (await api.runShell(dev!.serial, 'settings get global wifi_scan_always_enabled')).trim() !== '0';
      bleScan = (await api.runShell(dev!.serial, 'settings get global ble_scan_always_enabled')).trim() !== '0';
      const so = (await api.runShell(dev!.serial, 'settings get system screen_off_timeout')).trim();
      screenOff = (!so || so === 'null') ? '' : so;
    } catch { /* leave defaults */ }
  }
  function toggleDontKeep() { const nv = !dontKeep; dontKeep = nv; act(`settings put global always_finish_activities ${nv ? 1 : 0}`, i18n.t('Applied.')); }
  function toggleWifiScan() { const nv = !wifiScan; wifiScan = nv; act(`settings put global wifi_scan_always_enabled ${nv ? 1 : 0}`, i18n.t('Applied.')); }
  function toggleBleScan() { const nv = !bleScan; bleScan = nv; act(`settings put global ble_scan_always_enabled ${nv ? 1 : 0}`, i18n.t('Applied.')); }
  function setScreenOff(v: string) { screenOff = v; if (v) act(`settings put system screen_off_timeout ${v}`, i18n.t('Applied.')); }
  function fixedPerf(on: boolean) { act(`cmd power set-fixed-performance-mode-enabled ${on ? 'true' : 'false'}`, on ? i18n.t('Max performance ON — clocks pinned high (drains battery).') : i18n.t('Max performance off.')); }
  function trimCaches() { act('pm trim-caches 9999999999', i18n.t('Trimmed cached files across apps.')); }

  // ───────────── Root tweaks (su -c) ─────────────
  let hasRoot = $state<boolean | null>(null);
  let governor = $state(''); let governorsAvail = $state<string[]>([]);
  let ioSched = $state(''); let ioSchedAvail = $state<string[]>([]);
  let swappiness = $state('');
  let tcpCc = $state(''); let tcpAvail = $state<string[]>([]);
  let selinuxEnf = $state(true);

  let rootTried = $state(false);
  async function detectRoot() { if (!ready) return; try { hasRoot = await api.checkRoot(dev!.serial); } catch { hasRoot = false; } }
  function bracket(s: string) { const m = /\[([^\]]+)\]/.exec(s); return m ? m[1] : ''; }
  async function loadRoot() {
    if (!ready || !hasRoot || rootTried) return;
    rootTried = true;
    try {
      governor = (await api.runShell(dev!.serial, "su -c 'cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor'")).trim();
      governorsAvail = (await api.runShell(dev!.serial, "su -c 'cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_available_governors'")).trim().split(/\s+/).filter(Boolean);
      const sch = (await api.runShell(dev!.serial, "su -c 'cat /sys/block/sda/queue/scheduler 2>/dev/null || cat /sys/block/mmcblk0/queue/scheduler 2>/dev/null'")).trim();
      ioSched = bracket(sch) ?? ''; ioSchedAvail = sch.replace(/[\[\]]/g, '').split(/\s+/).filter(Boolean);
      if (!ioSched && ioSchedAvail.length) ioSched = ioSchedAvail[0] ?? '';
      swappiness = (await api.runShell(dev!.serial, "su -c 'cat /proc/sys/vm/swappiness'")).trim();
      tcpCc = (await api.runShell(dev!.serial, "su -c 'cat /proc/sys/net/ipv4/tcp_congestion_control'")).trim();
      tcpAvail = (await api.runShell(dev!.serial, "su -c 'cat /proc/sys/net/ipv4/tcp_available_congestion_control'")).trim().split(/\s+/).filter(Boolean);
      selinuxEnf = !/permissive/i.test((selinux ?? '') || (await api.runShell(dev!.serial, 'getenforce')));
    } catch { /* partial reads ok */ }
  }
  function actRoot(inner: string, ok: string) { act(`su -c '${inner}'`, ok); }
  function setGovernor(g: string) { governor = g; actRoot(`for f in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo ${g} > $f; done`, i18n.t('CPU governor: {{g}}.', { g })); }
  function setIoSched(s: string) { ioSched = s; actRoot(`for f in /sys/block/*/queue/scheduler; do echo ${s} > $f 2>/dev/null; done`, i18n.t('I/O scheduler: {{s}}.', { s })); }
  function setSwap(v: string) { swappiness = v; actRoot(`echo ${v} > /proc/sys/vm/swappiness`, i18n.t('Swappiness: {{v}}.', { v })); }
  function setTcp(cc: string) { tcpCc = cc; actRoot(`echo ${cc} > /proc/sys/net/ipv4/tcp_congestion_control`, i18n.t('TCP congestion control: {{cc}}.', { cc })); }
  function dropCaches() { actRoot('sync; echo 3 > /proc/sys/vm/drop_caches', i18n.t('Caches dropped.')); }
  function doFstrim() {
    if (!ready) return;
    busy = true; msg = null; terr = null;
    api.runFstrim(dev!.serial).then(() => { msg = i18n.t('fstrim done — freed block-level space.'); }).catch((e) => { terr = (e as DozeForgeError).message; }).finally(() => { busy = false; });
  }
  function setSelinux(enf: boolean) {
    if (!enf && !confirm(i18n.t('Permissive SELinux lowers security and can break banking apps / Play Integrity. Continue?'))) return;
    selinuxEnf = enf;
    actRoot(`setenforce ${enf ? 1 : 0}`, enf ? i18n.t('SELinux: Enforcing.') : i18n.t('SELinux: Permissive.'));
  }

  // I/O (root only)
  let ioStats = $state<any[]>([]);
  let ioLoading = $state(false);
  let ioError = $state<string | null>(null);
  async function loadIoStats() {
    if (!ready) return;
    ioLoading = true; ioError = null;
    try { ioStats = await api.getIoStats(dev!.serial); }
    catch (e) { ioError = (e as DozeForgeError).message; }
    finally { ioLoading = false; }
  }

  onMount(() => { if (ready) { loadProps(); loadTweaks(); loadPerf(); detectRoot(); } });
  $effect(() => {
    if (!ready) return;
    if ((activeTab === 'diag' || activeTab === 'props') && !props && !propsLoading) loadProps();
    if (activeTab === 'io' && hasRoot && ioStats.length === 0 && !ioLoading) loadIoStats();
    if (activeTab === 'root' && hasRoot && !rootTried) loadRoot();
  });

  const visibleProps = $derived.by(() => {
    if (!props) return [] as [string, string][];
    if (!propsFilter) return Object.entries(props);
    const q = propsFilter.toLowerCase();
    return Object.entries(props).filter(([k, v]) => k.toLowerCase().includes(q) || v.toLowerCase().includes(q));
  });

  function esc(s: string) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }
  function highlight(text: string, search: string) {
    if (!search) return esc(text);
    const idx = text.toLowerCase().indexOf(search.toLowerCase());
    if (idx === -1) return esc(text);
    return `${esc(text.slice(0, idx))}<mark>${esc(text.slice(idx, idx + search.length))}</mark>${esc(text.slice(idx + search.length))}`;
  }
  function copyProp(k: string, v: string) { copyText(`${k}=${v}`); }
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('System')}</h1>
    <p class="muted">{i18n.t('Device diagnostics and no-root quick tweaks over ADB.')}</p>
  </div>
</header>

<div class="tabs">
  <button class:active={activeTab === 'diag'} onclick={() => activeTab = 'diag'}>{i18n.t('Diagnostics')}</button>
  <button class:active={activeTab === 'tweaks'} onclick={() => activeTab = 'tweaks'}>{i18n.t('Quick tweaks')}</button>
  <button class:active={activeTab === 'root'} onclick={() => activeTab = 'root'}>{i18n.t('Root')} <span class="root-tag">root</span></button>
  <button class:active={activeTab === 'props'} onclick={() => activeTab = 'props'}>{i18n.t('Build Props')}</button>
  <button class:active={activeTab === 'io'} onclick={() => activeTab = 'io'}>{i18n.t('Storage I/O')} <span class="root-tag">root</span></button>
</div>

{#if msg}<div class="ok-banner">{msg}</div>{/if}
{#if terr}<div class="error">{terr}</div>{/if}

<div class="tab-content">
  {#if !ready}
    <div class="card empty"><p class="muted">{i18n.t('No device connected.')}</p></div>

  {:else if activeTab === 'diag'}
    {#if propsLoading && !props}
      <div class="card p-card"><Skeleton lines={8} /></div>
    {:else if propsError}
      <div class="error">{propsError}</div>
    {:else}
      <div class="card summary">
        <div class="sum-main">
          <div class="sum-name">{deviceName}</div>
          <div class="muted small mono fp">{fingerprint}</div>
        </div>
        <button class="btn outline small" onclick={() => copyText(fingerprint)}>{i18n.t('Copy fingerprint')}</button>
      </div>

      <div class="card diag-card">
        {#each diagnostics as d (d.label)}
          <div class="diag-row">
            <span class="diag-dot" data-tone={d.tone}></span>
            <div class="diag-body">
              <div class="diag-line">
                <span class="diag-label">{d.label}</span>
                <span class="diag-value" data-tone={d.tone}>{d.value}</span>
              </div>
              {#if d.hint}
                <p class="diag-hint">
                  {d.hint}
                  {#if d.recovery}<a class="diag-link" href="/recovery/">{i18n.t('Open Recovery')}</a>{/if}
                </p>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}

  {:else if activeTab === 'tweaks'}
    <div class="tweak-grid">
      <div class="card">
        <h3>{i18n.t('Font scale')}</h3>
        <p class="muted small">{i18n.t('Resize all system and app text. Reversible.')}</p>
        <div class="seg">
          {#each FONTS as f (f.v)}
            <button class="seg-btn" class:on={fontScale === f.v} disabled={busy} onclick={() => setFont(f.v)}>
              {f.label}<span class="seg-sub">{f.v}×</span>
            </button>
          {/each}
        </div>
      </div>

      <div class="card">
        <h3>{i18n.t('Immersive mode')}</h3>
        <p class="muted small">{i18n.t('Hide the status and/or navigation bars system-wide. Some Xiaomi/MIUI builds ignore it.')}</p>
        <div class="btn-row wrap">
          <button class="btn outline" disabled={busy} onclick={() => immersive('full')}>{i18n.t('Hide both')}</button>
          <button class="btn outline" disabled={busy} onclick={() => immersive('status')}>{i18n.t('Hide status bar')}</button>
          <button class="btn outline" disabled={busy} onclick={() => immersive('navigation')}>{i18n.t('Hide nav bar')}</button>
          <button class="btn outline" disabled={busy} onclick={() => immersive('off')}>{i18n.t('Off')}</button>
        </div>
      </div>

      <div class="card">
        <h3>{i18n.t('Screen demo helpers')}</h3>
        <p class="muted small">{i18n.t('Handy for screen recordings and tutorials.')}</p>
        <label class="opt"><input type="checkbox" checked={showTouches} disabled={busy} onchange={toggleTouches} /><span>{i18n.t('Show taps (visual touch feedback)')}</span></label>
        <label class="opt"><input type="checkbox" checked={pointerLoc} disabled={busy} onchange={togglePointer} /><span>{i18n.t('Pointer location overlay')}</span></label>
      </div>

      <div class="card">
        <h3>{i18n.t('Performance & cleanup')}</h3>
        <p class="muted small">{i18n.t('No root. Developer-option tweaks and cache cleanup.')}</p>
        <label class="opt"><input type="checkbox" checked={dontKeep} disabled={busy} onchange={toggleDontKeep} /><span>{i18n.t("Don't keep activities (frees RAM, hurts multitasking)")}</span></label>
        <label class="opt"><input type="checkbox" checked={wifiScan} disabled={busy} onchange={toggleWifiScan} /><span>{i18n.t('Wi-Fi always scanning')}</span></label>
        <label class="opt"><input type="checkbox" checked={bleScan} disabled={busy} onchange={toggleBleScan} /><span>{i18n.t('Bluetooth always scanning')}</span></label>
        <div class="field">
          <span class="field-label">{i18n.t('Screen-off timeout')}</span>
          <select value={screenOff} onchange={(e) => setScreenOff((e.currentTarget as HTMLSelectElement).value)} disabled={busy}>
            <option value="">{i18n.t('Keep current')}</option>
            {#each SCREEN_OFF as s (s.v)}<option value={s.v}>{s.label}</option>{/each}
          </select>
        </div>
        <div class="btn-row wrap" style="margin-top: 0.9rem;">
          <button class="btn outline" disabled={busy} onclick={() => fixedPerf(true)}>{i18n.t('Max performance ON')}</button>
          <button class="btn outline" disabled={busy} onclick={() => fixedPerf(false)}>{i18n.t('Off')}</button>
          <button class="btn outline" disabled={busy} onclick={trimCaches}>{i18n.t('Trim caches')}</button>
        </div>
      </div>

      <div class="card qs-card">
        <h3>{i18n.t('Quick Settings tiles')}</h3>
        <p class="muted small">{i18n.t('Order below sets the tile order. Unknown specs are ignored by the phone.')}</p>
        <div class="chips">
          {#each qsTiles as t (t)}
            <span class="chip">{t}<button class="chip-x" onclick={() => removeTile(t)} aria-label="remove">×</button></span>
          {/each}
          {#if qsTiles.length === 0}<span class="muted small">{i18n.t('Could not read current tiles (or none).')}</span>{/if}
        </div>
        <div class="qs-actions">
          <select bind:value={qsAdd}>
            <option value="">{i18n.t('Add tile…')}</option>
            {#each QS_PRESET.filter((t) => !qsTiles.includes(t)) as t (t)}<option value={t}>{t}</option>{/each}
          </select>
          <button class="btn outline small" disabled={!qsAdd} onclick={addTile}>{i18n.t('Add')}</button>
          <button class="primary small" disabled={busy || qsTiles.length === 0} onclick={applyTiles}>{i18n.t('Apply order')}</button>
          <button class="btn outline small" disabled={busy} onclick={resetTiles}>{i18n.t('Reset to default')}</button>
        </div>
      </div>
    </div>

  {:else if activeTab === 'root'}
    {#if hasRoot === null}
      <div class="card p-card"><Skeleton lines={4} /></div>
    {:else if !hasRoot}
      <div class="card empty">
        <p class="muted">{i18n.t('No root detected on this device.')}</p>
        <p class="muted small">{i18n.t('These tweaks need root (Magisk/KernelSU). Once shell has su, they work here.')}</p>
        <button class="btn outline small" style="margin-top: 0.8rem;" onclick={detectRoot}>{i18n.t('Re-check')}</button>
      </div>
    {:else}
      <div class="root-banner">{i18n.t('Root granted — these write directly to the kernel.')}</div>
      <div class="tweak-grid">
        <div class="card">
          <h3>{i18n.t('CPU governor')} <span class="muted small">({governor || '?'})</span></h3>
          <p class="muted small">{i18n.t('performance = max, powersave = min, schedutil = balanced.')}</p>
          <select value={governor} onchange={(e) => setGovernor((e.currentTarget as HTMLSelectElement).value)} disabled={busy}>
            {#if governorsAvail.length === 0 && governor}<option value={governor}>{governor}</option>{/if}
            {#each governorsAvail as g (g)}<option value={g}>{g}</option>{/each}
          </select>
        </div>
        <div class="card">
          <h3>{i18n.t('I/O scheduler')} <span class="muted small">({ioSched || '?'})</span></h3>
          <select value={ioSched} onchange={(e) => setIoSched((e.currentTarget as HTMLSelectElement).value)} disabled={busy}>
            {#each ioSchedAvail as s (s)}<option value={s}>{s}</option>{/each}
          </select>
        </div>
        <div class="card">
          <h3>{i18n.t('TCP congestion control')} <span class="muted small">({tcpCc || '?'})</span></h3>
          <p class="muted small">{i18n.t('bbr often improves throughput on mobile networks.')}</p>
          <select value={tcpCc} onchange={(e) => setTcp((e.currentTarget as HTMLSelectElement).value)} disabled={busy}>
            {#if tcpAvail.length === 0 && tcpCc}<option value={tcpCc}>{tcpCc}</option>{/if}
            {#each tcpAvail as c (c)}<option value={c}>{c}</option>{/each}
          </select>
        </div>
        <div class="card">
          <h3>{i18n.t('RAM swappiness')} <span class="muted small">({swappiness || '?'})</span></h3>
          <p class="muted small">{i18n.t('Lower keeps apps in RAM; higher leans on zram/swap.')}</p>
          <div class="seg">
            {#each ['0', '10', '60', '100'] as v (v)}
              <button class="seg-btn" class:on={swappiness === v} disabled={busy} onclick={() => setSwap(v)}>{v}</button>
            {/each}
          </div>
        </div>
        <div class="card">
          <h3>{i18n.t('Memory & storage')}</h3>
          <p class="muted small">{i18n.t('Free RAM caches and trim unused storage blocks.')}</p>
          <div class="btn-row wrap">
            <button class="btn outline" disabled={busy} onclick={dropCaches}>{i18n.t('Drop caches')}</button>
            <button class="btn outline" disabled={busy} onclick={doFstrim}>{i18n.t('Run fstrim')}</button>
          </div>
        </div>
        <div class="card">
          <h3>SELinux <span class="muted small">({selinuxEnf ? 'Enforcing' : 'Permissive'})</span></h3>
          <p class="muted small">{i18n.t('Permissive can break banking apps and Play Integrity. Debugging only.')}</p>
          <div class="btn-row wrap">
            <button class="btn outline" class:on={selinuxEnf} disabled={busy} onclick={() => setSelinux(true)}>{i18n.t('Enforcing')}</button>
            <button class="btn outline" class:on={!selinuxEnf} disabled={busy} onclick={() => setSelinux(false)}>{i18n.t('Permissive')}</button>
          </div>
        </div>
      </div>
    {/if}

  {:else if activeTab === 'props'}
    <div class="card p-card">
      <div class="props-head">
        <input type="search" placeholder={i18n.t('Filter props...')} bind:value={propsFilter} />
        <span class="muted small">{visibleProps.length} {i18n.t('props found')}</span>
      </div>
      {#if propsLoading && !props}
        <Skeleton lines={10} />
      {:else if propsError}
        <div class="error">{propsError}</div>
      {:else}
        <div class="table-container" style="max-height: 62vh; overflow-y: auto;">
          <table class="data-table prop-table">
            <thead><tr><th>{i18n.t('Key')}</th><th>{i18n.t('Value')}</th></tr></thead>
            <tbody>
              {#each visibleProps as [k, v] (k)}
                <tr onclick={() => copyProp(k, v)} title={i18n.t('Click to copy')}>
                  <td class="mono prop-key">{@html highlight(k, propsFilter)}</td>
                  <td class="mono prop-val">{@html highlight(v, propsFilter)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>

  {:else if activeTab === 'io'}
    <div class="card p-card">
      <div class="props-head">
        <div>
          <h3>{i18n.t('UFS Storage Degradation Monitor')}</h3>
          <p class="muted small">{i18n.t('Cumulative read/write bytes per app.')} <strong>{i18n.t('Requires root (Magisk/KernelSU).')}</strong></p>
        </div>
        <button class="btn outline small" onclick={loadIoStats} disabled={ioLoading || !hasRoot}>{i18n.t('Refresh')}</button>
      </div>
      {#if hasRoot === false}
        <div class="card empty">
          <p class="muted">{i18n.t('This device is not rooted — per-app I/O stats are unavailable.')}</p>
          <p class="muted small">{i18n.t('It reads /proc/uid_io/stats, which needs root (Magisk/KernelSU) and a compatible kernel.')}</p>
        </div>
      {:else if hasRoot === null}
        <p class="muted small" style="padding: 0.6rem;">{i18n.t('Checking root access…')}</p>
      {:else if ioLoading && ioStats.length === 0}
        <Skeleton lines={5} />
      {:else if ioError}
        <div class="error">{ioError}</div>
      {:else if ioStats.length > 0}
        <div class="table-container" style="max-height: 58vh; overflow-y: auto;">
          <table class="data-table">
            <thead><tr><th>{i18n.t('UID')}</th><th>{i18n.t('FG Read')}</th><th>{i18n.t('FG Write')}</th><th>{i18n.t('BG Read')}</th><th>{i18n.t('BG Write')}</th></tr></thead>
            <tbody>
              {#each ioStats.slice().sort((a, b) => (b.bg_write_bytes + b.fg_write_bytes) - (a.bg_write_bytes + a.fg_write_bytes)).slice(0, 50) as stat (stat.uid)}
                <tr>
                  <td class="mono">{stat.uid}</td>
                  <td class="mono">{(stat.fg_read_bytes / 1048576).toFixed(1)} MB</td>
                  <td class="mono">{(stat.fg_write_bytes / 1048576).toFixed(1)} MB</td>
                  <td class="mono">{(stat.bg_read_bytes / 1048576).toFixed(1)} MB</td>
                  <td class="mono" style="color: var(--warn);">{(stat.bg_write_bytes / 1048576).toFixed(1)} MB</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <div class="card empty">
          <p class="muted">{i18n.t('No I/O data — this needs root and a kernel exposing')} <code>/proc/uid_io/stats</code>.</p>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .page-head { margin-bottom: 1.5rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; max-width: 560px; }

  .tabs { display: flex; gap: 0.4rem; margin-bottom: 1.25rem; border-bottom: 1px solid var(--border); flex-wrap: wrap; }
  .tabs button { background: transparent; border: none; padding: 0.5rem 0.9rem; color: var(--fg-2); border-bottom: 2px solid transparent; font-weight: 500; cursor: pointer; border-radius: 0; }
  .tabs button:hover { color: var(--fg-0); }
  .tabs button.active { color: var(--accent); border-bottom-color: var(--accent); }
  .root-tag { font-size: 9px; text-transform: uppercase; letter-spacing: 0.08em; background: var(--bg-3); color: var(--fg-3); padding: 1px 5px; border-radius: 5px; margin-left: 2px; }

  .ok-banner { background: var(--good-soft); color: var(--good); border: 1px solid rgba(16,185,129,0.3); padding: 0.7rem 1rem; border-radius: var(--radius); font-size: var(--font-size-sm); margin-bottom: 1rem; }
  .error { padding: 0.65rem 1rem; background: rgba(239,68,68,0.1); border-left: 3px solid var(--bad); border-radius: var(--radius); color: var(--bad); margin-bottom: 1rem; }

  .summary { display: flex; align-items: center; justify-content: space-between; gap: 1rem; margin-bottom: 1rem; }
  .sum-name { font-family: var(--font-display); font-weight: 700; font-size: 1.2rem; color: var(--fg-0); }
  .fp { margin-top: 0.2rem; word-break: break-all; }
  .diag-card { display: flex; flex-direction: column; }
  .diag-row { display: flex; gap: 0.8rem; padding: 0.8rem 0; border-bottom: 1px solid var(--hairline); }
  .diag-row:last-child { border-bottom: none; }
  .diag-dot { width: 9px; height: 9px; border-radius: 50%; margin-top: 0.42rem; flex-shrink: 0; background: var(--fg-3); }
  .diag-dot[data-tone="good"] { background: var(--good); box-shadow: 0 0 7px var(--good); }
  .diag-dot[data-tone="warn"] { background: var(--warn); box-shadow: 0 0 7px var(--warn); }
  .diag-dot[data-tone="bad"]  { background: var(--bad);  box-shadow: 0 0 7px var(--bad); }
  .diag-body { flex: 1; min-width: 0; }
  .diag-line { display: flex; align-items: baseline; justify-content: space-between; gap: 1rem; }
  .diag-label { color: var(--fg-2); font-size: var(--font-size-sm); }
  .diag-value { font-weight: 600; color: var(--fg-0); font-size: var(--font-size-sm); text-align: right; word-break: break-word; }
  .diag-value[data-tone="good"] { color: var(--good); }
  .diag-value[data-tone="warn"] { color: var(--warn); }
  .diag-value[data-tone="bad"]  { color: var(--bad); }
  .diag-hint { margin: 0.3rem 0 0; font-size: var(--font-size-xs); color: var(--fg-3); line-height: 1.5; }
  .diag-link { color: var(--accent); margin-left: 0.4rem; }

  .tweak-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 1rem; align-items: start; }
  .tweak-grid h3 { margin-bottom: 0.3rem; }
  .seg { display: flex; gap: 0.5rem; margin-top: 0.8rem; flex-wrap: wrap; }
  .seg-btn { display: flex; flex-direction: column; align-items: center; gap: 0.15rem; padding: 0.55rem 0.9rem; background: var(--bg-2); border: 1px solid var(--hairline); border-radius: 12px; color: var(--fg-1); cursor: pointer; font-size: var(--font-size-sm); font-weight: 600; }
  .seg-btn:hover:not(:disabled) { border-color: var(--card-hover-border); }
  .seg-btn.on { background: var(--accent-soft); border-color: var(--accent); color: var(--accent); }
  .seg-sub { font-size: 10px; color: var(--fg-3); font-weight: 500; }
  .seg-btn.on .seg-sub { color: var(--accent); }
  .btn-row { display: flex; gap: 0.5rem; margin-top: 0.8rem; }
  .btn-row.wrap { flex-wrap: wrap; }
  .opt { display: flex; align-items: center; gap: 0.55rem; margin-top: 0.7rem; font-size: var(--font-size-sm); color: var(--fg-1); cursor: pointer; }
  .opt input { width: auto; }

  .qs-card { grid-column: 1 / -1; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; margin: 0.8rem 0; min-height: 2rem; }
  .chip { display: inline-flex; align-items: center; gap: 0.35rem; padding: 0.25rem 0.3rem 0.25rem 0.6rem; background: var(--bg-2); border: 1px solid var(--hairline); border-radius: 99px; font-size: var(--font-size-xs); font-family: var(--font-mono); color: var(--fg-1); }
  .chip-x { border: none; background: transparent; color: var(--fg-3); cursor: pointer; font-size: 15px; line-height: 1; padding: 0 3px; border-radius: 50%; }
  .chip-x:hover { color: var(--bad); }
  .qs-actions { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .qs-actions select { width: auto; min-width: 160px; }

  .small { font-size: var(--font-size-sm); padding: 0.4rem 0.8rem; }

  .p-card { min-height: 380px; }
  .props-head { display: flex; gap: 1rem; margin-bottom: 1rem; align-items: center; justify-content: space-between; }
  .props-head input[type="search"] { flex: 1; max-width: 320px; }
  .prop-table { font-size: 11.5px; }
  .prop-table thead th { position: sticky; top: 0; background: var(--bg-1); z-index: 10; }
  .prop-table tr:hover { background: var(--bg-hover); cursor: copy; }
  .prop-key { color: var(--fg-1); }
  .prop-val { color: var(--good); max-width: 420px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  :global(mark) { background: rgba(255,107,0,0.3); color: inherit; padding: 0 2px; border-radius: 2px; }

  .field { margin-top: 0.9rem; }
  .field-label { display: block; font-size: 10px; text-transform: uppercase; letter-spacing: 0.1em; color: var(--fg-3); font-weight: 700; margin-bottom: 0.35rem; }
  .field select { width: 100%; }
  .card > select { width: 100%; margin-top: 0.5rem; }
  .root-banner { background: var(--accent-soft); border: 1px solid var(--card-hover-border); color: var(--accent); padding: 0.6rem 0.9rem; border-radius: var(--radius); font-size: var(--font-size-sm); font-weight: 600; margin-bottom: 1rem; }
  .btn.outline.on { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }
</style>
