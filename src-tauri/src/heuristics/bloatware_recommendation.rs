//! Plain-language recommendations and universal presets for the Bloatware
//! page. The risk classifier in `heuristics::risk` produces Critical /
//! Elevated / Moderate tiers based on uid + install path. That answers
//! "can I disable this without bricking the device". This module answers
//! "*should* I disable this, and what happens if I do".
//!
//! Designed to work on any Android device (stock Google, Samsung One UI,
//! Xiaomi MIUI/HyperOS, OnePlus OxygenOS, Motorola, Sony, Asus, etc).
//! Categorisation is driven by package-name prefixes that are stable
//! across OEM ROMs, not by the install path (which differs between OEMs).

use serde::{Deserialize, Serialize};

use crate::heuristics::risk::{PackageVerdict, RiskTier};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    SafeToDisable,
    PreinstalledBloat,
    SystemUseWithCare,
    DoNotTouch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloatwareRecommendation {
    pub package: String,
    pub tier: RiskTier,
    pub recommendation: Recommendation,
    pub notes: String,
    pub category: Option<BloatCategory>,
    /// True when the recommendation was corroborated by the UAD-NG community
    /// database (an explicit human review), not just our prefix heuristic.
    #[serde(default)]
    pub community_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BloatCategory {
    GoogleOptionalApps,
    GoogleAssistant,
    GoogleAds,
    SamsungBixby,
    SamsungOptionalApps,
    SamsungAds,
    XiaomiAds,
    XiaomiOptionalApps,
    OnePlusOptionalApps,
    HuaweiOptionalApps,
    OppoVivoOptionalApps,
    MotorolaOptionalApps,
    CarrierApps,
    PreloadedSocial,
    PreloadedMicrosoft,
    PreloadedNetflix,
}

pub fn recommend(verdict: &PackageVerdict) -> BloatwareRecommendation {
    // Safety invariant: a package our classifier flags Critical (core OS /
    // system uid) is NEVER overridden, not even by a community "Recommended".
    if verdict.tier == RiskTier::Critical {
        return BloatwareRecommendation {
            package: verdict.package.clone(),
            tier: verdict.tier,
            recommendation: Recommendation::DoNotTouch,
            notes: "Core OS or system-uid package. Disabling will break the device.".to_string(),
            category: None,
            community_verified: false,
        };
    }

    let (category, mut recommendation, base_notes) = classify_by_name(&verdict.package, verdict.tier);
    let mut notes = base_notes.to_string();
    let mut community_verified = false;

    // Cross-reference the UAD-NG community database. When present, the human-
    // reviewed removal rating is authoritative and refines our heuristic.
    if let Some(entry) = crate::heuristics::uad_list::lookup(&verdict.package) {
        use crate::heuristics::uad_list::UadRemoval;
        community_verified = true;
        recommendation = match entry.removal {
            UadRemoval::Recommended => Recommendation::PreinstalledBloat,
            UadRemoval::Advanced | UadRemoval::Expert => Recommendation::SystemUseWithCare,
            UadRemoval::Unsafe => Recommendation::DoNotTouch,
        };
        if !entry.description.is_empty() {
            notes = format!("{} (UAD-NG: {:?})", entry.description, entry.removal);
        }
    }

    BloatwareRecommendation {
        package: verdict.package.clone(),
        tier: verdict.tier,
        recommendation,
        notes,
        category,
        community_verified,
    }
}

fn classify_by_name(
    pkg: &str,
    tier: RiskTier,
) -> (Option<BloatCategory>, Recommendation, &'static str) {
    let lower = pkg.to_ascii_lowercase();

    // Google / GMS - never touch core services.
    if matches_any(&lower, &[
        "com.google.android.gsf", "com.google.android.gms",
        "com.android.vending", "com.google.android.gsf.login",
    ]) {
        return (None, Recommendation::DoNotTouch,
            "Google Play Services / Play Store. Required for push notifications, login, and most apps.");
    }
    if lower == "com.google.android.googlequicksearchbox"
        || lower == "com.google.android.apps.googleassistant"
        || lower == "com.google.intelligence.sense"
    {
        return (Some(BloatCategory::GoogleAssistant), Recommendation::PreinstalledBloat,
            "Google Assistant and 'Hey Google' detection. Disable if you don't use voice search; saves background CPU.");
    }
    if lower.starts_with("com.google.android.adservices")
        || lower == "com.google.android.partnersetup"
        || lower == "com.google.android.feedback"
    {
        return (Some(BloatCategory::GoogleAds), Recommendation::PreinstalledBloat,
            "Google ads / partner setup. Safe to disable; you'll keep seeing ads served by app developers, just not Google's measurement.");
    }
    if matches_any(&lower, &[
        "com.google.android.apps.photos", "com.google.android.apps.docs",
        "com.google.android.youtube", "com.google.android.apps.youtube.music",
        "com.google.android.apps.maps", "com.google.android.apps.tachyon",
        "com.google.android.apps.messaging", "com.google.android.gm",
        "com.google.android.calendar", "com.google.android.keep",
        "com.google.android.apps.subscriptions.red", "com.google.android.apps.wellbeing",
        "com.google.android.videos", "com.google.android.apps.books",
        "com.google.android.apps.podcasts", "com.google.android.apps.fitness",
        "com.google.android.apps.turbo", "com.google.android.contacts",
        "com.google.android.apps.nbu.files",
    ]) {
        return (Some(BloatCategory::GoogleOptionalApps), Recommendation::PreinstalledBloat,
            "Google optional app. Disabling stops it from running and frees background resources; you can still install it from the Play Store later.");
    }

    // Samsung One UI.
    if lower.contains(".bixby") || lower.contains("samsung.android.bixby") || lower == "com.samsung.android.app.spage" {
        return (Some(BloatCategory::SamsungBixby), Recommendation::PreinstalledBloat,
            "Samsung Bixby (voice + home page). Disable if you don't use it; reduces background services and frees the side-key.");
    }
    if lower == "com.samsung.android.mobileservice"
        || lower == "com.samsung.android.app.aodservice"
        || lower == "com.samsung.android.pushservice"
        || lower == "com.samsung.android.scloud"
        || lower == "com.samsung.android.smartsuggestions"
    {
        return (Some(BloatCategory::SamsungAds), Recommendation::PreinstalledBloat,
            "Samsung push / Smart Things / promotional services. Safe to disable; Samsung account sync stops.");
    }
    if lower.starts_with("com.samsung.android.app.")
        || lower.starts_with("com.samsung.android.video")
        || lower.starts_with("com.samsung.android.game")
        || lower.starts_with("com.samsung.android.themecenter")
        || lower == "com.sec.android.app.samsungapps"
        || lower == "com.sec.android.app.shealth"
        || lower.starts_with("com.samsung.android.spay")
    {
        return (Some(BloatCategory::SamsungOptionalApps), Recommendation::PreinstalledBloat,
            "Samsung optional app. Disable if unused; rest of One UI is unaffected.");
    }

    // Xiaomi MIUI / HyperOS.
    if matches_any(&lower, &[
        "com.miui.msa.global", "com.miui.systemadsolution",
        "com.miui.analytics", "com.miui.daemon", "com.xiaomi.glgm",
    ]) {
        return (Some(BloatCategory::XiaomiAds), Recommendation::PreinstalledBloat,
            "Xiaomi MSA ads / analytics. Highly recommended to disable; this is the main source of ads in MIUI/HyperOS system menus.");
    }
    if lower.starts_with("com.miui.")
        || lower.starts_with("com.xiaomi.market")
        || lower.starts_with("com.mi.android.globalpersonalassistant")
    {
        return (Some(BloatCategory::XiaomiOptionalApps), Recommendation::PreinstalledBloat,
            "Xiaomi MIUI optional component. Disabling may remove a wallpaper picker, weather widget, or store; the OS keeps working.");
    }

    // OnePlus / Oppo / Vivo / Realme.
    if lower.starts_with("com.oneplus.") || lower.starts_with("net.oneplus.") {
        return (Some(BloatCategory::OnePlusOptionalApps), Recommendation::PreinstalledBloat,
            "OnePlus optional app. Disable if unused; OxygenOS core stays intact.");
    }
    if lower.starts_with("com.coloros.") || lower.starts_with("com.heytap.")
        || lower.starts_with("com.oppo.") || lower.starts_with("com.realme.")
        || lower.starts_with("com.vivo.") || lower.starts_with("com.bbk.")
    {
        return (Some(BloatCategory::OppoVivoOptionalApps), Recommendation::PreinstalledBloat,
            "OPPO / Realme / Vivo optional app. Safe to disable individually; the launcher and dialer remain.");
    }

    // Huawei / Honor.
    if lower.starts_with("com.huawei.") || lower.starts_with("com.hihonor.") {
        return (Some(BloatCategory::HuaweiOptionalApps), Recommendation::PreinstalledBloat,
            "Huawei / Honor optional app. Safe to disable; HMS Core (if present) stays untouched.");
    }

    // Motorola / Sony / Asus / Lenovo.
    if lower.starts_with("com.motorola.") || lower.starts_with("com.lenovo.")
        || lower.starts_with("com.sonyericsson.") || lower.starts_with("com.sony.")
        || lower.starts_with("com.asus.")
    {
        return (Some(BloatCategory::MotorolaOptionalApps), Recommendation::PreinstalledBloat,
            "OEM optional app (Motorola / Sony / Asus / Lenovo). Safe to disable individually.");
    }

    // Carrier apps - extra care because of VoLTE / Wi-Fi calling.
    if matches_any(&lower, &[
        "com.vzw.", "com.att.", "com.tmobile.", "com.sprint.",
        "com.vodafone.", "com.movistar.", "com.orange.", "com.telefonica.",
        "com.verizon.", "com.uscc.", "com.rogers.", "com.bell.",
        "com.telstra.", "com.docomo.", "com.softbank.", "com.kt.",
    ]) || lower.contains("carrier") || lower.contains("operator")
    {
        return (Some(BloatCategory::CarrierApps), Recommendation::SystemUseWithCare,
            "Carrier app. Disabling can break Wi-Fi calling, VoLTE, visual voicemail, or carrier billing. Check before disabling if you depend on these features.");
    }

    // Pre-loaded social / partner apps.
    if matches_any(&lower, &[
        "com.facebook.katana", "com.facebook.system", "com.facebook.appmanager",
        "com.facebook.services", "com.instagram.android", "com.zhiliaoapp.musically",
        "com.ss.android.ugc.trill", "com.twitter.android", "com.snapchat.android",
    ]) {
        return (Some(BloatCategory::PreloadedSocial), Recommendation::SafeToDisable,
            "Pre-loaded social app. Safe to disable; reinstall from Play Store if you ever want it back.");
    }
    if matches_any(&lower, &[
        "com.microsoft.skydrive", "com.linkedin.android",
        "com.microsoft.office.officehubrow", "com.microsoft.office.outlook",
        "com.microsoft.teams", "com.microsoft.bing",
    ]) {
        return (Some(BloatCategory::PreloadedMicrosoft), Recommendation::SafeToDisable,
            "Pre-loaded Microsoft app. Safe to disable; available on the Play Store.");
    }
    if matches_any(&lower, &["com.netflix.mediaclient", "com.netflix.partner.activation", "com.spotify.music"]) {
        return (Some(BloatCategory::PreloadedNetflix), Recommendation::SafeToDisable,
            "Pre-loaded streaming app. Safe to disable; reinstall from Play Store if needed.");
    }

    // Fallback by tier.
    match tier {
        RiskTier::Elevated => (None, Recommendation::SystemUseWithCare,
            "OEM-privileged path. Disabling may break a feature you didn't know depended on it. Test after disabling."),
        RiskTier::Moderate => (None, Recommendation::SafeToDisable,
            "User-installed app. Safe to disable; you can re-enable any time from this page."),
        RiskTier::Critical => unreachable!("handled at top of recommend()"),
    }
}

fn matches_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| {
        if n.ends_with('.') {
            haystack.starts_with(n)
        } else {
            haystack == *n || haystack.starts_with(&format!("{n}."))
        }
    })
}

