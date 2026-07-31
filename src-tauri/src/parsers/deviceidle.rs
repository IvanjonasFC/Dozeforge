//! `dumpsys deviceidle` parser.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::{PackageName, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DozeWhitelist {
    pub user_whitelisted: Vec<PackageName>,
    pub system_whitelisted: Vec<PackageName>,
}

static USER_BLOCK: Lazy<Regex> = Lazy::new(|| Regex::new(r"Whitelist user apps:").unwrap());
static SYSTEM_BLOCK: Lazy<Regex> = Lazy::new(|| Regex::new(r"Whitelist system apps:").unwrap());

static ENTRY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s+(?:system,)?(?P<pkg>[\w.]+)(?:,.*)?$").unwrap());

pub struct DeviceIdleParser;

impl Parser for DeviceIdleParser {
    type Output = DozeWhitelist;

    fn parse(&self, input: &str) -> Result<DozeWhitelist> {
        let mut user = Vec::new();
        let mut sys = Vec::new();
        let mut mode = Mode::None;

        for line in input.lines() {
            if USER_BLOCK.is_match(line) { mode = Mode::User; continue; }
            if SYSTEM_BLOCK.is_match(line) { mode = Mode::System; continue; }
            if !line.starts_with(' ') && !line.starts_with('\t') {
                mode = Mode::None;
            }
            if let Some(caps) = ENTRY.captures(line) {
                let pkg = PackageName::from(&caps["pkg"]);
                match mode {
                    Mode::User => user.push(pkg),
                    Mode::System => sys.push(pkg),
                    Mode::None => {}
                }
            }
        }

        Ok(DozeWhitelist { user_whitelisted: user, system_whitelisted: sys })
    }
}

enum Mode { None, User, System }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DozeState {
    pub state: String,
    pub deep_enabled: bool,
    pub force_idle: bool,
    pub screen_on: bool,
    pub charging: bool,
    pub next_alarm_elapsed: Option<String>,
}

pub struct DozeStateParser;

impl Parser for DozeStateParser {
    type Output = DozeState;

    fn parse(&self, input: &str) -> Result<DozeState> {
        let mut state = DozeState::default();
        for line in input.lines() {
            let t = line.trim();
            if let Some(v) = t.strip_prefix("mState=") {
                // The value can carry trailing detail, e.g. `mState=ACTIVE (nfc)`
                // or `mState=IDLE mLightState=...`. Keep only the state token so
                // it matches the UI's expected enum names (ACTIVE, IDLE, …).
                state.state = v.split_whitespace().next().unwrap_or("").to_string();
            } else if let Some(v) = t.strip_prefix("mDeepEnabled=") {
                state.deep_enabled = v == "true";
            } else if let Some(v) = t.strip_prefix("mForceIdle=") {
                state.force_idle = v == "true";
            } else if let Some(v) = t.strip_prefix("mScreenOn=") {
                state.screen_on = v == "true";
            } else if let Some(v) = t.strip_prefix("mCharging=") {
                state.charging = v == "true";
            } else if let Some(v) = t.strip_prefix("mNextAlarmElapsed=") {
                state.next_alarm_elapsed = Some(v.to_string());
            }
        }
        Ok(state)
    }
}
