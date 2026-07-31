<script lang="ts">
  import Fuse from 'fuse.js';
  import { onMount } from 'svelte';
  import { i18n } from '$stores/i18n.svelte';
  import { themeStore } from '$stores/theme.svelte';
  import { deviceStore } from '$stores/device.svelte';

  interface Props {
    open: boolean;
    onNavigate: (href: string) => void;
  }
  let { open = $bindable(false), onNavigate }: Props = $props();

  const commands = $derived([
    // ── Actions (executable — things the sidebar can't do) ──────────────
    { id: 'act:theme',   group: i18n.t('Actions'), label: i18n.t('Toggle light / dark theme'),            hint: i18n.t('light / dark'), action: () => { themeStore.toggle(); open = false; } },
    { id: 'act:lang',    group: i18n.t('Actions'), label: i18n.t('Change Language (English / Español)'),  hint: i18n.lang === 'es' ? 'ES → EN' : 'EN → ES', action: () => { i18n.toggle(); open = false; } },
    { id: 'act:refresh', group: i18n.t('Actions'), label: i18n.t('Refresh devices'),                     hint: i18n.t('rescan ADB'), action: () => { deviceStore.refresh(); open = false; } },
    // ── Navigate ────────────────────────────────────────────────────────
    { id: 'nav:overview',   group: i18n.t('Navigate'), label: i18n.t('Go to Overview'),      hint: i18n.t('snapshot of device state'),         action: () => onNavigate('/') },
    { id: 'nav:fleet',      group: i18n.t('Navigate'), label: i18n.t('Go to Fleet'),         hint: i18n.t('bulk actions on many devices'),     action: () => onNavigate('/fleet/') },
    { id: 'nav:apps',       group: i18n.t('Navigate'), label: i18n.t('Go to App Manager'),   hint: i18n.t('bloatware, firewall, permissions'), action: () => onNavigate('/apps/') },
    { id: 'nav:sleep',      group: i18n.t('Navigate'), label: i18n.t('Go to Doze & Sleep'),  hint: i18n.t('wakelock analysis'),                action: () => onNavigate('/sleep/') },
    { id: 'nav:battery',    group: i18n.t('Navigate'), label: i18n.t('Go to Battery'),       hint: i18n.t('health, cycles, sysfs'),            action: () => onNavigate('/battery/') },
    { id: 'nav:storage',    group: i18n.t('Navigate'), label: i18n.t('Go to Storage'),       hint: i18n.t('inventory, trim, dexopt'),          action: () => onNavigate('/storage/') },
    { id: 'nav:safety',     group: i18n.t('Navigate'), label: i18n.t('Go to Profiles & Snapshots'), hint: i18n.t('1-click optimize, undo'),     action: () => onNavigate('/safety/') },
    { id: 'nav:recovery',   group: i18n.t('Navigate'), label: i18n.t('Go to Recovery'),      hint: i18n.t('reboot modes, sideload, unbrick'), action: () => onNavigate('/recovery/') },
    { id: 'nav:backup',     group: i18n.t('Navigate'), label: i18n.t('Go to Backup & Restore'), hint: i18n.t('encrypted .ab backups'),          action: () => onNavigate('/backup/') },
    { id: 'nav:tweaks',     group: i18n.t('Navigate'), label: i18n.t('Go to Advanced Tweaks'), hint: i18n.t('RAM Plus, phantom limit, …'),      action: () => onNavigate('/tweaks/') },
    { id: 'nav:network',    group: i18n.t('Navigate'), label: i18n.t('Go to Network & DNS'), hint: i18n.t('private DNS, data saver'),          action: () => onNavigate('/network/') },
    { id: 'nav:system',     group: i18n.t('Navigate'), label: i18n.t('Go to System Tweaks'), hint: i18n.t('global system settings'),           action: () => onNavigate('/system/') },
    { id: 'nav:telemetry',  group: i18n.t('Navigate'), label: i18n.t('Go to Telemetry'),     hint: i18n.t('live process table'),               action: () => onNavigate('/telemetry/') },
    { id: 'nav:tools',      group: i18n.t('Navigate'), label: i18n.t('Go to Diagnostics & Tools'), hint: i18n.t('logs, bugreport, automation'), action: () => onNavigate('/tools/') },
    { id: 'nav:files',      group: i18n.t('Navigate'), label: i18n.t('Go to File Manager'),  hint: i18n.t('browse device storage'),            action: () => onNavigate('/files/') },
    { id: 'apps:permissions', group: i18n.t('Jump to'), label: i18n.t('Open Permissions Audit'), hint: i18n.t('review granted permissions'),  action: () => onNavigate('/apps/?tab=permissions') },
    { id: 'network:dns',      group: i18n.t('Jump to'), label: i18n.t('Open Private DNS'),  hint: i18n.t('AdGuard, Cloudflare, ...'),          action: () => onNavigate('/network/') },
    { id: 'storage:inventory', group: i18n.t('Jump to'), label: i18n.t('Storage inventory'), hint: i18n.t('apps by code size'),               action: () => onNavigate('/storage/') },
    { id: 'tools:logs',       group: i18n.t('Jump to'), label: i18n.t('Open Live Logs'),    hint: i18n.t('logcat / dmesg stream'),            action: () => onNavigate('/tools/?tab=logs') },
    { id: 'tools:bugreport',  group: i18n.t('Jump to'), label: i18n.t('Capture Bugreport'), hint: i18n.t('full device dump'),                 action: () => onNavigate('/tools/?tab=bugreport') },
    { id: 'tools:actions',    group: i18n.t('Jump to'), label: i18n.t('Advanced Tools'),    hint: i18n.t('power-user operations'),            action: () => onNavigate('/tools/?tab=actions') },
    { id: 'tools:profiles',   group: i18n.t('Jump to'), label: i18n.t('Automation Profiles'), hint: i18n.t('export / import scripts'),        action: () => onNavigate('/tools/?tab=profiles') },
  ]);

  const fuse = $derived(new Fuse(commands, {
    keys: ['label', 'hint', 'id', 'group'],
    threshold: 0.4,
    includeScore: false
  }));

  let query = $state('');
  let cursor = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();

  const results = $derived.by(() => {
    if (!query.trim()) return commands.slice(0, 8);
    return fuse.search(query).map((r) => r.item).slice(0, 8);
  });

  $effect(() => {
    if (open) {
      setTimeout(() => inputEl?.focus(), 0);
      query = '';
      cursor = 0;
    }
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') { open = false; return; }
    if (e.key === 'ArrowDown') { e.preventDefault(); cursor = Math.min(cursor + 1, results.length - 1); }
    if (e.key === 'ArrowUp')   { e.preventDefault(); cursor = Math.max(cursor - 1, 0); }
    if (e.key === 'Enter') {
      e.preventDefault();
      const item = results[cursor];
      if (item) item.action();
    }
  }

  function close() { open = false; }
</script>

{#if open}
  <div class="backdrop" onclick={close} onkeydown={onKey} role="presentation">
    <div
      class="palette"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <input
        bind:this={inputEl}
        bind:value={query}
        onkeydown={onKey}
        placeholder={i18n.t('Type a command or page name…')}
        spellcheck="false"
        autocomplete="off"
      />
      <ul>
        {#each results as item, i (item.id)}
          <li
            class:active={i === cursor}
            onclick={() => item.action()}
            onmouseenter={() => cursor = i}
            role="option"
            aria-selected={i === cursor}
            tabindex="0"
            onkeydown={(e) => { if (e.key === 'Enter') item.action(); }}
          >
            <div class="cmd-main">
              <div class="label">{item.label}</div>
              <div class="hint">{item.hint}</div>
            </div>
            <span class="cmd-group">{item.group}</span>
          </li>
        {/each}
        {#if results.length === 0}
          <li class="empty">{i18n.t('No matches for "{{query}}"', { query })}</li>
        {/if}
      </ul>
      <footer>
        <span><kbd>↑↓</kbd> {i18n.t('navigate')}</span>
        <span><kbd>↵</kbd> {i18n.t('select')}</span>
        <span><kbd>Esc</kbd> {i18n.t('close')}</span>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 18vh;
    z-index: 1000;
    animation: fadeIn 120ms ease-out;
  }
  @keyframes fadeIn {
    from { opacity: 0; }
    to   { opacity: 1; }
  }
  .palette {
    width: min(640px, 90vw);
    background: var(--bg-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    animation: slideDown 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  @keyframes slideDown {
    from { transform: translateY(-12px); opacity: 0; }
    to   { transform: translateY(0); opacity: 1; }
  }
  .palette input {
    border: none;
    border-bottom: 1px solid var(--border);
    background: transparent;
    border-radius: 0;
    padding: 1rem 1.25rem;
    font-size: var(--font-size-lg);
    color: var(--fg-0);
  }
  .palette input:focus {
    outline: none;
    box-shadow: none;
    border-bottom-color: var(--accent);
  }
  ul {
    list-style: none;
    padding: 0.4rem;
    margin: 0;
    max-height: 50vh;
    overflow-y: auto;
  }
  li {
    padding: 0.7rem 0.9rem;
    border-radius: var(--radius);
    cursor: pointer;
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    transition: background var(--t-fast);
  }
  li.active { background: var(--bg-3); }
  .cmd-main { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  li .label { color: var(--fg-0); font-size: var(--font-size-base); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  li .hint { color: var(--fg-3); font-size: var(--font-size-xs); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cmd-group {
    flex-shrink: 0;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-3);
    background: var(--bg-3);
    border: 1px solid var(--border);
    border-radius: 99px;
    padding: 2px 8px;
  }
  li.active .cmd-group { border-color: var(--border-strong); color: var(--fg-2); }
  li.empty {
    color: var(--fg-3);
    text-align: center;
    padding: 2rem 1rem;
  }
  footer {
    border-top: 1px solid var(--border);
    padding: 0.55rem 1rem;
    display: flex;
    gap: 1.25rem;
    font-size: var(--font-size-xs);
    color: var(--fg-3);
    background: var(--bg-1);
  }
</style>
