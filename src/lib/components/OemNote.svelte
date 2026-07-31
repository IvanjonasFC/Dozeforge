<script lang="ts">
  // Honest, OEM-aware guidance. DozeForge controls the AOSP battery layer
  // (Doze, standby buckets, appops) on every device — but each OEM adds a
  // *private* battery layer that ADB cannot reach. Rather than pretend, we
  // detect the manufacturer and tell the user exactly where to configure it.
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';

  const maker = $derived((deviceStore.selected?.manufacturer ?? '').toLowerCase());

  const GUIDES = [
    { keys: ['samsung'], brand: 'Samsung One UI',
      body: 'One UI adds "Sleeping apps / Deep sleeping apps" on top of AOSP Doze — a private layer ADB cannot set. Configure it in Settings → Battery → Background usage limits.' },
    { keys: ['xiaomi', 'redmi', 'poco'], brand: 'Xiaomi MIUI / HyperOS',
      body: 'MIUI/HyperOS adds "Autostart" and aggressive per-app saving (private). For each app: Settings → Apps → [app] → Battery saver → No restrictions, and turn Autostart on.' },
    { keys: ['oneplus', 'oppo', 'realme'], brand: 'OnePlus / OPPO / realme (ColorOS / OxygenOS)',
      body: 'ColorOS/OxygenOS adds "Sleep standby optimization" and per-app optimization (private). Configure in Settings → Battery → More settings, and disable optimization for the apps you keep alive.' },
    { keys: ['vivo', 'iqoo'], brand: 'vivo / iQOO (Funtouch / OriginOS)',
      body: 'Funtouch/OriginOS adds "High background power consumption" and an Autostart manager (private). Set them in Settings → Battery → Background power / Autostart.' },
    { keys: ['huawei', 'honor'], brand: 'Huawei / Honor (EMUI / MagicOS)',
      body: 'EMUI/MagicOS adds "App launch" and manufacturer optimization (private). Set each app to "Manage manually" in Settings → Battery → App launch.' },
  ];

  const guide = $derived(GUIDES.find((g) => g.keys.some((k) => maker.includes(k))) ?? null);
</script>

{#if guide}
  <div class="oem-note">
    <div class="oem-head">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4"/><path d="M12 17h.01"/>
      </svg>
      <span>{i18n.t('{{brand}} adds its own battery layer', { brand: guide.brand })}</span>
    </div>
    <p class="oem-lead">{i18n.t('DozeForge already applies AOSP Doze, standby buckets and appops here — those work on your device. But this OEM adds a private layer ADB cannot reach:')}</p>
    <p class="oem-body">{i18n.t(guide.body)}</p>
  </div>
{/if}

<style>
  .oem-note {
    background: var(--warn-soft, rgba(245, 158, 11, 0.1));
    border: 1px solid rgba(245, 158, 11, 0.28);
    border-radius: var(--radius); padding: 0.8rem 1rem; margin-bottom: 1.25rem;
  }
  .oem-head { display: flex; align-items: center; gap: 0.5rem; color: var(--warn); font-weight: 600; font-size: var(--font-size-sm); }
  .oem-head svg { flex-shrink: 0; }
  .oem-lead { margin: 0.5rem 0 0.35rem; font-size: var(--font-size-xs); color: var(--fg-3); line-height: 1.5; }
  .oem-body { margin: 0; font-size: var(--font-size-sm); color: var(--fg-1); line-height: 1.55; }
</style>
