export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = s / 60;
  if (m < 60) return `${m.toFixed(1)}min`;
  const h = m / 60;
  return `${h.toFixed(1)}h`;
}

export function formatPct(n: number): string {
  return `${n.toFixed(1)}%`;
}

export function formatScore(n: number): string {
  if (n < 10) return n.toFixed(2);
  if (n < 1000) return n.toFixed(0);
  return `${(n / 1000).toFixed(1)}k`;
}

export function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString();
}

export function truncate(s: string, max = 48): string {
  if (s.length <= max) return s;
  return s.slice(0, max - 1) + '...';
}
