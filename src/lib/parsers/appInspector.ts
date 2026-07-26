// Parser for `dumpsys package <pkg>` — extracts app metadata + the requested
// permissions and their grant state. Pure/dependency-free for unit testing.

export type AppPermission = { name: string; granted: boolean | null; dangerous: boolean };

export type AppInspection = {
  versionName: string | null;
  versionCode: string | null;
  minSdk: string | null;
  targetSdk: string | null;
  installer: string | null;
  firstInstall: string | null;
  lastUpdate: string | null;
  flags: string[];
  permissions: AppPermission[];
};

// A pragmatic set of privacy-sensitive permission leaf names to flag.
const DANGEROUS = new Set([
  'CAMERA', 'RECORD_AUDIO', 'READ_CONTACTS', 'WRITE_CONTACTS', 'READ_SMS', 'SEND_SMS',
  'RECEIVE_SMS', 'READ_CALL_LOG', 'WRITE_CALL_LOG', 'CALL_PHONE', 'READ_PHONE_STATE',
  'ACCESS_FINE_LOCATION', 'ACCESS_COARSE_LOCATION', 'ACCESS_BACKGROUND_LOCATION',
  'BODY_SENSORS', 'ACTIVITY_RECOGNITION', 'READ_CALENDAR', 'WRITE_CALENDAR',
  'READ_EXTERNAL_STORAGE', 'WRITE_EXTERNAL_STORAGE', 'MANAGE_EXTERNAL_STORAGE',
  'SYSTEM_ALERT_WINDOW', 'REQUEST_INSTALL_PACKAGES', 'QUERY_ALL_PACKAGES',
  'READ_PHONE_NUMBERS', 'GET_ACCOUNTS', 'READ_MEDIA_IMAGES', 'READ_MEDIA_VIDEO', 'READ_MEDIA_AUDIO',
]);

function leaf(name: string): string {
  const parts = name.split('.');
  return parts[parts.length - 1] ?? name;
}

export function parseAppInspector(raw: string): AppInspection {
  const lines = raw.split(/\r?\n/);
  const first = (re: RegExp): string | null => { const m = raw.match(re); return m ? (m[1] ?? null) : null; };

  const versionName = first(/versionName=(\S+)/);
  const versionCode = first(/versionCode=(\d+)/);
  const minSdk = first(/minSdk=(\d+)/);
  const targetSdk = first(/targetSdk=(\d+)/);
  const installer = first(/installerPackageName=(\S+)/) ?? first(/installInitiator=(\S+)/);
  const firstInstall = first(/firstInstallTime=(\S+)/);
  const lastUpdate = first(/lastUpdateTime=(\S+)/);
  const flagsRaw = first(/\b(?:pkg)?[Ff]lags=\[([^\]]*)\]/);
  const flags = flagsRaw ? flagsRaw.trim().split(/\s+/).filter(Boolean) : [];

  const perms = new Map<string, boolean | null>();

  // Grant states (install + runtime blocks): "android.permission.X: granted=true"
  for (const line of lines) {
    const gm = line.trim().match(/^([\w.]+):\s*granted=(true|false)/);
    if (gm) perms.set(gm[1]!, gm[2] === 'true');
  }

  // Requested-permissions block: bare "android.permission.X" leaf lines.
  let inReq = false;
  for (const line of lines) {
    const t = line.trim();
    if (/^requested permissions:/i.test(t)) { inReq = true; continue; }
    if (inReq) {
      const pm = t.match(/^([\w.]+\.[\w.]+)$/);
      if (pm) { if (!perms.has(pm[1]!)) perms.set(pm[1]!, null); }
      else if (t === '' || t.includes(':') || /permissions:/i.test(t)) inReq = false;
    }
  }

  const permissions: AppPermission[] = [...perms.entries()]
    .map(([name, granted]) => ({ name, granted, dangerous: DANGEROUS.has(leaf(name)) }))
    .sort((a, b) => Number(b.dangerous) - Number(a.dangerous) || a.name.localeCompare(b.name));

  return { versionName, versionCode, minSdk, targetSdk, installer, firstInstall, lastUpdate, flags, permissions };
}
