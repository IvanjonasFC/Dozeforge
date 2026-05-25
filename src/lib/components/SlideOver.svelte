<script lang="ts">
  interface Props {
    open: boolean;
    onClose: () => void;
    title?: string;
    width?: string;
    children?: import('svelte').Snippet;
  }
  let { open, onClose, title = '', width = '420px', children }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) onClose();
  }

  $effect(() => {
    if (open) {
      document.addEventListener('keydown', onKey);
      return () => document.removeEventListener('keydown', onKey);
    }
  });
</script>

{#if open}
  <div class="overlay" onclick={onClose} onkeydown={onKey} role="presentation"></div>
  <div class="slide" style="--width: {width};" role="dialog" aria-modal="true" aria-label={title}>
    <header>
      <h3>{title}</h3>
      <button class="ghost close" onclick={onClose} aria-label="Close">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M4 4l8 8M12 4l-8 8"/>
        </svg>
      </button>
    </header>
    <div class="body">
      {@render children?.()}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(2px);
    z-index: 90;
    animation: fadeIn 180ms ease-out;
  }
  .slide {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: var(--width);
    max-width: 92vw;
    background: var(--bg-1);
    border-left: 1px solid var(--border-strong);
    box-shadow: var(--shadow-lg);
    z-index: 100;
    display: flex;
    flex-direction: column;
    animation: slideIn 240ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  @keyframes slideIn {
    from { transform: translateX(40px); opacity: 0.6; }
    to   { transform: translateX(0); opacity: 1; }
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-2);
  }
  header h3 {
    margin: 0;
    font-size: var(--font-size-lg);
    letter-spacing: -0.01em;
  }
  .close { padding: 0.35rem 0.5rem; color: var(--fg-3); }
  .close:hover { color: var(--fg-0); }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem;
  }
</style>
