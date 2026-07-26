<script lang="ts">
  import { api } from '$lib/tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';

  let { address = '', open = $bindable(false) } = $props<{ address: string; open: boolean }>();

  let pin = $state('');
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function handlePair() {
    if (!pin || pin.length < 6) return;
    loading = true;
    error = null;
    try {
      await api.adbPair(address, pin);
      // Success! Now we wait for the user to select the _adb-tls-connect._tcp to connect,
      // or we can auto-refresh the device list.
      await deviceStore.refresh();
      open = false;
      pin = '';
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

{#if open}
  <div class="modal-backdrop" onclick={() => open = false} role="presentation">
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <h3>{i18n.t('Pair Device')}</h3>
      <p class="muted small">
        {i18n.t('Enter the 6-digit Wi-Fi pairing code from your Android device for')} <strong>{address}</strong>.
      </p>

      {#if error}
        <div class="error" style="margin-bottom: 1rem;">{error}</div>
      {/if}

      <div style="margin-bottom: 1.5rem;">
        <input 
          type="text" 
          placeholder="000000" 
          bind:value={pin} 
          disabled={loading}
          class="pin-input"
          maxlength="6"
          autocomplete="off"
        />
      </div>

      <div class="modal-actions">
        <button onclick={() => open = false} disabled={loading}>{i18n.t('Cancel')}</button>
        <button class="primary" onclick={handlePair} disabled={loading || pin.length < 6}>
          {loading ? i18n.t('Pairing...') : i18n.t('Pair')}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 1.5rem;
    width: 320px;
    box-shadow: 0 10px 40px rgba(0,0,0,0.5);
  }
  .modal h3 {
    margin: 0 0 0.5rem 0;
  }
  .pin-input {
    width: 100%;
    text-align: center;
    font-size: 2rem;
    letter-spacing: 0.5rem;
    font-family: var(--font-mono);
    padding: 0.5rem;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
