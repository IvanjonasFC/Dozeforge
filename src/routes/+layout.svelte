<script lang="ts">
  import '../styles/global.css';
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { deviceStore } from '$stores/device.svelte';
  import { labelStore } from '$stores/labels.svelte';
  import DevicePicker from '$components/DevicePicker.svelte';
  import CommandPalette from '$components/CommandPalette.svelte';
  import AppDetailsModal from '$components/AppDetailsModal.svelte';

  let { children } = $props();

  type NavItem = { href: string; label: string };
  type NavGroup = { id: 'diagnose' | 'optimize' | 'apply'; label: string; pages: NavItem[] };

  const groups: NavGroup[] = [
    {
      id: 'diagnose',
      label: 'Diagnose',
      pages: [
        { href: '/',           label: 'Overview'  },
        { href: '/telemetry/', label: 'Telemetry' },
        { href: '/system/',    label: 'System Info'},
        { href: '/sleep/',     label: 'Sleep'     }
      ]
    },
    {
      id: 'optimize',
      label: 'Optimize',
      pages: [
        { href: '/battery/', label: 'Battery+' },
        { href: '/privacy/', label: 'Privacy'  },
        { href: '/storage/', label: 'Storage'  }
      ]
    },
    {
      id: 'apply',
      label: 'Apply',
      pages: [
        { href: '/actions/',    label: 'Actions'    },
        { href: '/tweaks/',     label: 'Tweaks'     },
        { href: '/bloatware/',  label: 'Bloatware'  },
        { href: '/files/',      label: 'Files'      },
        { href: '/automation/', label: 'Profiles' }
      ]
    }
  ];

  // Map every page href back to its group id, used to highlight the active group.
  const groupByHref = new Map<string, NavGroup['id']>();
  for (const g of groups) for (const p of g.pages) groupByHref.set(p.href, g.id);

  const activeGroupId = $derived(groupByHref.get(page.url.pathname) ?? 'diagnose');
  const activeGroup: NavGroup = $derived(groups.find(g => g.id === activeGroupId) ?? groups[0]!);

  let paletteOpen = $state(false);

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
      e.preventDefault();
      paletteOpen = !paletteOpen;
    }
  }

  onMount(() => {
    deviceStore.refresh();
    window.addEventListener('keydown', onKey);
  });
  onDestroy(() => {
    window.removeEventListener('keydown', onKey);
  });

  // Opportunistic background hydration of app labels.
  // Fires once per device the moment a device is selected; subsequent page
  // navigations see the cached labels instantly. The scan itself takes
  // 30-90s on a typical device but is fully asynchronous, so this does
  // not block any UI.
  $effect(() => {
    const dev = deviceStore.selected;
    if (dev && dev.state === 'device') {
      labelStore.hydrate(dev.serial);
    }
  });
</script>

