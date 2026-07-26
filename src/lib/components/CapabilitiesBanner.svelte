<script lang="ts">
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';

  const missing = $derived.by(() => {
    const caps = deviceStore.capabilities;
    if (!caps) return [];
    const labels: Record<keyof typeof caps, string> = {
      appops_set: 'cmd appops set',
      appops_get: 'cmd appops get',
      am_set_standby_bucket: 'am set-standby-bucket',
      pm_disable_user: 'pm disable-user',
      device_config_put: 'device_config put',
      dumpsys_jobscheduler: 'dumpsys jobscheduler',
      dumpsys_deviceidle: 'dumpsys deviceidle',
      dumpsys_sensorservice: 'dumpsys sensorservice',
      write_secure_settings: 'WRITE_SECURE_SETTINGS'
    };
    return (Object.keys(caps) as (keyof typeof caps)[])
      .filter((k) => !caps[k])
      .map((k) => labels[k]);
  });
</script>

{#if missing.length > 0}
  <div class="warn">
    <strong>{i18n.t('Limited ROM capabilities detected.')}</strong>
    {i18n.t('The following primitives are unavailable on this device:')}
    <span class="mono">{missing.join(', ')}</span>.
    {i18n.t('Affected optimisations will be disabled in the UI.')}
  </div>
{/if}

<style>
  .warn {
    border-left: 3px solid var(--warn);
    background: rgba(250, 204, 21, 0.08);
    padding: 0.75rem 1rem;
    border-radius: var(--radius);
    color: var(--fg-0);
    font-size: 0.85rem;
    line-height: 1.5;
  }
</style>
