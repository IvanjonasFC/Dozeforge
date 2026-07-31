<script lang="ts">
  import { onMount } from 'svelte';
  import { deviceStore } from '$stores/device.svelte';
  import { api, DozeForgeError } from '$tauri/api';
  import { i18n } from '$stores/i18n.svelte';

  let busy = $state(false);
  let msg = $state<string | null>(null);
  let err = $state<string | null>(null);

  const dev = $derived(deviceStore.selected);
  const devState = $derived(dev?.state ?? null);
  // Reboot modes only work with a responsive device (not offline/unauthorized/bootloader).
  const canReboot = $derived(!!dev && (devState === 'device' || devState === 'recovery' || devState === 'sideload'));

  // Per-state explanation + tone for the status banner.
  const info = $derived.by(() => {
    switch (devState) {
      case 'device':
        return { tone: 'good', title: i18n.t('Device is booting normally'),
          detail: i18n.t('The system booted fine. Use the controls below only if you need to enter recovery, bootloader or sideload on purpose.') };
      case 'unauthorized':
        return { tone: 'warn', title: i18n.t('Debugging not authorized'),
          detail: i18n.t('Unlock the phone and accept the "Allow USB debugging" prompt, then reconnect.') };
      case 'offline':
        return { tone: 'bad', title: i18n.t('Device is offline'),
          detail: i18n.t('No response over ADB. Replug the cable (or reconnect over Wi-Fi) and retry.') };
      case 'recovery':
        return { tone: 'warn', title: i18n.t('In recovery mode'),
          detail: i18n.t('You can sideload an official OTA package or reboot back to the system.') };
      case 'sideload':
        return { tone: 'warn', title: i18n.t('Waiting for sideload'),
          detail: i18n.t('Recovery is waiting for a package. Choose a file below to apply it.') };
      case 'bootloader':
        return { tone: 'warn', title: i18n.t('In fastboot / bootloader'),
          detail: i18n.t('The device is in fastboot. Reboot back to the system, or use fastboot from Toolbox.') };
      default:
        return { tone: 'neutral', title: i18n.t('No device connected'),
          detail: i18n.t('Plug in a phone. If it is stuck in a boot loop, connect it while holding the volume keys to enter recovery/fastboot.') };
    }
  });

  // Turns a raw exec failure (fastboot/adb missing from PATH) into a clear hint.
  function toMsg(e: unknown): string {
    const m = (e as DozeForgeError)?.message ?? String(e);
    if (/exec failed|not found|no such file|cannot find|program not found/i.test(m)) {
      return i18n.t('fastboot / adb not found. Install Android platform-tools and add it to your PATH.');
    }
    return m;
  }

  async function run(fn: () => Promise<unknown>, okMsg: string) {
    busy = true; msg = null; err = null;
    try { await fn(); msg = okMsg; }
    catch (e) { err = toMsg(e); }
    finally { busy = false; }
  }

  function reboot(mode: string, label: string, danger = false) {
    if (!dev) return;
    if (danger && !confirm(i18n.t('This is an advanced action that can be risky on some devices. Continue?'))) return;
    run(() => api.rebootDevice(dev!.serial, mode), i18n.t('Rebooting to {{mode}}…', { mode: label }));
  }

  function fastbootToSystem() {
    if (!dev) return;
    run(() => api.fastbootReboot(dev!.serial), i18n.t('Rebooting to system from fastboot…'));
  }

  async function sideloadPackage() {
    if (!dev) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const sel = await open({ multiple: false, filters: [{ name: 'OTA / APK', extensions: ['zip', 'apk'] }] });
    if (!sel || Array.isArray(sel)) return;
    run(() => api.sideloadApk(dev!.serial, sel as string), i18n.t('Sideloading package…'));
  }

  function reconnect() { deviceStore.refresh(); }

  async function openUrl(url: string) {
    try { const { open } = await import('@tauri-apps/plugin-shell'); await open(url); } catch { /* shell may be blocked; URL is shown as text too */ }
  }

  // ── Device identity for the stock-firmware finder ──
  let props = $state<Record<string, string> | null>(null);
  async function loadProps() {
    if (!dev || dev.state !== 'device') return;
    try { props = await api.getSystemProperties(dev.serial); } catch { /* offline/bootloader: fall back to the device list */ }
  }
  onMount(loadProps);
  $effect(() => { if (dev?.state === 'device' && !props) loadProps(); });

  const model = $derived(props?.['ro.product.model'] ?? dev?.model ?? null);
  const maker = $derived((props?.['ro.product.manufacturer'] ?? dev?.manufacturer ?? '').toLowerCase());
  const fingerprint = $derived(props?.['ro.build.fingerprint'] ?? props?.['ro.bootimage.build.fingerprint'] ?? null);
  const androidVer = $derived(props?.['ro.build.version.release'] ?? null);

  const FIRMWARE = [
    { key: 'google',   name: 'Google Pixel / Nexus', how: 'Official Factory Images — flash with the bundled flash-all script (fastboot).', url: 'https://developers.google.com/android/images' },
    { key: 'samsung',  name: 'Samsung Galaxy',       how: 'NOT fastboot. Download firmware by model + CSC (SamFW) and flash with Odin in Download mode.', url: 'https://samfw.com' },
    { key: 'xiaomi',   name: 'Xiaomi / Redmi / POCO', how: 'Fastboot ROM flashed with MiFlash, or a recovery ROM.', url: 'https://xiaomifirmwareupdater.com' },
    { key: 'oneplus',  name: 'OnePlus',              how: 'Official OxygenOS build for your model (rollback / local upgrade); MSM Tool for hard bricks.', url: 'https://www.oneplus.com/support' },
    { key: 'motorola', name: 'Motorola',             how: 'Stock firmware + fastboot (flashfile.xml script).', url: 'https://mirrors.lolinet.com/firmware/moto/' },
  ];
  const matchedOem = $derived.by(() => {
    if (maker.includes('samsung')) return FIRMWARE.find((f) => f.key === 'samsung');
    if (maker.includes('google')) return FIRMWARE.find((f) => f.key === 'google');
    if (maker.includes('xiaomi') || maker.includes('redmi') || maker.includes('poco')) return FIRMWARE.find((f) => f.key === 'xiaomi');
    if (maker.includes('oneplus')) return FIRMWARE.find((f) => f.key === 'oneplus');
    if (maker.includes('motorola') || maker.includes('lenovo')) return FIRMWARE.find((f) => f.key === 'motorola');
    return null;
  });

  // ── Fastboot: flash a single partition (uses the existing command) ──
  const PARTITIONS = ['boot', 'boot_a', 'boot_b', 'init_boot', 'recovery', 'vbmeta', 'dtbo', 'vendor_boot', 'system', 'product'];
  let flashPart = $state('boot');
  async function flashPartition() {
    if (!dev) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const img = await open({ multiple: false, filters: [{ name: 'Partition image', extensions: ['img', 'bin'] }] });
    if (!img || Array.isArray(img)) return;
    const file = String(img).split(/[\\/]/).pop() ?? '';
    if (!confirm(i18n.t('Flash "{{file}}" to partition "{{part}}"? A wrong or mismatched image can brick the device.', { file, part: flashPart }))) return;
    run(() => api.fastbootFlash(dev!.serial, flashPart, img as string), i18n.t('Flashed partition {{part}}.', { part: flashPart }));
  }

  // ── Fastboot diagnostics + A/B slot switch (the #1 bootloop fix) ──
  let fbInfo = $state<string | null>(null);
  function diagnose() {
    if (!dev) return;
    busy = true; msg = null; err = null; fbInfo = null;
    api.fastbootGetvar(dev.serial)
      .then((t) => { fbInfo = t; })
      .catch((e) => { err = toMsg(e); })
      .finally(() => { busy = false; });
  }
  const currentSlot = $derived.by(() => {
    const m = fbInfo?.match(/current-slot:\s*_?([ab])/i);
    return m?.[1] ? m[1].toLowerCase() : null;
  });
  const unlocked = $derived.by(() => {
    const m = fbInfo?.match(/unlocked:\s*(yes|no|true|false)/i);
    return m?.[1] ? /yes|true/i.test(m[1]) : null;
  });
  function setSlot(slot: string) {
    if (!dev) return;
    if (!confirm(i18n.t('Switch active slot to {{slot}} and reboot? This is the usual fix for an update-induced boot loop.', { slot: slot.toUpperCase() }))) return;
    busy = true; msg = null; err = null;
    api.fastbootSetSlot(dev.serial, slot)
      .then(() => api.fastbootReboot(dev!.serial))
      .then(() => { msg = i18n.t('Switched to slot {{slot}} and rebooting…', { slot: slot.toUpperCase() }); })
      .catch((e) => { err = toMsg(e); })
      .finally(() => { busy = false; });
  }
  async function bootImage() {
    if (!dev) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const img = await open({ multiple: false, filters: [{ name: 'Boot / recovery image', extensions: ['img'] }] });
    if (!img || Array.isArray(img)) return;
    run(() => api.fastbootBoot(dev!.serial, img as string), i18n.t('Booting the image (no flash)…'));
  }

  const REBOOTS = [
    { mode: '',           label: 'System',     desc: 'Normal boot',                danger: false },
    { mode: 'recovery',   label: 'Recovery',   desc: 'Stock/custom recovery',      danger: false },
    { mode: 'bootloader', label: 'Bootloader', desc: 'Fastboot / flashing',        danger: false },
    { mode: 'fastboot',   label: 'Fastboot',   desc: 'Fastbootd (dynamic parts)',  danger: false },
    { mode: 'sideload',   label: 'Sideload',   desc: 'Apply an OTA over ADB',       danger: false },
    { mode: 'edl',        label: 'EDL',        desc: 'Qualcomm emergency download', danger: true  },
  ];