// Preset engine -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BloatPreset {
    DebloatGoogle,
    DebloatOem,
    DebloatAdsAndTrackers,
    DebloatPartnerApps,
    DebloatCarrier,
}

impl BloatPreset {
    pub fn categories(self) -> &'static [BloatCategory] {
        use BloatCategory::*;
        match self {
            Self::DebloatGoogle => &[GoogleOptionalApps, GoogleAssistant],
            Self::DebloatOem => &[
                SamsungOptionalApps, SamsungBixby,
                XiaomiOptionalApps,
                OnePlusOptionalApps,
                OppoVivoOptionalApps,
                HuaweiOptionalApps,
                MotorolaOptionalApps,
            ],
            Self::DebloatAdsAndTrackers => &[GoogleAds, SamsungAds, XiaomiAds],
            Self::DebloatPartnerApps => &[PreloadedSocial, PreloadedMicrosoft, PreloadedNetflix],
            Self::DebloatCarrier => &[CarrierApps],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DebloatGoogle => "Debloat Google",
            Self::DebloatOem => "Debloat OEM bloat",
            Self::DebloatAdsAndTrackers => "Disable ads & trackers",
            Self::DebloatPartnerApps => "Disable preloaded partner apps",
            Self::DebloatCarrier => "Disable carrier apps",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::DebloatGoogle => "Disables Google's optional pre-installed apps (Photos, Drive, YouTube, Maps, Assistant). Play Store, Play Services and account sync stay intact.",
            Self::DebloatOem => "Disables OEM-specific extras for any matching brand on this device: Samsung One UI extras, Xiaomi MIUI extras, OnePlus / Oppo / Vivo / Huawei / Motorola / Sony / Asus optional apps.",
            Self::DebloatAdsAndTrackers => "Disables known ad and tracking endpoints across vendors: Google Ads services, Xiaomi MSA, Samsung promotional pushers.",
            Self::DebloatPartnerApps => "Disables pre-loaded third-party apps the OEM ships for distribution deals: Facebook, Microsoft suite, LinkedIn, Netflix, Spotify, TikTok.",
            Self::DebloatCarrier => "Disables carrier-injected apps. WARNING: depending on your carrier and country, this can break Wi-Fi calling, VoLTE, or visual voicemail.",
        }
    }

    pub fn all() -> &'static [BloatPreset] {
        &[
            Self::DebloatGoogle,
            Self::DebloatOem,
            Self::DebloatAdsAndTrackers,
            Self::DebloatPartnerApps,
            Self::DebloatCarrier,
        ]
    }
}

