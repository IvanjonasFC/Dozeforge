//! Auto-exclusion list: apps that profile-based optimization must never touch.
//!
//! Even if a heuristic marks these as "Moderate" or "Elevated", restricting
//! them breaks the user's daily workflow. Users can still target them
//! manually via the Optimize page if they really want to.

use std::collections::HashSet;

use crate::parsers::PackageName;

/// Communication apps. Restricting them delays messages and calls.
const COMMUNICATION_APPS: &[&str] = &[
    // Messaging
    "com.whatsapp",
    "com.whatsapp.w4b",
    "org.telegram.messenger",
    "org.telegram.plus",
    "org.thunderdog.challegram",
    "org.thoughtcrime.securesms",
    "com.discord",
    "com.facebook.orca",
    "com.facebook.mlite",
    "com.viber.voip",
    "jp.naver.line.android",
    "com.tencent.mm",
    "com.kakao.talk",
    "com.skype.raider",
    "com.microsoft.teams",
    "us.zoom.videomeetings",
    "com.google.android.apps.tachyon",
    "com.google.android.apps.messaging",
    // Email
    "com.google.android.gm",
    "com.microsoft.office.outlook",
    "com.fsck.k9",
    "ch.protonmail.android",
    "com.yahoo.mobile.client.android.mail",
    // Calls & dialer
    "com.android.dialer",
    "com.google.android.dialer",
    "com.android.contacts",
    "com.google.android.contacts",
    // Maps & live navigation
    "com.google.android.apps.maps",
    "com.waze",
    "com.sygic.aura",
    "com.tomtom.gplay.navapp",
    // Banking (must receive push notifications instantly)
    "com.bbva.bbvacontigo",
    "es.lacaixa.mobile.android.newwapicon",
    "com.imaginbank.app",
    "com.bankinter.launcher",
    "es.bancosantander.apps",
    "com.bbva.netcash",
    "com.revolut.revolut",
    "com.wise",
    "com.paypal.android.p2pmobile",
    "com.google.android.apps.walletnfcrel",
    // Auth / 2FA
    "com.google.android.apps.authenticator2",
    "com.azure.authenticator",
    "com.duosecurity.duomobile",
    "com.authy.authy",
    "fr.acinq.phoenix",
];

/// Input method editors. Restricting them breaks the keyboard system-wide.
const INPUT_METHODS: &[&str] = &[
    "com.google.android.inputmethod.latin",
    "com.samsung.android.honeyboard",
    "com.touchtype.swiftkey",
    "com.miui.securityinputmethod",
    "com.coloros.input",
    "com.huawei.ohos.inputmethod",
    "com.sohu.inputmethod.sogou",
    "com.baidu.input",
];

/// System UI components and stores. Risk classifier catches most, this catches
/// edge cases that ship from `/data/app/` or other non-system paths on some ROMs.
const SYSTEM_CRITICAL: &[&str] = &[
    "com.android.settings",
    "com.android.permissioncontroller",
    "com.google.android.permissioncontroller",
    "com.android.vending",
    "com.android.captiveportallogin",
    "com.google.android.networkstack",
    "com.google.android.networkstack.permissionconfig",
    "com.google.android.networkstack.tethering",
    "com.android.cellbroadcastreceiver",
    "com.google.android.cellbroadcastreceiver",
    "com.android.emergency",
    "com.android.providers.downloads",
    "com.google.android.gms.location.history",
];

/// Accessibility services and assistive tech.
const ACCESSIBILITY: &[&str] = &[
    "com.google.android.marvin.talkback",
    "com.samsung.accessibility",
    "com.android.systemui.accessibility",
    "com.google.android.accessibility.switchaccess",
    "com.google.android.accessibility.soundamplifier",
];

pub struct Exclusions {
    user_overrides: HashSet<String>,
}

impl Exclusions {
    /// Default exclusion set (communication + IMEs + system + a11y).
    pub fn new_default() -> Self {
        Self {
            user_overrides: HashSet::new(),
        }
    }

    /// Add user-supplied packages to keep untouched. Stack on top of defaults.
    pub fn with_user_overrides(mut self, overrides: Vec<String>) -> Self {
        self.user_overrides = overrides.into_iter().collect();
        self
    }

    /// Returns `Some(reason)` if the package must be excluded, else `None`.
    pub fn reason_for(&self, pkg: &PackageName) -> Option<&'static str> {
        if self.user_overrides.contains(&pkg.0) {
            return Some("user-excluded");
        }
        let s = pkg.0.as_str();
        if COMMUNICATION_APPS.contains(&s) {
            return Some("communication app");
        }
        if INPUT_METHODS.contains(&s) {
            return Some("input method (keyboard)");
        }
        if SYSTEM_CRITICAL.contains(&s) {
            return Some("system-critical");
        }
        if ACCESSIBILITY.contains(&s) {
            return Some("accessibility service");
        }
        None
    }

    pub fn is_user_excluded(&self, pkg: &PackageName) -> bool {
        self.user_overrides.contains(&pkg.0)
    }
}