</script>

<header class="page-head">
  <div>
    <h1>{i18n.t('Recovery')}</h1>
    <p class="muted">{i18n.t('Get an unbootable or stuck device back to a safe state — no root, only public reboot modes.')}</p>
  </div>
</header>

{#if msg}<div class="ok-banner">{msg}</div>{/if}
{#if err}<div class="error">{err}</div>{/if}

<section class="card state-card" data-tone={info.tone}>
  <span class="s-dot" data-tone={info.tone}></span>
  <div class="s-body">
    <div class="s-title">{info.title}</div>
    <p class="s-detail">{info.detail}</p>
    <div class="s-actions">
      {#if devState === 'offline' || devState === 'unauthorized' || !dev}
        <button onclick={reconnect} disabled={busy}>{i18n.t('Reconnect')}</button>
      {/if}
      {#if devState === 'bootloader'}
        <button class="primary" onclick={fastbootToSystem} disabled={busy}>{i18n.t('Reboot to system (fastboot)')}</button>
      {/if}
    </div>
  </div>
</section>

{#snippet firmwareCard()}
<div class="card fw-card">
  <h3>{i18n.t('Get the stock ROM to recover this device')}</h3>
  {#if model}
    <p class="fw-id"><strong>{model}</strong>{maker ? ` · ${maker}` : ''}{androidVer ? ` · Android ${androidVer}` : ''}</p>
    {#if fingerprint}<p class="muted small mono fw-fp">{fingerprint}</p>{/if}
  {:else}
    <p class="muted small">{i18n.t('Connect the device over ADB to read its exact model and build.')}</p>
  {/if}
  {#if matchedOem}
    <div class="fw-oem">
      <div class="fw-oem-name">{matchedOem.name}</div>
      <p class="muted small">{i18n.t(matchedOem.how)}</p>
      <button class="fw-btn" onclick={() => openUrl(matchedOem!.url)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
        {i18n.t('Open firmware source')}
      </button>
      <span class="muted small mono fw-url">{matchedOem.url}</span>
    </div>
  {:else}
    <div class="fw-list">
      {#each FIRMWARE as f (f.key)}
        <div class="fw-item">
          <span class="fw-item-name">{f.name}</span>
          <button class="fw-btn sm" onclick={() => openUrl(f.url)}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
            {i18n.t('Open')}
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>
{/snippet}

{#if devState === 'bootloader'}
<div class="rec-grid">
  <div class="rec-main">
  <div class="card fb-card">
    <h3>{i18n.t('Fastboot toolkit')}</h3>
    <p class="muted small">{i18n.t('Bootloop rescue over fastboot. The A/B slot switch fixes most update-induced boot loops in seconds.')}</p>

    <div class="fb-row">
      <button onclick={diagnose} disabled={busy}>{i18n.t('Diagnose (read fastboot vars)')}</button>
      {#if currentSlot}<span class="fb-pill">{i18n.t('Slot')}: {currentSlot.toUpperCase()}</span>{/if}
      {#if unlocked !== null}<span class="fb-pill" class:warn={!unlocked}>{unlocked ? i18n.t('Bootloader unlocked') : i18n.t('Bootloader locked')}</span>{/if}
    </div>
    {#if fbInfo}<pre class="fb-out">{fbInfo}</pre>{/if}

    <div class="fb-sub">{i18n.t('Switch active slot (bootloop fix)')}</div>
    <div class="fb-row">
      <button onclick={() => setSlot('a')} disabled={busy}>{i18n.t('Switch to slot A + reboot')}</button>
      <button onclick={() => setSlot('b')} disabled={busy}>{i18n.t('Switch to slot B + reboot')}</button>
    </div>
    {#if fbInfo && !currentSlot}<p class="muted small">{i18n.t('This device reports no A/B slots (single-slot) — slot switching does not apply.')}</p>{/if}

    <div class="fb-sub">{i18n.t('Boot an image once (e.g. TWRP, without flashing)')}</div>
    <div class="fb-row">
      <button onclick={bootImage} disabled={busy}>{i18n.t('Choose image & boot…')}</button>
    </div>

    <div class="fb-sub">{i18n.t('Flash a partition')}</div>
    <p class="muted small">{i18n.t('Reflash a stock image (boot, recovery, vbmeta…) to fix a bad flash. Needs an unlocked bootloader. Use only images that match your exact model and build.')}</p>
    <div class="fb-row">
      <select bind:value={flashPart}>
        {#each PARTITIONS as p (p)}<option value={p}>{p}</option>{/each}
      </select>
      <button class="danger" onclick={flashPartition} disabled={busy}>{i18n.t('Choose image & flash…')}</button>
    </div>
  </div>
  </div>

  <div class="rec-side">
    {@render firmwareCard()}
  </div>
</div>
{:else if canReboot}
  <h3 class="section-title">{i18n.t('Reboot into…')}</h3>
  <div class="reboot-grid">
    {#each REBOOTS as r (r.label)}
      <button class="reboot-btn" class:danger={r.danger} disabled={busy}
        onclick={() => reboot(r.mode, i18n.t(r.label), r.danger)}>
        <span class="reboot-label">{i18n.t(r.label)}</span>
        <span class="reboot-desc">{i18n.t(r.desc)}</span>
      </button>
    {/each}
  </div>

  {#if devState === 'recovery' || devState === 'sideload'}
    <div class="card sideload-card">
      <h3>{i18n.t('Sideload an OTA / package')}</h3>
      <p class="muted small">{i18n.t('Apply an official OTA .zip (or APK) via adb sideload. Only use packages you trust — a bad image can brick the device.')}</p>
      <button onclick={sideloadPackage} disabled={busy}>{i18n.t('Choose file to sideload…')}</button>
    </div>
  {/if}

  <div class="fw-below">{@render firmwareCard()}</div>
{:else}
  {@render firmwareCard()}
{/if}

<style>
  .page-head { margin-bottom: 1.5rem; }
  .page-head h1 { margin-bottom: 0.25rem; letter-spacing: -0.025em; }
  .page-head p { margin: 0; }

  .ok-banner {
    background: var(--good-soft); color: var(--good);
    border: 1px solid rgba(16, 185, 129, 0.3);
    padding: 0.7rem 1rem; border-radius: var(--radius);
    font-size: var(--font-size-sm); margin-bottom: 1rem;
  }

  .state-card {
    display: flex; gap: 0.9rem; align-items: flex-start;
    margin-bottom: 1.5rem;
  }
  .s-dot {
    width: 10px; height: 10px; border-radius: 50%; margin-top: 0.4rem;
    flex-shrink: 0; background: var(--fg-3);
  }
  .s-dot[data-tone="good"] { background: var(--good); box-shadow: 0 0 8px var(--good); }
  .s-dot[data-tone="warn"] { background: var(--warn); box-shadow: 0 0 8px var(--warn); }
  .s-dot[data-tone="bad"]  { background: var(--bad);  box-shadow: 0 0 8px var(--bad); }
  .s-body { flex: 1; min-width: 0; }
  .s-title { font-weight: 600; color: var(--fg-0); font-size: var(--font-size-lg); margin-bottom: 0.2rem; }
  .s-detail { color: var(--fg-2); font-size: var(--font-size-sm); margin: 0 0 0.5rem; line-height: 1.5; }
  .s-actions { display: flex; gap: 0.6rem; flex-wrap: wrap; }
  .s-actions:empty { display: none; }

  .section-title {
    font-size: 11px; text-transform: uppercase; letter-spacing: 0.12em;
    color: var(--fg-3); font-weight: 700; margin: 1.75rem 0 0.85rem 0;
  }

  .rec-grid { display: grid; grid-template-columns: 1.4fr 1fr; gap: 1rem; align-items: start; }
  @media (max-width: 1000px) { .rec-grid { grid-template-columns: 1fr; } }
  .rec-main { display: flex; flex-direction: column; }
  .rec-side { display: flex; flex-direction: column; gap: 1rem; }
  .reboot-grid {
    display: grid; grid-template-columns: repeat(auto-fit, minmax(184px, 1fr)); gap: 0.7rem;
    margin-bottom: 1.5rem;
  }
  .fw-below { max-width: 760px; }
  .reboot-desc { white-space: normal; overflow-wrap: anywhere; }

  .reboot-btn {
    display: flex; flex-direction: column; align-items: flex-start; gap: 0.2rem;
    text-align: left; padding: 1rem 1.1rem;
    background: var(--bg-2); border: 1px solid var(--hairline);
    border-radius: 16px; box-shadow: var(--shadow-sm);
    cursor: pointer; transition: border-color var(--t-fast), background var(--t-fast);
  }
  .reboot-btn:hover:not(:disabled) { border-color: var(--card-hover-border); background: var(--bg-3); }
  .reboot-btn.danger:hover:not(:disabled) { border-color: rgba(239, 68, 68, 0.45); }
  .reboot-label { font-family: var(--font-display); font-weight: 700; font-size: 1.05rem; color: var(--fg-0); }
  .reboot-btn.danger .reboot-label { color: var(--bad); }
  .reboot-desc { font-size: var(--font-size-xs); color: var(--fg-3); }

  .sideload-card { margin-bottom: 1.5rem; }
  .sideload-card h3 { margin-bottom: 0.35rem; }

  .fb-card, .fw-card { margin-bottom: 1.5rem; }
  .fb-card h3, .fw-card h3 { margin-bottom: 0.35rem; }
  .fb-row { display: flex; gap: 0.6rem; align-items: center; margin-top: 0.6rem; flex-wrap: wrap; }
  .fb-row select { width: auto; min-width: 150px; }
  .fb-sub { font-size: 11px; text-transform: uppercase; letter-spacing: 0.1em; color: var(--fg-3); font-weight: 700; margin-top: 1.1rem; }
  .fb-pill {
    font-size: var(--font-size-xs); font-weight: 600; padding: 3px 9px; border-radius: 99px;
    background: var(--good-soft); color: var(--good); border: 1px solid rgba(16,185,129,0.3);
  }
  .fb-pill.warn { background: var(--warn-soft); color: var(--warn); border-color: rgba(245,158,11,0.3); }
  .fb-out {
    margin: 0.6rem 0 0; max-height: 180px; overflow: auto; padding: 0.6rem 0.75rem;
    background: var(--bg-1); border: 1px solid var(--hairline); border-radius: var(--radius);
    font-family: var(--font-mono); font-size: 11px; color: var(--fg-2); white-space: pre-wrap; word-break: break-all;
  }
  .fw-id { color: var(--fg-0); font-size: var(--font-size-base); margin: 0.3rem 0 0.1rem; }
  .fw-fp { word-break: break-all; margin: 0 0 0.6rem; }
  .fw-oem { margin-top: 0.7rem; padding-top: 0.7rem; border-top: 1px solid var(--hairline); }
  .fw-oem-name { font-weight: 600; color: var(--fg-0); margin-bottom: 0.2rem; }
  .fw-url { display: block; margin-top: 0.25rem; word-break: break-all; }
  .fw-list { margin-top: 0.5rem; }
  .fw-item {
    display: flex; align-items: center; justify-content: space-between; gap: 0.6rem;
    padding: 0.55rem 0; border-bottom: 1px solid var(--hairline);
  }
  .fw-item:last-child { border-bottom: none; }
  .fw-item-name { color: var(--fg-1); font-size: var(--font-size-sm); font-weight: 500; }
  .fw-btn {
    display: inline-flex; align-items: center; gap: 0.45rem;
    margin-top: 0.55rem;
    padding: 0.5rem 0.9rem;
    background: var(--accent-soft);
    border: 1px solid var(--card-hover-border);
    color: var(--accent);
    border-radius: 10px;
    font-family: inherit; font-size: var(--font-size-sm); font-weight: 600;
    cursor: pointer;
    transition: background var(--t-fast), border-color var(--t-fast), color var(--t-fast);
  }
  .fw-btn:hover { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .fw-btn.sm { margin-top: 0; padding: 0.35rem 0.7rem; font-size: var(--font-size-xs); }
</style>