<div class="app">
  <header class="topbar">
    <!-- Logo -->
    <a href="/" class="logo-link" title="ForgeAndroid">
      <div class="logo">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2L2 7l10 5 10-5-10-5z"/>
          <path d="M2 17l10 5 10-5"/>
          <path d="M2 12l10 5 10-5"/>
        </svg>
        <span class="logo-text">ForgeAndroid</span>
      </div>
    </a>

    <!-- Group tabs (Diagnose / Optimize / Apply) -->
    <nav class="groups" aria-label="Main sections">
      {#each groups as g (g.id)}
        <a
          href={g.pages[0]!.href}
          class="group"
          class:active={activeGroupId === g.id}
          onclick={() => goto(g.pages[0]!.href)}
          title={g.label}
        >
          {g.label}
        </a>
      {/each}
    </nav>

    <!-- Sub-tabs for active group -->
    <div class="subtabs-inline" aria-label="Section pages">
      {#each activeGroup.pages as p (p.href)}
        <a
          href={p.href}
          class="subtab"
          class:active={page.url.pathname === p.href}
          onclick={() => goto(p.href)}
        >
          {p.label}
        </a>
      {/each}
    </div>

    <!-- Right side: device picker + search -->
    <div class="topbar-right">
      <DevicePicker />
      <button
        class="palette-btn"
        onclick={() => paletteOpen = true}
        title="Command Palette (Ctrl+K)"
        aria-label="Open command palette"
      >
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="7" cy="7" r="5"/>
          <path d="M11 11l3 3" stroke-linecap="round"/>
        </svg>
        <span class="kbd mono">Ctrl+K</span>
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

  <CommandPalette bind:open={paletteOpen} onNavigate={(href) => { paletteOpen = false; goto(href); }} />
  <AppDetailsModal />
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  /* ─── Topbar ─────────────────────────────────────────── */
  .topbar {
    display: flex;
    align-items: center;
    gap: 0;
    padding: 0 1.5rem;
    background: var(--bg-1);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    height: 52px;
    min-height: 52px;
  }

  /* ─── Logo ─────────────────────────────────────────────  */
  .logo-link {
    display: flex;
    align-items: center;
    text-decoration: none;
    margin-right: 1.75rem;
    flex-shrink: 0;
  }
  .logo-link:hover { text-decoration: none; }
  .logo {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--fg-0);
  }
  .logo svg { color: var(--accent); flex-shrink: 0; }
  .logo-text {
    font-family: var(--font-mono);
    font-weight: 700;
    font-size: 0.82rem;
    letter-spacing: 0.04em;
    color: var(--fg-0);
    white-space: nowrap;
  }

  /* ─── Group tabs (Diagnose / Optimize / Apply) ─────────  */
  .groups {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 100%;
    flex-shrink: 0;
  }
  .group {
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0 1rem;
    color: var(--fg-3);
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    text-decoration: none;
    border-bottom: 2px solid transparent;
    transition: color var(--t-fast), border-color var(--t-fast);
    white-space: nowrap;
  }
  .group:hover { color: var(--fg-1); text-decoration: none; }
  .group.active {
    color: var(--fg-0);
    border-bottom-color: var(--accent);
  }

  /* ─── Separator ─────────────────────────────────────────  */
  .groups::after {
    content: '';
    display: block;
    width: 1px;
    height: 20px;
    background: var(--border);
    margin: 0 1rem;
    flex-shrink: 0;
  }

  /* ─── Sub-tabs (inline with topbar) ─────────────────────  */
  .subtabs-inline {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 100%;
    overflow-x: auto;
    scrollbar-width: none;
    flex: 1;
  }
  .subtabs-inline::-webkit-scrollbar { display: none; }
  .subtab {
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0 0.85rem;
    color: var(--fg-3);
    font-size: var(--font-size-sm);
    font-weight: 500;
    text-decoration: none;
    border-bottom: 2px solid transparent;
    transition: color var(--t-fast), border-color var(--t-fast);
    white-space: nowrap;
  }
  .subtab:hover { color: var(--fg-1); text-decoration: none; }
  .subtab.active {
    color: var(--fg-0);
    border-bottom-color: var(--accent);
  }

  /* ─── Right side ─────────────────────────────────────────  */
  .topbar-right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
    padding-left: 1rem;
  }

  .palette-btn {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.35rem 0.65rem;
    color: var(--fg-3);
    font-size: 0.72rem;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: color var(--t-fast), background var(--t-fast);
  }
  .palette-btn:hover { color: var(--fg-1); background: var(--bg-3); }
  .kbd {
    color: var(--fg-3);
    font-size: 0.65rem;
    letter-spacing: 0.02em;
  }

  /* ─── Error banner ───────────────────────────────────────  */
  .topbar-error {
    background: var(--bad-soft);
    color: var(--bad);
    padding: 0.4rem 1.5rem;
    font-size: var(--font-size-sm);
    border-bottom: 1px solid rgba(239, 68, 68, 0.2);
    flex-shrink: 0;
  }

  /* ─── Main content ───────────────────────────────────────  */
  main {
    flex: 1;
    overflow: hidden;
    background: var(--bg-0);
    display: flex;
    flex-direction: column;
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 2rem 2.5rem 3rem;
    max-width: 1400px;
    width: 100%;
    margin: 0 auto;
  }

  /* ─── Responsive ─────────────────────────────────────────  */
  @media (max-width: 960px) {
    .topbar { padding: 0 1rem; }
    .logo-link { margin-right: 1rem; }
    .group { padding: 0 0.65rem; font-size: 0.72rem; }
    .subtab { padding: 0 0.6rem; }
    .logo-text { display: none; }
  }
</style>

