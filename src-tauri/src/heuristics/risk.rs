//! Risk classifier.

use serde::{Deserialize, Serialize};

use crate::parsers::InstalledPackage;

use super::manifest::HybridManifest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskTier {
    Critical,
    Elevated,
    Moderate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVerdict {
    pub package: String,
    pub uid: i32,
    pub install_path: String,
    pub tier: RiskTier,
    pub reasons: Vec<String>,
}

pub fn classify(pkg: &InstalledPackage, manifest: &HybridManifest) -> PackageVerdict {
    let mut reasons: Vec<String> = Vec::new();
    let mut tier = if pkg.uid >= 0 && pkg.uid < 10_000 {
        reasons.push(format!("uid {} is below 10000 (core OS)", pkg.uid));
        RiskTier::Critical
    } else if is_system_path(&pkg.install_path) {
        reasons.push(format!("installed under {}", root_of_path(&pkg.install_path)));
        RiskTier::Critical
    } else if is_oem_priv(&pkg.install_path) {
        reasons.push("OEM privileged app path".into());
        RiskTier::Elevated
    } else {
        reasons.push("user-installed app".into());
        RiskTier::Moderate
    };

    if let Some(rule) = manifest.lookup(&pkg.name) {
        if rule.never_touch && tier != RiskTier::Critical {
            tier = RiskTier::Critical;
            reasons.push(format!("manifest: {}", rule.reason));
        } else if rule.known_offender && tier == RiskTier::Elevated {
            tier = RiskTier::Moderate;
            reasons.push(format!("manifest: {}", rule.reason));
        }
    }

    PackageVerdict {
        package: pkg.name.0.clone(),
        uid: pkg.uid,
        install_path: pkg.install_path.clone(),
        tier,
        reasons,
    }
}

fn is_system_path(p: &str) -> bool {
    p.starts_with("/system/")
        || p.starts_with("/vendor/")
        || p.starts_with("/apex/")
        || p.starts_with("/product/")
}

fn is_oem_priv(p: &str) -> bool {
    p.starts_with("/system_ext/") || p.starts_with("/product/priv-app/") || p.contains("/priv-app/")
}

fn root_of_path(p: &str) -> &str {
    if let Some(rest) = p.strip_prefix('/') {
        if let Some(idx) = rest.find('/') {
            return &p[..idx + 1];
        }
    }
    "/"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::{InstalledPackage, PackageName};

    fn fake_pkg(name: &str, uid: i32, path: &str) -> InstalledPackage {
        InstalledPackage {
            name: PackageName::from(name),
            uid,
            install_path: path.into(),
            is_system: path.starts_with("/system"),
            label: None,
        }
    }

    #[test]
    fn low_uid_is_critical() {
        let m = HybridManifest::empty();
        let v = classify(&fake_pkg("system", 1000, "/system/priv-app/Sys/x.apk"), &m);
        assert_eq!(v.tier, RiskTier::Critical);
    }

    #[test]
    fn user_app_is_moderate() {
        let m = HybridManifest::empty();
        let v = classify(&fake_pkg("com.x", 10086, "/data/app/com.x/base.apk"), &m);
        assert_eq!(v.tier, RiskTier::Moderate);
    }
}
