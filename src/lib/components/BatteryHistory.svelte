<script lang="ts">
  import { api, DozeForgeError } from '$tauri/api';
  import { deviceStore } from '$stores/device.svelte';
  import { i18n } from '$stores/i18n.svelte';
  import { parseBatteryHistory, type BatteryEvent } from '$lib/parsers/batteryHistory';

  // F8 — Battery Historian (lite). Parsing lives in `$lib/parsers/batteryHistory`
  // (unit-tested); this component renders battery level + temperature, plus
  // screen / charging / doze / wakelock lanes and the top wakelock holders.

  let loading = $state(false);
  let error = $state<string | null>(null);
  let events = $state<BatteryEvent[]>([]);
  let total = $state(0);
  let holders = $state<{ tag: string; ms: number }[]>([]);
  let rawText = $state('');
  let showRaw = $state(false);

  async function load() {
    if (!deviceStore.selected) return;
    loading = true; error = null; events = []; holders = []; showRaw = false;
    try {
      const raw = await api.runShell(deviceStore.selected.serial, 'dumpsys batterystats --history');
      rawText = raw;
      const res = parseBatteryHistory(raw);
      events = res.events;
      holders = res.holders;
      total = res.events.length ? (res.events[res.events.length - 1]?.t ?? 0) : 0;
      if (res.events.length === 0) {
        error = i18n.t('No battery history found. The phone must have run on battery (unplugged) for a while.');
      } else if (total === 0) {
        error = i18n.t('Parsed {{n}} events but all timestamps are 0 — this ROM uses a different history format. Raw output below so the parser can be adapted.', { n: res.events.length });
      }
    } catch (e) {
      error = (e as DozeForgeError).message;
    } finally {
      loading = false;
    }
  }


  // ---- Rendering helpers ----
  const W = 1000, H = 140;
  const tempRange = $derived.by(() => {
    const temps = events.map((e) => e.temp).filter((x): x is number => x != null);
    if (temps.length === 0) return null;
    const min = Math.min(...temps), max = Math.max(...temps);
    return { min, max: max === min ? min + 1 : max };
  });
  const levelPts = $derived.by(() => {
    if (!total) return '';
    return events.filter((e) => e.level != null)
      .map((e) => `${((e.t / total) * W).toFixed(1)},${(H - (e.level! / 100) * H).toFixed(1)}`).join(' ');
  });
  const tempPts = $derived.by(() => {
    const r = tempRange;
    if (!total || !r) return '';
    return events.filter((e) => e.temp != null)
      .map((e) => `${((e.t / total) * W).toFixed(1)},${(H - ((e.temp! - r.min) / (r.max - r.min)) * H).toFixed(1)}`).join(' ');
  });

  function lane(key: 'screen' | 'charging' | 'wake' | 'doze') {
    const segs: { x: number; w: number }[] = [];
    if (!total) return segs;
    let startT: number | null = null;
    for (let i = 0; i < events.length; i++) {
      const on = !!events[i]![key];
      if (on && startT === null) startT = events[i]!.t;
      if (!on && startT !== null) {
        segs.push({ x: (startT / total) * W, w: Math.max(1, ((events[i]!.t - startT) / total) * W) });
        startT = null;
      }
    }
    if (startT !== null) segs.push({ x: (startT / total) * W, w: Math.max(1, ((total - startT) / total) * W) });
    return segs;
  }
  const screenSegs = $derived(lane('screen'));
  const chargeSegs = $derived(lane('charging'));
  const dozeSegs = $derived(lane('doze'));
  const wakeSegs = $derived(lane('wake'));

  function fmtDur(ms: number): string {
    const s = Math.round(ms / 1000);
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${sec}s`;
    return `${sec}s`;
  }
</script>

<div class="card">
  <div class="bh-head">
    <div>
      <h3>{i18n.t('Battery timeline')}</h3>
      <p class="muted small">{i18n.t('Battery level, temperature, screen, charging, doze and wakelocks over the last charge cycle.')}</p>
    </div>
    <button class="primary" onclick={load} disabled={loading || !deviceStore.selected}>
      {loading ? i18n.t('Loading…') : i18n.t('Load history')}
    </button>
  </div>

  {#if error}
    <div class="warn-box" style="margin-top: 1rem;">{error}</div>
    {#if rawText.trim()}
      <button class="btn outline small" style="margin-top: 0.75rem;" onclick={() => showRaw = !showRaw}>
        {showRaw ? i18n.t('Hide raw output') : i18n.t('Show raw output')}
      </button>
      {#if showRaw}<pre class="raw">{rawText.slice(0, 4000)}</pre>{/if}
    {/if}
  {:else if events.length > 0}
    <div class="bh-meta">
      {i18n.t('Span:')} {fmtDur(total)} · {events.length} {i18n.t('events')}
      {#if tempRange}· {i18n.t('Temp:')} {tempRange.min.toFixed(1)}–{tempRange.max.toFixed(1)}°C{/if}
    </div>
    <svg class="bh-svg" viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="Battery level and temperature over time">
      <line x1="0" y1="0" x2={W} y2="0" stroke="var(--border)" stroke-width="1" />
      <line x1="0" y1={H / 2} x2={W} y2={H / 2} stroke="var(--border)" stroke-width="0.5" stroke-dasharray="4 4" />
      <line x1="0" y1={H} x2={W} y2={H} stroke="var(--border)" stroke-width="1" />
      {#if tempPts}<polyline points={tempPts} fill="none" stroke="var(--warn)" stroke-width="1.2" stroke-opacity="0.7" />{/if}
      {#if levelPts}<polyline points={levelPts} fill="none" stroke="url(#bhg)" stroke-width="2.5" />{/if}
      <defs><linearGradient id="bhg" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#FF6B00" /><stop offset="1" stop-color="#FF3C00" /></linearGradient></defs>
    </svg>
    <div class="bh-axis">
      <span>{i18n.t('Battery')} 0–100% <span style="color: var(--warn);">· {i18n.t('temp')}</span></span>
      <span>{i18n.t('now')}</span>
    </div>

    <div class="lanes">
      {#each [{ label: i18n.t('Screen on'), segs: screenSegs, color: 'var(--good)' }, { label: i18n.t('Charging'), segs: chargeSegs, color: '#38BDF8' }, { label: i18n.t('Doze'), segs: dozeSegs, color: '#A78BFA' }, { label: i18n.t('Wakelocks'), segs: wakeSegs, color: 'var(--warn)' }] as row (row.label)}
        <div class="lane">
          <span class="lane-label">{row.label}</span>
          <svg class="lane-track" viewBox="0 0 {W} 12" preserveAspectRatio="none">
            <rect x="0" y="0" width={W} height="12" fill="var(--bg-1)" rx="2" />
            {#each row.segs as seg}<rect x={seg.x} y="0" width={seg.w} height="12" fill={row.color} rx="1" />{/each}
          </svg>
        </div>
      {/each}
    </div>

    {#if holders.length > 0}
      <div class="holders">
        <div class="holders-title">{i18n.t('Top wakelock holders')}</div>
        {#each holders as h (h.tag)}
          <div class="holder">
            <span class="holder-tag mono">{h.tag}</span>
            <span class="holder-ms mono">{fmtDur(h.ms)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {:else if !loading}
    <p class="muted" style="margin-top: 1rem;">{i18n.t('Load the on-device battery history to see level, temperature, screen, charging, doze and wakelocks.')}</p>
  {/if}
</div>

<style>
  .bh-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; flex-wrap: wrap; }
  .bh-head h3 { margin: 0; }
  .bh-meta { margin-top: 1rem; color: var(--fg-2); font-size: 12.5px; font-family: var(--font-mono); }
  .bh-svg { width: 100%; height: 140px; margin-top: 0.5rem; display: block; }
  .bh-axis { display: flex; justify-content: space-between; font-size: 11px; color: var(--fg-3); }
  .lanes { margin-top: 1rem; display: flex; flex-direction: column; gap: 0.5rem; }
  .lane { display: grid; grid-template-columns: 90px 1fr; align-items: center; gap: 0.75rem; }
  .lane-label { font-size: 12px; color: var(--fg-2); }
  .lane-track { width: 100%; height: 12px; display: block; }
  .holders { margin-top: 1.25rem; }
  .holders-title { font-size: 11.5px; text-transform: uppercase; letter-spacing: 0.05em; color: var(--fg-3); margin-bottom: 0.5rem; }
  .holder { display: flex; justify-content: space-between; gap: 1rem; padding: 0.35rem 0; border-bottom: 1px solid var(--border); font-size: 12.5px; }
  .holder-tag { color: var(--fg-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .holder-ms { color: var(--warn); flex-shrink: 0; }
  .raw { margin-top: 0.75rem; max-height: 320px; overflow: auto; background: var(--bg-0); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 0.6rem; font-family: var(--font-mono); font-size: 11px; white-space: pre; color: var(--fg-2); }
</style>
