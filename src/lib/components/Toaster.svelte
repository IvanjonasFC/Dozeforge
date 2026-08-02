<script lang="ts">
  import { toast, type ToastKind } from '$stores/toast.svelte';
  import { fly } from 'svelte/transition';
  import { flip } from 'svelte/animate';

  // Accent colour per kind, aligned with the app's design tokens.
  const color: Record<ToastKind, string> = {
    success: 'var(--good)',
    error: 'var(--bad)',
    info: 'var(--accent)'
  };

  const icon: Record<ToastKind, string> = {
    success: 'M20 6L9 17l-5-5',
    error: 'M18 6L6 18M6 6l12 12',
    info: 'M12 16v-4M12 8h.01'
  };
</script>

<div class="toaster" role="status" aria-live="polite">
  {#each toast.items as t (t.id)}
    <div
      class="toast"
      style="--tc: {color[t.kind]}"
      role="alert"
      in:fly={{ x: 24, duration: 220 }}
      out:fly={{ x: 24, duration: 160 }}
      animate:flip={{ duration: 180 }}
    >
      <svg class="toast-icon" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
        {#if t.kind === 'info'}<circle cx="12" cy="12" r="9" stroke-width="2" />{/if}
        <path d={icon[t.kind]} />
      </svg>
      <span class="toast-msg">{t.message}</span>
      <button class="toast-x" onclick={() => toast.dismiss(t.id)} aria-label="Dismiss">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><path d="M3 3l6 6M9 3l-6 6" /></svg>
      </button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    bottom: 1rem;
    right: 1rem;
    z-index: 5000;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: min(360px, calc(100vw - 2rem));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 0.7rem 0.6rem 0.75rem;
    background: color-mix(in srgb, var(--bg-2) 92%, var(--tc));
    border: 1px solid color-mix(in srgb, var(--tc) 45%, var(--border));
    border-left: 3px solid var(--tc);
    border-radius: 10px;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(6px);
    font-size: 13px;
    color: var(--fg-1);
  }
  .toast-icon { color: var(--tc); flex-shrink: 0; }
  .toast-msg { flex: 1; line-height: 1.35; }
  .toast-x {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 6px;
    color: var(--fg-3);
    cursor: pointer;
  }
  .toast-x:hover { background: rgba(255, 255, 255, 0.08); color: var(--fg-1); }
</style>
