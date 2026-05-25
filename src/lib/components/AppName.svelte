<script lang="ts">
  /**
   * Reusable inline app identifier. Renders a colored letter chip + the
   * human-readable label, with the raw package name as a tooltip and
   * optional below-the-name subtitle.
   *
   * Usage:
   *   <AppName package="com.twitter.android" />
   *   <AppName package={row.package} size="sm" hidePackage />
   *
   * All visual state derives from the global `labelStore`. The component
   * itself is stateless — drop it in a hot list (telemetry / drain table)
   * and it stays in sync with the rest of the app for free.
   */

  import { deviceStore } from '$stores/device.svelte';
  import { labelStore, packageHue, packageInitial } from '$stores/labels.svelte';

  type Size = 'sm' | 'md' | 'lg';

  let {
    package: pkg,
    size = 'md',
    hidePackage = false,
    showInitial = true,
    inline = false,
  }: {
    package: string;
    size?: Size;
    /** Hide the secondary `com.x.y` line under the label. */
    hidePackage?: boolean;
    /** Hide the colored letter chip entirely. */
    showInitial?: boolean;
    /** Compact one-line layout (no subtitle, even if hidePackage=false). */
    inline?: boolean;
  } = $props();

  const serial = $derived(deviceStore.selected?.serial ?? null);
  const label = $derived(labelStore.labelFor(serial, pkg));
  const hue = $derived(packageHue(pkg));
  const initial = $derived(packageInitial(pkg, label));

  // Tooltip combines label + raw package so power-users can copy the
  // package name from the browser's native tooltip.
  const tooltip = $derived(label === pkg ? pkg : `${label}\n${pkg}`);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  class="app-name"
  class:size-sm={size === 'sm'}
  class:size-md={size === 'md'}
  class:size-lg={size === 'lg'}
  class:inline-mode={inline}
  class:interactive={true}
  title={tooltip}
  onclick={() => {
    import('$stores/appModal.svelte').then(m => m.appModalStore.open(pkg));
  }}
>
  {#if showInitial}
    <span
      class="chip"
      style="--chip-hue: {hue}"
      aria-hidden="true"
    >{initial}</span>
  {/if}

  <span class="labels">
    <span class="label">{label}</span>
    {#if !hidePackage && !inline && label !== pkg}
      <span class="package mono">{pkg}</span>
    {/if}
  </span>
</span>

<style>
  .app-name {
    display: inline-flex;
    align-items: center;
    gap: 0.55rem;
    min-width: 0;
    max-width: 100%;
  }
  .inline-mode { gap: 0.4rem; }
  .interactive { cursor: pointer; transition: opacity var(--t-fast); }
  .interactive:hover { opacity: 0.8; }

  .chip {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    font-weight: 700;
    font-family: var(--font-mono);
    /* Use HSL so each package gets a stable hue but consistent saturation
       and lightness. The card is a tinted background with a darker text. */
    background: hsl(var(--chip-hue) 55% 22%);
    color: hsl(var(--chip-hue) 75% 78%);
    border: 1px solid hsl(var(--chip-hue) 50% 35%);
    line-height: 1;
    user-select: none;
  }

  .size-sm .chip { width: 18px; height: 18px; font-size: 10px; }
  .size-md .chip { width: 24px; height: 24px; font-size: 12px; }
  .size-lg .chip { width: 32px; height: 32px; font-size: 15px; }

  .labels {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .inline-mode .labels {
    flex-direction: row;
    align-items: baseline;
    gap: 0.4rem;
  }

  .label {
    color: var(--fg-0);
    font-weight: 500;
    text-overflow: ellipsis;
    overflow: hidden;
    white-space: nowrap;
    max-width: 100%;
  }
  .size-sm .label { font-size: var(--font-size-xs); }
  .size-md .label { font-size: var(--font-size-sm); }
  .size-lg .label { font-size: var(--font-size-base); font-weight: 600; }

  .package {
    color: var(--fg-3);
    font-size: 10.5px;
    text-overflow: ellipsis;
    overflow: hidden;
    white-space: nowrap;
    max-width: 100%;
  }
</style>
