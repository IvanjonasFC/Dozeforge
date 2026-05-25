/**
 * Generic TTL-aware cache store for ADB-bound data.
 *
 * Each entry is keyed by an arbitrary string (e.g. "privacy:38121FDJG00H7T")
 * and holds a typed value, the time it was fetched, and its TTL. Calling
 * `getOrFetch` returns the cached value if still fresh, otherwise runs the
 * fetcher and stores the result.
 *
 * The store is reactive — Svelte 5 components subscribed to `lastFetched(key)`
 * via $derived will re-render when the timestamp moves.
 *
 * Cross-tab navigation no longer triggers full reloads: the second time a
 * user opens a page within the TTL window, data renders instantly.
 */

interface Entry<T> {
  value: T;
  fetchedAt: number;
  ttlMs: number;
}

class CacheStore {
  // SvelteMap is reactive when used inside .svelte.ts — plain Map is not,
  // but for our purposes only `lastFetched` needs reactivity and we expose
  // a $state-tracked timestamp counter for that.
  private entries = new Map<string, Entry<unknown>>();
  private bumpCounter = $state(0);

  /**
   * Returns cached value if fresh, otherwise fetches and stores.
   * Errors from the fetcher are propagated; the cache is NOT polluted with
   * partial/failed responses.
   */
  async getOrFetch<T>(
    key: string,
    ttlMs: number,
    fetcher: () => Promise<T>
  ): Promise<T> {
    const entry = this.entries.get(key) as Entry<T> | undefined;
    const now = Date.now();
    if (entry && now - entry.fetchedAt < entry.ttlMs) {
      return entry.value;
    }
    const value = await fetcher();
    this.entries.set(key, { value, fetchedAt: now, ttlMs });
    this.bumpCounter++;
    return value;
  }

  /**
   * Returns the cached value WITHOUT triggering a fetch.
   * Useful for derived data (e.g. health score computed from multiple sources).
   */
  peek<T>(key: string): T | null {
    const entry = this.entries.get(key) as Entry<T> | undefined;
    if (!entry) return null;
    return entry.value;
  }

  /** Returns the epoch ms at which `key` was last fetched, or null. */
  lastFetched(key: string): number | null {
    // Reading bumpCounter ties this fn into the reactive graph so consumers
    // re-evaluate when any cache write occurs.
    void this.bumpCounter;
    const entry = this.entries.get(key);
    return entry ? entry.fetchedAt : null;
  }

  /** Invalidates a single entry. Next read will trigger a fetch. */
  invalidate(key: string): void {
    if (this.entries.delete(key)) this.bumpCounter++;
  }

  /** Invalidates every entry matching a key prefix (e.g. "privacy:"). */
  invalidatePrefix(prefix: string): void {
    let removed = 0;
    for (const k of this.entries.keys()) {
      if (k.startsWith(prefix)) {
        this.entries.delete(k);
        removed++;
      }
    }
    if (removed > 0) this.bumpCounter++;
  }

  /** Flush everything — used on device disconnect. */
  invalidateAll(): void {
    if (this.entries.size > 0) {
      this.entries.clear();
      this.bumpCounter++;
    }
  }

  /** Debug helper: list all keys + ages. */
  inspect(): Array<{ key: string; ageMs: number; ttlMs: number }> {
    const now = Date.now();
    return Array.from(this.entries.entries()).map(([k, e]) => ({
      key: k,
      ageMs: now - e.fetchedAt,
      ttlMs: e.ttlMs
    }));
  }
}

export const cache = new CacheStore();

/**
 * TTL presets — exported so call sites use the same constants.
 * Adjust here if any module proves too aggressive or too stale.
 */
export const TTL = {
  /** Volatile data that should refresh quickly. Battery level, processes. */
  short: 30_000,            // 30s
  /** Slowly-changing data. Sleep stats, package lists. */
  medium: 5 * 60_000,       // 5 min
  /** Near-static data. DNS settings, system tweaks. */
  long: 15 * 60_000,        // 15 min
  /** APK inventory — only changes when apps are installed/removed. */
  inventory: 24 * 60 * 60_000  // 24 h
} as const;