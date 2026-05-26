<script lang="ts">
  import Fuse from 'fuse.js';
  import { onMount } from 'svelte';

  interface Props {
    open: boolean;
    onNavigate: (href: string) => void;
  }
  let { open = $bindable(false), onNavigate }: Props = $props();

  const commands = [
    { id: 'nav:overview',   label: 'Go to Overview',    hint: 'snapshot of device state',    action: () => onNavigate('/') },
    { id: 'nav:telemetry',  label: 'Go to Telemetry',   hint: 'live process table',          action: () => onNavigate('/telemetry/') },
    { id: 'nav:sleep',      label: 'Go to Sleep',       hint: 'wakelock analysis',           action: () => onNavigate('/sleep/') },
    { id: 'nav:battery',    label: 'Go to Battery',     hint: 'health, cycles, sysfs',       action: () => onNavigate('/battery/') },
    { id: 'nav:privacy',    label: 'Go to Privacy',     hint: 'DNS, firewall, clipboard',    action: () => onNavigate('/privacy/') },
    { id: 'nav:storage',    label: 'Go to Storage',     hint: 'inventory, trim, dexopt',     action: () => onNavigate('/storage/') },
    { id: 'nav:actions',    label: 'Go to Actions',     hint: 'apply optimizations',         action: () => onNavigate('/actions/') },
    { id: 'nav:bloatware',  label: 'Go to Bloatware',   hint: 'package manager',             action: () => onNavigate('/bloatware/') },
    { id: 'nav:automation', label: 'Go to Automation',  hint: 'export scripts',              action: () => onNavigate('/automation/') },
    { id: 'privacy:firewall', label: 'Open Firewall', hint: 'block background apps',         action: () => onNavigate('/privacy/?tab=firewall') },
    { id: 'privacy:dns',      label: 'Open Private DNS', hint: 'AdGuard, Cloudflare, ...',   action: () => onNavigate('/privacy/?tab=dns') },
    { id: 'privacy:clipboard', label: 'Open Clipboard guard', hint: 'deny READ_CLIPBOARD',   action: () => onNavigate('/privacy/?tab=clipboard') },
    { id: 'storage:inventory', label: 'Storage inventory', hint: 'apps by code size',        action: () => onNavigate('/storage/?tab=inventory') },
    { id: 'storage:trim',      label: 'Trim system caches', hint: 'free up disk space',      action: () => onNavigate('/storage/?tab=optimize') },
    { id: 'storage:dexopt',    label: 'Run ART dexopt', hint: 'destructive',                action: () => onNavigate('/storage/?tab=optimize') },
    { id: 'profile:conservative', label: 'Apply Conservative profile', hint: 'safest',       action: () => onNavigate('/actions/?profile=conservative') },
    { id: 'profile:balanced',     label: 'Apply Balanced profile',     hint: 'recommended',  action: () => onNavigate('/actions/?profile=balanced') },
    { id: 'profile:aggressive',   label: 'Apply Aggressive profile',   hint: 'all user apps',action: () => onNavigate('/actions/?profile=aggressive') },
    { id: 'profile:nuclear',      label: 'Apply Nuclear profile',      hint: 'max savings',  action: () => onNavigate('/actions/?profile=nuclear') },
  ];

  const fuse = new Fuse(commands, {
    keys: ['label', 'hint', 'id'],
    threshold: 0.4,
    includeScore: false
  });

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
        placeholder="Type a command or page name…"
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
            <div class="label">{item.label}</div>
            <div class="hint">{item.hint}</div>
          </li>
        {/each}
        {#if results.length === 0}
          <li class="empty">No matches for "{query}"</li>
        {/if}
      </ul>
      <footer>
        <span><kbd>↑↓</kbd> navigate</span>
        <span><kbd>↵</kbd> select</span>
        <span><kbd>Esc</kbd> close</span>
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
    flex-direction: column;
    gap: 2px;
    transition: background var(--t-fast);
  }
  li.active { background: var(--bg-3); }
  li .label { color: var(--fg-0); font-size: var(--font-size-base); }
  li .hint { color: var(--fg-3); font-size: var(--font-size-xs); }
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
