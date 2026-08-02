<script lang="ts">
  import '../styles/global.css';
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { deviceStore } from '$stores/device.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import { themeStore } from '$stores/theme.svelte';
  import { warmCache } from '$stores/prefetch.svelte';
  import DevicePicker from '$components/DevicePicker.svelte';
  import CommandPalette from '$components/CommandPalette.svelte';
  import AppDetailsModal from '$components/AppDetailsModal.svelte';
  import Toaster from '$components/Toaster.svelte';

  let { children } = $props();

  // One-time first-run safety disclaimer (persisted in localStorage).
  let showDisclaimer = $state(false);
  onMount(() => {
    try { if (localStorage.getItem('df_disclaimer_v1') !== '1') showDisclaimer = true; } catch { /* storage blocked */ }
  });
  function ackDisclaimer() {
    try { localStorage.setItem('df_disclaimer_v1', '1'); } catch { /* ignore */ }
    showDisclaimer = false;
  }

  // Warm other tabs' data in the background so switching feels instant. Runs
  // once per connected device; sequential + paced so it never hurts fluidity.
  $effect(() => {
    const d = deviceStore.selected;
    if (d?.state === 'device') warmCache(d.serial);
  });

  // App version shown in the sidebar footer (kept in sync with tauri.conf.json).
  let appVersion = $state('');
  onMount(async () => {
    try {
      const { getVersion } = await import('@tauri-apps/api/app');
      appVersion = await getVersion();
    } catch { /* non-Tauri / dev fallback */ }
  });

  type NavItem = { href: string; label: string; icon: string };
  type NavSection = { label: string; items: NavItem[] };

  // Inline icon paths (24x24, stroke). Rendered inside a shared <svg> wrapper.
  const ICON = {
    overview: '<rect x="3" y="3" width="7" height="7" rx="1.4"/><rect x="14" y="3" width="7" height="7" rx="1.4"/><rect x="14" y="14" width="7" height="7" rx="1.4"/><rect x="3" y="14" width="7" height="7" rx="1.4"/>',
    fleet: '<polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 17 12 22 22 17"/><polyline points="2 12 12 17 22 12"/>',
    sleep: '<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>',
    battery: '<rect x="2" y="7" width="16" height="10" rx="2.5"/><line x1="22" y1="11" x2="22" y2="13"/><line x1="6" y1="11" x2="6" y2="13"/><line x1="10" y1="11" x2="10" y2="13"/>',
    storage: '<line x1="22" y1="12" x2="2" y2="12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><line x1="6" y1="16" x2="6.01" y2="16"/><line x1="10" y1="16" x2="10.01" y2="16"/>',
    network: '<circle cx="12" cy="12" r="9"/><line x1="3" y1="12" x2="21" y2="12"/><path d="M12 3a15 15 0 0 1 0 18 15 15 0 0 1 0-18z"/>',
    system: '<line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/>',
    apps: '<rect x="3" y="3" width="7" height="7" rx="1.4"/><rect x="14" y="3" width="7" height="7" rx="1.4"/><rect x="14" y="14" width="7" height="7" rx="1.4"/><circle cx="6.5" cy="17.5" r="3.5"/>',
    files: '<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>',
    backup: '<rect x="2" y="4" width="20" height="5" rx="1"/><path d="M4 9v9a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9"/><line x1="10" y1="13" x2="14" y2="13"/>',
    telemetry: '<polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>',
    tools: '<polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>',
    safety: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M9 12l2 2 4-4"/>',
    recovery: '<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="4"/><line x1="4.93" y1="4.93" x2="9.17" y2="9.17"/><line x1="14.83" y1="14.83" x2="19.07" y2="19.07"/><line x1="14.83" y1="9.17" x2="19.07" y2="4.93"/><line x1="4.93" y1="19.07" x2="9.17" y2="14.83"/>',
    toolbox: '<path d="M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.6 2.6a2 2 0 0 1-2.8-2.8z"/>',
    tweaks: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>'
  };

  const sections: NavSection[] = [
    { label: 'Menu', items: [
      { href: '/',           label: 'Overview',      icon: ICON.overview },
      { href: '/fleet/',     label: 'Fleet',         icon: ICON.fleet }
    ]},
    { label: 'Optimize', items: [
      { href: '/sleep/',     label: 'Doze & Sleep',  icon: ICON.sleep },
      { href: '/battery/',   label: 'Battery',       icon: ICON.battery },
      { href: '/storage/',   label: 'Storage',       icon: ICON.storage },
      { href: '/network/',   label: 'Network & DNS', icon: ICON.network },
      { href: '/system/',    label: 'System Tweaks', icon: ICON.system },
      { href: '/tweaks/',    label: 'Advanced Tweaks', icon: ICON.tweaks }
    ]},
    { label: 'Manage', items: [
      { href: '/apps/',      label: 'App Manager',   icon: ICON.apps },
      { href: '/files/',     label: 'File Manager',  icon: ICON.files },
      { href: '/backup/',    label: 'Backup & Restore', icon: ICON.backup }
    ]},
    { label: 'Safety', items: [
      { href: '/safety/',    label: 'Profiles & Snapshots', icon: ICON.safety },
      { href: '/recovery/',  label: 'Recovery',             icon: ICON.recovery }
    ]},
    { label: 'Diagnostics', items: [
      { href: '/telemetry/', label: 'Telemetry',     icon: ICON.telemetry },
      { href: '/tools/',     label: 'Logs & Tools',  icon: ICON.tools },
      { href: '/toolbox/',   label: 'Toolbox',       icon: ICON.toolbox }
    ]}
  ];

  let collapsed = $state(false);
  let paletteOpen = $state(false);
  let maximized = $state(false);

  // ─── Custom window chrome (frameless) ──────────────────────────────────
  async function win() {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return getCurrentWindow();
  }
  async function winMinimize() { (await win()).minimize(); }
  async function winToggleMax() { const w = await win(); await w.toggleMaximize(); maximized = await w.isMaximized(); }
  async function winClose() { (await win()).close(); }
  async function startResize(dir: string) {
    const w = await win();
    // ResizeDirection string values accepted by Tauri v2.
    await (w as unknown as { startResizeDragging: (d: string) => Promise<void> }).startResizeDragging(dir);
  }

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
      e.preventDefault();
      paletteOpen = !paletteOpen;
    }
  }

  onMount(() => {
    themeStore.apply();
    deviceStore.refresh();
    window.addEventListener('keydown', onKey);
    // Track maximize state so the button icon reflects it.
    (async () => {
      try {
        const w = await win();
        maximized = await w.isMaximized();
        await w.onResized(async () => { maximized = await w.isMaximized(); });
      } catch { /* not running under Tauri */ }
    })();
  });
  onDestroy(() => {
    window.removeEventListener('keydown', onKey);
  });

  // Opportunistic background hydration of app labels once a device is selected.
  $effect(() => {
    const dev = deviceStore.selected;
    if (dev && dev.state === 'device') {
      labelStore.hydrate(dev.serial);
    }
  });
