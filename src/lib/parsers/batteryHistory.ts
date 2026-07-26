// Client-side parser for `dumpsys batterystats --history` (Format 2, Android 8+).
// Kept as a pure, dependency-free module so it can be unit-tested with vitest.

export type BatteryEvent = {
  t: number;
  level: number | null;
  temp: number | null;
  screen: boolean;
  charging: boolean;
  wake: boolean;
  doze: boolean;
};

export type BatteryHistory = {
  events: BatteryEvent[];
  /** Wakelock holders ranked by total held time (ms). */
  holders: { tag: string; ms: number }[];
};

export function parseBatteryHistory(raw: string): BatteryHistory {
  // Split on \r?\n — ADB on Windows emits \r\n.
  const lines = raw.split(/\r?\n/);
  let inHistory = false;
  let t0: number | null = null;
  let level: number | null = null;
  let temp: number | null = null;
  let screen = false, charging = false, wake = false, doze = false;
  const out: BatteryEvent[] = [];

  const wakeMs = new Map<string, number>();
  let curWake: { tag: string; start: number } | null = null;

  // Format 2: "MM-DD HH:MM:SS.mmm <level> <+flag -flag key=val …>"
  const re = /^\s*(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\.(\d{3})\s+(.*)/;

  for (const line of lines) {
    if (!inHistory) {
      if (line.includes('Battery History')) inHistory = true;
      continue;
    }
    const m = re.exec(line);
    if (!m) continue;

    const rest = m[7] ?? '';
    // Skip metadata rows (TIME:, RESET:, START, SHUTDOWN, …) — they carry no state.
    if (/^[A-Z][A-Z_]*:/.test(rest.trim())) continue;

    const ts = Date.UTC(2024, (+m[1]!) - 1, +m[2]!, +m[3]!, +m[4]!, +m[5]!, +m[6]!);
    if (t0 === null) t0 = ts;
    const t = ts - t0;
    const tokens = rest.trim().split(/\s+/);

    if (tokens[0] && /^\d{1,3}$/.test(tokens[0])) level = Math.min(100, parseInt(tokens[0], 10));
    const tm = rest.match(/\btemp=(\d+)/);
    if (tm) temp = parseInt(tm[1]!, 10) / 10;

    for (const tok of tokens) {
      if (tok === '+screen') screen = true;
      else if (tok === '-screen') screen = false;
      else if (tok === 'status=charging' || tok === 'status=full') charging = true;
      else if (tok === 'status=discharging' || tok === 'status=not-charging' || tok === 'plug=none') charging = false;
      else if (tok === 'device_idle=full' || tok === 'device_idle=light') doze = true;
      else if (tok === 'device_idle=off' || tok === '-device_idle') doze = false;
    }

    const wlOn = rest.match(/\+wake_lock=(\d+):"([^"]*)"/);
    const wlOff = /-wake_lock\b/.test(rest);
    if (wlOn) {
      if (curWake) wakeMs.set(curWake.tag, (wakeMs.get(curWake.tag) ?? 0) + (t - curWake.start));
      curWake = { tag: (wlOn[2] || `uid ${wlOn[1]}`).slice(0, 60), start: t };
      wake = true;
    }
    if (wlOff) {
      if (curWake) { wakeMs.set(curWake.tag, (wakeMs.get(curWake.tag) ?? 0) + (t - curWake.start)); curWake = null; }
      wake = false;
    }

    out.push({ t, level, temp, screen, charging, wake, doze });
    if (out.length > 8000) break;
  }
  if (curWake) {
    wakeMs.set(curWake.tag, (wakeMs.get(curWake.tag) ?? 0) + ((out[out.length - 1]?.t ?? curWake.start) - curWake.start));
  }

  const holders = [...wakeMs.entries()]
    .map(([tag, ms]) => ({ tag, ms }))
    .filter((h) => h.ms > 0)
    .sort((a, b) => b.ms - a.ms)
    .slice(0, 6);

  return { events: out, holders };
}
