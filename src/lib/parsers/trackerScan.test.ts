import { describe, it, expect } from 'vitest';
import { scanForTrackers } from './trackerScan';

const DUMPSYS = `
  Package [com.example.app]:
    Activity com.google.android.gms.ads.AdActivity
    Service com.google.firebase.crashlytics.CrashlyticsService
    Provider com.facebook.ads.AudienceNetworkContentProvider
    Service com.appsflyer.AppsFlyerService
    Activity com.example.app.MainActivity
`;

describe('scanForTrackers', () => {
  it('detects bundled tracker SDKs from component signatures', () => {
    const found = scanForTrackers(DUMPSYS).map((t) => t.name);
    expect(found).toContain('Google AdMob');
    expect(found).toContain('Google Crashlytics');
    expect(found).toContain('Facebook Ads');
    expect(found).toContain('AppsFlyer');
  });

  it('does not flag base Play Services (no ads/analytics sub-package)', () => {
    const clean = 'Service com.google.android.gms.common.GmsService';
    expect(scanForTrackers(clean)).toEqual([]);
  });

  it('deduplicates and returns empty for tracker-free apps', () => {
    expect(scanForTrackers('com.example.foo.Bar')).toEqual([]);
  });
});