</script>

<div class="app" class:collapsed>
  <!-- ─── Sidebar ─────────────────────────────────────── -->
  <aside class="sidebar">
    <div class="brand" data-tauri-drag-region>
      <a href="/" class="brand-link" title="DozeForge">
        <span class="brand-mark" aria-hidden="true">
          <img src="/logo.png?v=5" width="28" height="28" alt="DozeForge" />
        </span>
        {#if !collapsed}<span class="brand-text">DozeForge</span>{/if}
      </a>
      <button class="collapse-btn" onclick={() => collapsed = !collapsed} title={collapsed ? i18n.t('Expand') : i18n.t('Collapse')} aria-label="Toggle sidebar">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          {#if collapsed}<polyline points="9 18 15 12 9 6"/>{:else}<polyline points="15 18 9 12 15 6"/>{/if}
        </svg>
      </button>
    </div>

    <nav class="nav" aria-label="Main navigation">
      {#each sections as section (section.label)}
        <div class="nav-section">
          {#if !collapsed}<div class="nav-section-label">{i18n.t(section.label)}</div>{/if}
          {#each section.items as item (item.href)}
            <a
              href={item.href}
              class="nav-item"
              class:active={page.url.pathname === item.href}
              onclick={() => goto(item.href)}
              title={i18n.t(item.label)}
            >
              <svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                {@html item.icon}
              </svg>
              {#if !collapsed}<span class="nav-label">{i18n.t(item.label)}</span>{/if}
            </a>
          {/each}
        </div>
      {/each}
    </nav>

    <div class="sidebar-foot">
      {#if deviceStore.hasRootAccess}
        <button
          class="foot-btn"
          class:active={deviceStore.rootMode}
          onclick={() => deviceStore.toggleRootMode()}
          title={i18n.t('Toggle Root Mode (Access /data/data)')}
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          </svg>
          {#if !collapsed}<span>{i18n.t('Root Mode')}</span>{/if}
        </button>
      {/if}
      {#if appVersion}
        <span class="app-version" title="DozeForge v{appVersion}">{collapsed ? `v${appVersion.split('-')[0]}` : `v${appVersion}`}</span>
      {/if}
    </div>
  </aside>

  <!-- ─── Main column ─────────────────────────────────── -->
  <div class="main-col">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header class="topbar" data-tauri-drag-region ondblclick={(e) => { if (e.target === e.currentTarget) winToggleMax(); }}>
      <div class="topbar-left">
        <DevicePicker />
        <button class="search" onclick={() => paletteOpen = true} aria-label={i18n.t('Open command palette')}>
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6">
            <circle cx="7" cy="7" r="5"/><path d="M11 11l3 3" stroke-linecap="round"/>
          </svg>
          <span class="search-text">{i18n.t('Search or jump to…')}</span>
          <span class="kbd mono">Ctrl K</span>
        </button>
      </div>

      <div class="topbar-right">
        <button
          class="icon-btn"
          onclick={() => themeStore.toggle()}
          title={i18n.t('Toggle light / dark theme')}
          aria-label="Toggle theme"
        >
          {#if themeStore.theme === 'light'}
            <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="12" cy="12" r="5" fill="currentColor"/>
              <path class="rays" d="M12 1v3M12 20v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M1 12h3M20 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/>
            </svg>
          {:else}
            <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z" fill="currentColor"/>
            </svg>
          {/if}
        </button>
        <button
          class="lang-btn"
          onclick={() => i18n.toggle()}
          title={i18n.t('Change Language (English / Español)')}
        >
          {i18n.lang === 'es' ? 'ES' : 'EN'}
        </button>
      </div>

      <div class="win-controls">
        <button class="win-btn" onclick={winMinimize} aria-label={i18n.t('Minimize')} title={i18n.t('Minimize')}>
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.3"><line x1="2" y1="6" x2="10" y2="6"/></svg>
        </button>
        <button class="win-btn" onclick={winToggleMax} aria-label={i18n.t('Maximize')} title={i18n.t('Maximize')}>
          {#if maximized}
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.3"><rect x="2.5" y="3.5" width="6" height="6" rx="1"/><path d="M4.5 3.5V2.5h5v5h-1"/></svg>
          {:else}
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.3"><rect x="2.5" y="2.5" width="7" height="7" rx="1"/></svg>
          {/if}
        </button>
        <button class="win-btn close" onclick={winClose} aria-label={i18n.t('Close')} title={i18n.t('Close')}>
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.3"><path d="M3 3l6 6M9 3l-6 6"/></svg>
        </button>
      </div>
    </header>

    {#if deviceStore.error}
      <div class="topbar-error">{deviceStore.error}</div>
    {/if}

    <main>
      <div class="content">
        {@render children?.()}
      </div>
    </main>
  </div>

  <CommandPalette bind:open={paletteOpen} onNavigate={(href) => { paletteOpen = false; goto(href); }} />
  <AppDetailsModal />
  <Toaster />

  {#if showDisclaimer}
    <div class="disclaimer-backdrop" role="dialog" aria-modal="true">
      <div class="disclaimer-box">
        <h2>{i18n.t('Before you start')}</h2>
        <p>{i18n.t("DozeForge's core optimizer is no-root and reversible. But the Recovery and Root tools can flash partitions, switch slots, set SELinux permissive and write to the kernel.")}</p>
        <p class="warn">{i18n.t('Misuse — or a wrong or interrupted flash — can brick your device, void your warranty, or break banking apps. Use it at your own risk; the authors accept no liability.')}</p>
        <button class="primary" onclick={ackDisclaimer}>{i18n.t('I understand and accept')}</button>
      </div>
    </div>
  {/if}

  {#if !maximized}
    <!-- Frameless window resize handles (edges + corners). -->
    <div class="rz rz-t"  onmousedown={() => startResize('North')}     role="presentation"></div>
    <div class="rz rz-b"  onmousedown={() => startResize('South')}     role="presentation"></div>
    <div class="rz rz-l"  onmousedown={() => startResize('West')}      role="presentation"></div>
    <div class="rz rz-r"  onmousedown={() => startResize('East')}      role="presentation"></div>
    <div class="rz rz-tl" onmousedown={() => startResize('NorthWest')} role="presentation"></div>
    <div class="rz rz-tr" onmousedown={() => startResize('NorthEast')} role="presentation"></div>
    <div class="rz rz-bl" onmousedown={() => startResize('SouthWest')} role="presentation"></div>
    <div class="rz rz-br" onmousedown={() => startResize('SouthEast')} role="presentation"></div>
  {/if}
</div>

<style>
  .disclaimer-backdrop {
    position: fixed; inset: 0; z-index: 3000;
    display: flex; align-items: center; justify-content: center;
    background: rgba(0, 0, 0, 0.62); backdrop-filter: blur(2px); padding: 1.5rem;
  }
  .disclaimer-box {
    max-width: 460px; background: var(--bg-2); border: 1px solid var(--border-strong);
    border-radius: 16px; padding: 1.5rem; box-shadow: var(--shadow-lg);
  }
  .disclaimer-box h2 { margin: 0 0 0.7rem; font-size: 1.2rem; color: var(--fg-0); letter-spacing: -0.02em; }
  .disclaimer-box p { margin: 0 0 0.7rem; font-size: var(--font-size-sm); color: var(--fg-2); line-height: 1.55; }
  .disclaimer-box p.warn { color: var(--warn); }
  .disclaimer-box .primary { width: 100%; margin-top: 0.5rem; }

  .app {
    display: flex;
    height: 100vh;
    overflow: hidden;
    /* Shared atmosphere for the whole shell: orange glow + faint 32px grid. */
    background-color: var(--bg-0);
    background-image:
      radial-gradient(circle at 18% -6%, var(--glow-1), transparent 40%),
      radial-gradient(circle at 100% 0%, var(--glow-2), transparent 44%),
      linear-gradient(to right, var(--grid-line) 1px, transparent 1px),
      linear-gradient(to bottom, var(--grid-line) 1px, transparent 1px);
    background-size: 100% 100%, 100% 100%, 32px 32px, 32px 32px;
  }

  /* ─── Sidebar ─────────────────────────────────────── */
  .sidebar {
    width: 232px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--glass);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    border-right: none;
    transition: width var(--t-base);
  }
  .app.collapsed .sidebar { width: 66px; }

  .brand {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    height: 60px;
    padding: 0 0.85rem 0 1.05rem;
    flex-shrink: 0;
  }
  .brand-link { display: flex; align-items: center; gap: 0.6rem; text-decoration: none; overflow: hidden; }
  .brand-link:hover { text-decoration: none; }
  .brand-mark { display: flex; flex-shrink: 0; }
  .brand-text {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1rem;
    letter-spacing: 0.01em;
    white-space: nowrap;
    background: var(--accent-gradient);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  .collapse-btn {
    display: flex; align-items: center; justify-content: center;
    width: 26px; height: 26px; padding: 0;
    background: transparent; border: 1px solid var(--hairline);
    border-radius: 6px; color: var(--fg-3); cursor: pointer; flex-shrink: 0;
  }
  .collapse-btn:hover { color: var(--fg-1); border-color: var(--border-strong); background: var(--bg-2); }
  /* Collapsed: the brand row becomes just the centered expand toggle (the logo
     hides), so nothing stacks awkwardly and the control stays obvious. */
  .app.collapsed .brand { justify-content: center; padding: 0; }
  .app.collapsed .brand-link { display: none; }
  .app.collapsed .collapse-btn { width: 34px; height: 34px; }

  .nav {
    flex: 1;
    overflow-y: auto;
    padding: 0.35rem 0.6rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .nav-section { display: flex; flex-direction: column; gap: 2px; }
  .nav-section-label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--fg-3);
    padding: 0.9rem 0.65rem 0.35rem;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.5rem 0.65rem;
    border-radius: var(--radius);
    color: var(--fg-2);
    font-size: 13.5px;
    font-weight: 500;
    text-decoration: none;
    white-space: nowrap;
    position: relative;
    transition: background var(--t-fast), color var(--t-fast);
  }
  .nav-item:hover { background: var(--chrome-hover); color: var(--fg-0); text-decoration: none; }
  .nav-item.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .nav-item.active::before {
    content: '';
    position: absolute;
    left: -0.6rem;
    top: 50%;
    transform: translateY(-50%);
    width: 3px;
    height: 60%;
    border-radius: 0 99px 99px 0;
    background: var(--accent-gradient);
    box-shadow: 0 0 12px var(--accent-glow);
  }
  .nav-icon { width: 18px; height: 18px; flex-shrink: 0; }
  .nav-label { overflow: hidden; text-overflow: ellipsis; }

  .app.collapsed .nav { padding: 0.35rem 0.5rem 1rem; align-items: center; }
  .app.collapsed .nav-item { justify-content: center; padding: 0.55rem; width: 42px; }
  .app.collapsed .nav-item.active::before { left: -0.5rem; }

  .sidebar-foot {
    padding: 0.6rem;
    border-top: 1px solid var(--hairline);
    flex-shrink: 0;
  }
  .app-version {
    display: block;
    margin-top: 0.5rem;
    text-align: center;
    font-size: 10px;
    letter-spacing: 0.04em;
    color: var(--fg-3);
    font-variant-numeric: tabular-nums;
    cursor: default;
  }
  .foot-btn {
    display: flex; align-items: center; gap: 0.6rem;
    width: 100%;
    padding: 0.5rem 0.65rem;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--fg-2);
    font-size: 13px;
    cursor: pointer;
  }
  .foot-btn:hover { color: var(--fg-0); border-color: var(--border-strong); }
  .foot-btn.active { background: var(--bad-soft); border-color: rgba(239, 68, 68, 0.4); color: var(--bad); }
  .app.collapsed .foot-btn { justify-content: center; }

  /* ─── Main column ─────────────────────────────────── */
  .main-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    height: 60px;
    min-height: 60px;
    padding: 0 1.5rem;
    background: var(--glass);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    border-bottom: none;
    flex-shrink: 0;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 340px;
    max-width: 42vw;
    height: 38px;
    padding: 0 0.85rem;
    background: var(--chrome-hover);
    border: 1px solid var(--hairline);
    border-radius: var(--radius);
    color: var(--fg-3);
    cursor: pointer;
    transition: border-color var(--t-fast), background var(--t-fast);
    font-size: 13px;
  }
  .search:hover { border-color: var(--border-strong); background: var(--chrome-hover); }
  .search-text { flex: 1; text-align: left; }
  .kbd {
    font-size: 0.66rem;
    color: var(--fg-3);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 1px 6px;
    background: var(--bg-1);
  }

  .topbar-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .topbar-right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }
  .icon-btn {
    display: flex; align-items: center; justify-content: center;
    width: 32px; height: 32px;
    padding: 0;
    border-radius: 7px;
    background: var(--chrome-hover);
    border: 1px solid var(--hairline);
    color: var(--fg-1);
    cursor: pointer;
    transition: all var(--t-fast);
  }
  .icon-btn:hover { color: var(--accent); border-color: var(--accent); }
  .icon-btn svg { width: 18px; height: 18px; display: block; fill: currentColor; }
  .icon-btn svg .rays { fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; }
  .lang-btn {
    display: flex; align-items: center; justify-content: center;
    font-size: 11px; font-weight: 700; font-family: var(--font-mono);
    width: 32px; height: 32px;
    border-radius: 7px;
    background: var(--chrome-hover);
    border: 1px solid var(--hairline);
    color: var(--fg-1);
    cursor: pointer;
    transition: all var(--t-fast);
  }
  .lang-btn:hover { color: var(--accent); border-color: var(--accent); }

  .topbar-error {
    background: var(--bad-soft);
    color: var(--bad);
    padding: 0.4rem 1.5rem;
    font-size: var(--font-size-sm);
    border-bottom: 1px solid rgba(239, 68, 68, 0.2);
    flex-shrink: 0;
  }

  /* ─── Content ─────────────────────────────────────── */
  main {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: transparent;
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 2rem 2.5rem 3rem;
    max-width: 1500px;
    width: 100%;
    margin: 0 auto;
  }

  /* Only shrink the search on very narrow windows — never auto-collapse the
     sidebar (display scaling made that hide the expand button unexpectedly).
     Collapsing is now fully manual via the toggle in the brand row. */
  @media (max-width: 900px) {
    .search { width: 200px; }
  }

  /* ─── Custom window controls (frameless) ───────────────── */
  .win-controls { display: flex; align-items: center; gap: 2px; margin-left: 0.35rem; position: relative; z-index: 10001; }
  .win-btn {
    display: flex; align-items: center; justify-content: center;
    width: 34px; height: 30px; padding: 0;
    background: transparent; border: none; border-radius: 6px;
    color: var(--fg-2); cursor: pointer; transition: background var(--t-fast), color var(--t-fast);
  }
  .win-btn:hover { background: var(--chrome-hover); color: var(--fg-0); }
  .win-btn.close:hover { background: #E24B4A; color: #fff; }

  /* ─── Resize handles ───────────────────────────────────── */
  .rz { position: fixed; z-index: 10000; }
  .rz-t { top: 0; left: 8px; right: 8px; height: 4px; cursor: ns-resize; }
  .rz-b { bottom: 0; left: 8px; right: 8px; height: 4px; cursor: ns-resize; }
  .rz-l { left: 0; top: 8px; bottom: 8px; width: 4px; cursor: ew-resize; }
  .rz-r { right: 0; top: 8px; bottom: 8px; width: 4px; cursor: ew-resize; }
  .rz-tl { top: 0; left: 0; width: 8px; height: 8px; cursor: nwse-resize; }
  .rz-tr { top: 0; right: 0; width: 8px; height: 8px; cursor: nesw-resize; }
  .rz-bl { bottom: 0; left: 0; width: 8px; height: 8px; cursor: nesw-resize; }
  .rz-br { bottom: 0; right: 0; width: 8px; height: 8px; cursor: nwse-resize; }
</style>
