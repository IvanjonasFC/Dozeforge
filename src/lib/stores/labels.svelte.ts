/**
 * Reactive label store for `package -> human-readable app name`.
 *
 * # Design
 *
 * Resolving labels for every installed package via ADB takes 30-90 seconds.
 * Blocking the UI on that scan would be unacceptable, so this store does:
 *
 *   1. On `hydrate(serial)`, fire `api.resolveAppLabels(serial)` once in
 *      the background. The first call per device is slow; subsequent calls
 *      from any page hit the in-memory map.
 *   2. UI components use `labelFor(pkg)` synchronously. While the scan is
 *      pending they get an instant fallback (last segment of the package
 *      name), so the table still reads naturally.
 *   3. When the scan resolves, the Svelte 5 rune triggers re-render and
 *      every `<AppName>` swaps to the real label.
 *
 * Hydration is automatically invalidated when the user picks a different
 * device. We also expose `forceRefresh(serial)` for the "Reload" button
 * users see on Bloatware / Telemetry.
 *
 * The store is also responsible for the deterministic per-package color
 * used by the icon-letter component, so the same package always renders
 * with the same hue across the whole app.
 */

import { api, DozeForgeError } from '$tauri/api';

interface DeviceState {
  /** Resolved labels indexed by package name. */
  labels: Map<string, string>;
  /** Promise of the in-flight resolution, if any. */
  pending: Promise<void> | null;
  /** Last error, surfaced to the UI for diagnostics. */
  error: string | null;
  /** Epoch ms when the full scan finished. */
  resolvedAt: number | null;
}

function emptyDeviceState(): DeviceState {
  return { labels: new Map(), pending: null, error: null, resolvedAt: null };
}

class LabelStore {
  /** Keyed by device serial. */
  private byDevice = new Map<string, DeviceState>();
  /** Bump counter — read inside derived expressions to make consumers reactive. */
  private bump = $state(0);

  private deviceFor(serial: string): DeviceState {
    let s = this.byDevice.get(serial);
    if (!s) {
      s = emptyDeviceState();
      this.byDevice.set(serial, s);
    }
    return s;
  }

  /**
   * Lazily fires a full label scan for `serial` if one hasn't completed
   * or isn't already in flight. Returns the in-flight promise so callers
   * that want to await completion (e.g. tests, "warm up" effects) can.
   *
   * Resolves silently — the scan failing should never crash the UI; the
   * fallback labels render fine.
   */
  hydrate(serial: string): Promise<void> {
    const state = this.deviceFor(serial);
    if (state.resolvedAt !== null) {
      return Promise.resolve();
    }
    if (state.pending) {
      return state.pending;
    }
    state.error = null;
    state.pending = api
      .resolveAppLabels(serial)
      .then((map) => {
        state.labels = new Map(Object.entries(map));
        state.resolvedAt = Date.now();
        this.bump++;
      })
      .catch((e: unknown) => {
        const msg = e instanceof DozeForgeError ? e.message : String(e);
        state.error = msg;
        this.bump++;
      })
      .finally(() => {
        state.pending = null;
      });
    return state.pending;
  }

  /**
   * Drops the cached labels for one device and triggers a fresh hydrate.
   * Use after installing / uninstalling apps via Bloatware.
   */
  forceRefresh(serial: string): Promise<void> {
    this.byDevice.delete(serial);
    this.bump++;
    return this.hydrate(serial);
  }

  /** Removes everything. Called when the device list changes drastically. */
  clearAll(): void {
    if (this.byDevice.size > 0) {
      this.byDevice.clear();
      this.bump++;
    }
  }

  /**
   * Synchronous lookup. Always returns *something* renderable:
   *   - The resolved label if present.
   *   - The last `.`-separated segment of the package otherwise.
   *
   * Reading `this.bump` here is what makes this method reactive: any
   * Svelte 5 `$derived(labelStore.labelFor(pkg))` will re-evaluate when
   * the bump counter changes.
   */
  labelFor(serial: string | null, pkg: string): string {
    void this.bump;
    if (!serial) return fallbackLabel(pkg);
    const state = this.byDevice.get(serial);
    if (!state) return fallbackLabel(pkg);
    const resolved = state.labels.get(pkg);
    if (resolved && resolved.length > 0) return resolved;
    return fallbackLabel(pkg);
  }

  /** Returns true while the full scan is still in flight for this device. */
  isResolving(serial: string | null): boolean {
    void this.bump;
    if (!serial) return false;
    const state = this.byDevice.get(serial);
    return state?.pending !== null && state?.pending !== undefined;
  }

  /** True once the full scan has completed at least once for this device. */
  hasResolved(serial: string | null): boolean {
    void this.bump;
    if (!serial) return false;
    return this.byDevice.get(serial)?.resolvedAt !== null;
  }

  errorFor(serial: string | null): string | null {
    void this.bump;
    if (!serial) return null;
    return this.byDevice.get(serial)?.error ?? null;
  }

  /**
   * Total number of resolved labels for diagnostics.
   * Used by the "Loaded X app names" tooltip.
   */
  countFor(serial: string | null): number {
    void this.bump;
    if (!serial) return 0;
    return this.byDevice.get(serial)?.labels.size ?? 0;
  }
}

function fallbackLabel(pkg: string): string {
  const last = pkg.split('.').filter(Boolean).pop();
  if (!last) return pkg;
  // Up-case first letter so it reads as a name, not as a code symbol.
  return last.charAt(0).toUpperCase() + last.slice(1);
}

/**
 * Deterministic hue 0..360 from package name. Used by `<AppName>` to pick
 * the colored chip background. The same package always returns the same hue
 * across runs because the hash is purely a function of the string.
 */
export function packageHue(pkg: string): number {
  let h = 0;
  for (let i = 0; i < pkg.length; i++) {
    h = (h * 31 + pkg.charCodeAt(i)) & 0xffffffff;
  }
  // Bias against pure red (reserved for warnings) by shifting +25.
  return (Math.abs(h) + 25) % 360;
}

/**
 * First display letter for the icon chip. Strips `com.` / `org.` etc and
 * uses the first character of the most distinctive segment.
 */
export function packageInitial(pkg: string, resolvedLabel?: string): string {
  // Prefer the resolved label's first character when it looks like a word.
  if (resolvedLabel) {
    const t = resolvedLabel.trim();
    if (t.length > 0 && /[a-zA-Z0-9]/.test(t.charAt(0))) {
      return t.charAt(0).toUpperCase();
    }
  }
  const segments = pkg.split('.').filter(Boolean);
  // Drop leading common TLD-style prefixes.
  const skip = new Set(['com', 'org', 'net', 'io', 'co', 'app', 'android']);
  const meaningful = segments.find((s) => !skip.has(s)) ?? segments[segments.length - 1] ?? '?';
  return meaningful.charAt(0).toUpperCase();
}

export const labelStore = new LabelStore();
