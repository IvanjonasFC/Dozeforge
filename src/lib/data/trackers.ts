// DozeForge's own list of well-known third-party SDK package signatures. Each
// `signature` is the public, factual package/class prefix these widely-used SDKs
// ship under (e.g. Firebase Analytics is `com.google.firebase.analytics`); when
// such a prefix appears in an app's declared components (from `dumpsys package`),
// the SDK is bundled. These identifiers are public facts, independently compiled
// here — this is NOT the Exodus Privacy database. It's a component-based
// heuristic (not a full dex scan) that catches the common trackers.

export type Tracker = { name: string; category: string; signature: string };

export const TRACKERS: Tracker[] = [
  { name: 'Google AdMob', category: 'Ads', signature: 'com.google.android.gms.ads' },
  { name: 'Google Analytics', category: 'Analytics', signature: 'com.google.android.gms.analytics' },
  { name: 'Google Firebase Analytics', category: 'Analytics', signature: 'com.google.firebase.analytics' },
  { name: 'Google Firebase Messaging', category: 'Notifications', signature: 'com.google.firebase.messaging' },
  { name: 'Google Crashlytics', category: 'Crash reporting', signature: 'com.google.firebase.crashlytics' },
  { name: 'Google Tag Manager', category: 'Analytics', signature: 'com.google.android.gms.tagmanager' },
  { name: 'Facebook Ads', category: 'Ads', signature: 'com.facebook.ads' },
  { name: 'Facebook Login', category: 'Identification', signature: 'com.facebook.login' },
  { name: 'Facebook Analytics', category: 'Analytics', signature: 'com.facebook.appevents' },
  { name: 'AppsFlyer', category: 'Analytics', signature: 'com.appsflyer' },
  { name: 'Adjust', category: 'Analytics', signature: 'com.adjust.sdk' },
  { name: 'Flurry', category: 'Analytics', signature: 'com.flurry' },
  { name: 'Amplitude', category: 'Analytics', signature: 'com.amplitude' },
  { name: 'Mixpanel', category: 'Analytics', signature: 'com.mixpanel' },
  { name: 'Segment', category: 'Analytics', signature: 'com.segment.analytics' },
  { name: 'Branch', category: 'Analytics', signature: 'io.branch' },
  { name: 'OneSignal', category: 'Notifications', signature: 'com.onesignal' },
  { name: 'Unity Ads', category: 'Ads', signature: 'com.unity3d.ads' },
  { name: 'Unity3d Services', category: 'Ads', signature: 'com.unity3d.services' },
  { name: 'ironSource', category: 'Ads', signature: 'com.ironsource' },
  { name: 'AppLovin', category: 'Ads', signature: 'com.applovin' },
  { name: 'Vungle', category: 'Ads', signature: 'com.vungle' },
  { name: 'InMobi', category: 'Ads', signature: 'com.inmobi' },
  { name: 'Chartboost', category: 'Ads', signature: 'com.chartboost' },
  { name: 'Bugsnag', category: 'Crash reporting', signature: 'com.bugsnag' },
  { name: 'Sentry', category: 'Crash reporting', signature: 'io.sentry' },
  { name: 'Microsoft AppCenter', category: 'Crash reporting', signature: 'com.microsoft.appcenter' },
  { name: 'Braze', category: 'Analytics', signature: 'com.braze' },
  { name: 'Braze (Appboy)', category: 'Analytics', signature: 'com.appboy' },
  { name: 'Kochava', category: 'Analytics', signature: 'com.kochava' },
  { name: 'MoEngage', category: 'Analytics', signature: 'com.moengage' },
  { name: 'Yandex Metrica', category: 'Analytics', signature: 'com.yandex.metrica' },
  { name: 'Tapjoy', category: 'Ads', signature: 'com.tapjoy' },
  { name: 'MoPub', category: 'Ads', signature: 'com.mopub' },
  { name: 'Comscore', category: 'Analytics', signature: 'com.comscore' },
  { name: 'Localytics', category: 'Analytics', signature: 'com.localytics' },
  { name: 'New Relic', category: 'Analytics', signature: 'com.newrelic' },
  { name: 'Huawei Mobile Services', category: 'Identification', signature: 'com.huawei.hms' },
  { name: 'ByteDance / Pangle', category: 'Ads', signature: 'com.bytedance.sdk' },
  { name: 'Pangle', category: 'Ads', signature: 'com.bytedance.pangle' },
];
