import { TRACKERS, type Tracker } from '$lib/data/trackers';

export type DetectedTracker = { name: string; category: string };

// Heuristic tracker scan: looks for each tracker's class/package signature in the
// `dumpsys package <pkg>` output (declared components + references). Not a full
// dex scan, but reliably catches SDKs that register components/providers.
export function scanForTrackers(dumpsysOutput: string, db: Tracker[] = TRACKERS): DetectedTracker[] {
  const found: DetectedTracker[] = [];
  const seen = new Set<string>();
  for (const t of db) {
    if (seen.has(t.name)) continue;
    if (dumpsysOutput.includes(t.signature)) {
      seen.add(t.name);
      found.push({ name: t.name, category: t.category });
    }
  }
  return found.sort((a, b) => a.category.localeCompare(b.category) || a.name.localeCompare(b.name));
}