pub fn packages_for_preset(
    preset: BloatPreset,
    recommendations: &[BloatwareRecommendation],
) -> Vec<String> {
    let cats = preset.categories();
    recommendations
        .iter()
        .filter(|r| r.tier != RiskTier::Critical)
        .filter(|r| r.category.map(|c| cats.contains(&c)).unwrap_or(false))
        .map(|r| r.package.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(name: &str, tier: RiskTier) -> PackageVerdict {
        PackageVerdict {
            package: name.into(),
            uid: 10100,
            install_path: String::new(),
            tier,
            reasons: vec![],
        }
    }

    #[test]
    fn critical_always_do_not_touch() {
        let r = recommend(&verdict("com.android.systemui", RiskTier::Critical));
        assert_eq!(r.recommendation, Recommendation::DoNotTouch);
        assert!(r.category.is_none());
    }

    #[test]
    fn detects_google_optional() {
        let r = recommend(&verdict("com.google.android.apps.photos", RiskTier::Elevated));
        assert_eq!(r.recommendation, Recommendation::PreinstalledBloat);
        assert_eq!(r.category, Some(BloatCategory::GoogleOptionalApps));
    }

    #[test]
    fn detects_samsung_bixby() {
        let r = recommend(&verdict("com.samsung.android.bixby.agent", RiskTier::Elevated));
        assert_eq!(r.category, Some(BloatCategory::SamsungBixby));
    }

    #[test]
    fn detects_xiaomi_ads() {
        let r = recommend(&verdict("com.miui.msa.global", RiskTier::Elevated));
        assert_eq!(r.category, Some(BloatCategory::XiaomiAds));
    }

    #[test]
    fn detects_facebook_preload() {
        let r = recommend(&verdict("com.facebook.katana", RiskTier::Moderate));
        // Category still comes from our prefix classifier.
        assert_eq!(r.category, Some(BloatCategory::PreloadedSocial));
        // Facebook is in the UAD-NG list (removal=Recommended), so the community
        // rating refines our verdict and marks it verified.
        assert!(r.community_verified);
        assert_eq!(r.recommendation, Recommendation::PreinstalledBloat);
    }

    #[test]
    fn uad_unsafe_downgrades_to_do_not_touch() {
        // GMS is Moderate/Elevated by uid heuristic in this synthetic verdict,
        // but UAD marks it Unsafe → must become DoNotTouch and verified.
        let r = recommend(&verdict("com.google.android.gms", RiskTier::Elevated));
        assert!(r.community_verified);
        assert_eq!(r.recommendation, Recommendation::DoNotTouch);
    }

    #[test]
    fn non_uad_package_is_not_verified() {
        let r = recommend(&verdict("com.random.indiegame", RiskTier::Moderate));
        assert!(!r.community_verified);
    }

    #[test]
    fn unknown_user_app_is_safe() {
        let r = recommend(&verdict("com.random.indiegame", RiskTier::Moderate));
        assert_eq!(r.recommendation, Recommendation::SafeToDisable);
        assert!(r.category.is_none());
    }

    #[test]
    fn unknown_oem_priv_is_use_with_care() {
        let r = recommend(&verdict("com.weirdoem.priv", RiskTier::Elevated));
        assert_eq!(r.recommendation, Recommendation::SystemUseWithCare);
    }

    #[test]
    fn preset_filters_categories() {
        let recs = vec![
            recommend(&verdict("com.google.android.apps.photos", RiskTier::Elevated)),
            recommend(&verdict("com.samsung.android.bixby.agent", RiskTier::Elevated)),
            recommend(&verdict("com.random.app", RiskTier::Moderate)),
        ];
        let google = packages_for_preset(BloatPreset::DebloatGoogle, &recs);
        assert_eq!(google.len(), 1);
        assert_eq!(google[0], "com.google.android.apps.photos");
    }

    #[test]
    fn preset_never_includes_critical() {
        let mut v = verdict("com.android.systemui", RiskTier::Critical);
        v.tier = RiskTier::Critical;
        let recs = vec![recommend(&v)];
        for preset in BloatPreset::all() {
            let pkgs = packages_for_preset(*preset, &recs);
            assert!(pkgs.is_empty(), "preset {preset:?} leaked a critical pkg");
        }
    }
}
