/**
 * Background cache warming — makes tab switches feel instant.
 *
 * After the current page has loaded, this trickles the other tabs' data into the
 * shared `cache` (the same keys the pages read), ONE request at a time with a
 * pause between each. That way:
 *   - ADB (a single transport) is never saturated, so the current view stays fluid.
 *   - By the time the user opens another tab, its data is already cached and the
 *     page renders instantly from `cache.peek` (no skeleton).
 *
 * Each task is a no-op if the entry is already fresh (TTL) or the user already
 * visited that tab, so nothing is fetched twice. Aborts cleanly if the selected
 * device changes mid-run.
 */

import { cache, TTL } from './cache.svelte';
import { api } from '$tauri/api';

let running = false;
let target = '';

/** Schedule a background warm for `serial`. Safe to call repeatedly. */
export function warmCache(serial: string): void {
  if (!serial) return;
  target = serial;
  if (running) return; // an in-flight run will pick up the new target
  running = true;
  // Let the current page's own fetch finish first, then trickle the rest in.
  setTimeout(() => void run(serial), 1200);
}

async function run(serial: string): Promise<void> {
  // Ordered lightest → heaviest. Heavy dumpsys (per-app drain, batterystats
  // health) go last so the common tabs warm first.
  const tasks: Array<[string, number, () => Promise<unknown>]> = [
    ['overview:' + serial, TTL.short, () => api.overviewSnapshot(serial)],
    ['privacy:' + serial, TTL.medium, () => api.getPrivacyState(serial)],
    ['packages:' + serial, TTL.medium, () => api.listPackages(serial)],
    ['tweaks:' + serial, TTL.medium, () => api.getSystemTweaks(serial)],
    ['perf:' + serial, TTL.medium, () => api.getPerformanceSettings(serial)],
    ['props:' + serial, TTL.medium, () => api.getSystemProperties(serial)],
    ['storage:' + serial, TTL.medium, () => api.storageOverview(serial)],
    ['miscat:' + serial, TTL.medium, () => api.miscategorizedApps(serial)],
    ['bloat:' + serial, TTL.long, () => api.bloatwareRecommendations(serial)],
    ['dangerous_perms:' + serial, TTL.medium, () => api.getDangerousPermissions(serial)],
    ['health:' + serial, TTL.short, () => api.batteryHealth(serial, true)],
    ['drain:' + serial, TTL.medium, () => api.batteryPerApp(serial)], // heaviest — last
  ];

  try {
    for (const [key, ttl, fetcher] of tasks) {
      if (target !== serial) break; // device changed → abort this run
      try {
        await cache.getOrFetch(key, ttl, fetcher);
      } catch {
        /* best-effort: the page will fetch/retry on its own if needed */
      }
      await pause(); // yield so the current view and input stay responsive
    }
  } finally {
    running = false;
    // A device switch happened while running → warm the new one.
    if (target && target !== serial) warmCache(target);
  }
}

/** Gap between background requests — keeps the single ADB transport uncongested. */
function pause(ms = 500): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
