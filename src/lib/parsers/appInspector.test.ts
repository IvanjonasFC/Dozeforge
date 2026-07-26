import { describe, it, expect } from 'vitest';
import { parseAppInspector } from './appInspector';

// Representative `dumpsys package <pkg>` excerpt (Android 14).
const FIXTURE = `Packages:
  Package [com.example.app] (a1b2c3):
    userId=10234
    versionCode=451 minSdk=26 targetSdk=34
    versionName=3.2.1
    installerPackageName=com.android.vending
    firstInstallTime=2024-01-02 10:00:00
    lastUpdateTime=2024-06-15 12:30:00
    flags=[ HAS_CODE ALLOW_BACKUP ]
    requested permissions:
      android.permission.INTERNET
      android.permission.CAMERA
      android.permission.ACCESS_FINE_LOCATION
      android.permission.RECEIVE_BOOT_COMPLETED
    install permissions:
      android.permission.INTERNET: granted=true
      android.permission.RECEIVE_BOOT_COMPLETED: granted=true
    runtime permissions:
      android.permission.CAMERA: granted=false, flags=[ USER_SET ]
      android.permission.ACCESS_FINE_LOCATION: granted=true, flags=[ USER_SET ]
`;

describe('parseAppInspector', () => {
  it('extracts metadata', () => {
    const r = parseAppInspector(FIXTURE);
    expect(r.versionName).toBe('3.2.1');
    expect(r.versionCode).toBe('451');
    expect(r.targetSdk).toBe('34');
    expect(r.installer).toBe('com.android.vending');
    expect(r.flags).toContain('HAS_CODE');
  });

  it('lists permissions with grant state', () => {
    const r = parseAppInspector(FIXTURE);
    const cam = r.permissions.find((p) => p.name.endsWith('CAMERA'));
    expect(cam?.granted).toBe(false);
    const loc = r.permissions.find((p) => p.name.endsWith('ACCESS_FINE_LOCATION'));
    expect(loc?.granted).toBe(true);
    const boot = r.permissions.find((p) => p.name.endsWith('RECEIVE_BOOT_COMPLETED'));
    expect(boot?.granted).toBe(true);
  });

  it('flags dangerous permissions and sorts them first', () => {
    const r = parseAppInspector(FIXTURE);
    expect(r.permissions[0]!.dangerous).toBe(true);
    expect(r.permissions.some((p) => p.name.endsWith('CAMERA') && p.dangerous)).toBe(true);
    // INTERNET is not privacy-dangerous
    expect(r.permissions.find((p) => p.name.endsWith('INTERNET'))!.dangerous).toBe(false);
  });

  it('tolerates CRLF and unrelated input', () => {
    expect(parseAppInspector(FIXTURE.replace(/\n/g, '\r\n')).versionName).toBe('3.2.1');
    expect(parseAppInspector('garbage').permissions).toEqual([]);
  });
});
