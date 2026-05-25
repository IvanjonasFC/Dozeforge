//! Sleep Score — a single 0-100 number summarising how "well" a device can
//! sleep when its screen is off. Computed deterministically from observed
//! data, no ML, no online state.
//!
//! Penalties stack additively from a base of 100. The score is meant to be
//! visually intuitive: 90+ is excellent, 70-89 good, 40-69 mediocre, <40 bad.

use serde::{Deserialize, Serialize};

use crate::parsers::deviceidle::DozeWhitelist;
use crate::parsers::sensorservice::SensorClient;
use crate::parsers::{AlarmAttribution, WakelockEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepScore {
    pub score: u8,
    pub tier: SleepTier,
    pub penalties: Vec<Penalty>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SleepTier {
    Excellent,
    Good,
    Mediocre,
    Bad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Penalty {
    pub label: String,
    pub points: i32,
}

pub struct SleepScoreCalc<'a> {
    pub wakelocks: &'a [WakelockEntry],
    pub alarms: &'a [AlarmAttribution],
    pub doze: &'a DozeWhitelist,
    pub sensors: &'a [SensorClient],
}

impl<'a> SleepScoreCalc<'a> {
    pub fn compute(&self) -> SleepScore {
        let mut score: i32 = 100;
        let mut penalties = Vec::new();

        // Penalty 1: total wakelock time (uptime % owned by wakelocks).
        let total_ms: u64 = self.wakelocks.iter().map(|w| w.total_ms).sum();
        let hours = (total_ms as f64) / 3_600_000.0;
        let wl_penalty = if hours > 4.0 {
            -35
        } else if hours > 2.0 {
            -22
        } else if hours > 1.0 {
            -12
        } else if hours > 0.3 {
            -5
        } else {
            0
        };
        if wl_penalty < 0 {
            score += wl_penalty;
            penalties.push(Penalty {
                label: format!("{:.1}h of wakelock time", hours),
                points: wl_penalty,
            });
        }

        // Penalty 2: total wakeup count (alarms triggering the device).
        let total_wakeups: u32 = self.alarms.iter().map(|a| a.wake_count).sum();
        let alarm_penalty = match total_wakeups {
            0..=80 => 0,
            81..=250 => -8,
            251..=600 => -18,
            _ => -28,
        };
        if alarm_penalty < 0 {
            score += alarm_penalty;
            penalties.push(Penalty {
                label: format!("{} wakeups", total_wakeups),
                points: alarm_penalty,
            });
        }

        // Penalty 3: doze whitelist size (user-whitelisted apps bypass deep sleep).
        let whitelist_size = self.doze.user_whitelisted.len() as i32;
        if whitelist_size > 0 {
            // -3 per app, max -20
            let p = (whitelist_size * -3).max(-20);
            score += p;
            penalties.push(Penalty {
                label: format!("{} apps bypassing Doze", whitelist_size),
                points: p,
            });
        }

        // Penalty 4: long-lived sensor clients.
        let sensor_count = self.sensors.len() as i32;
        if sensor_count > 2 {
            let p = ((sensor_count - 2) * -2).max(-10);
            score += p;
            penalties.push(Penalty {
                label: format!("{} sensor clients active", sensor_count),
                points: p,
            });
        }

        let score = score.clamp(0, 100) as u8;
        let tier = match score {
            90..=100 => SleepTier::Excellent,
            70..=89 => SleepTier::Good,
            40..=69 => SleepTier::Mediocre,
            _ => SleepTier::Bad,
        };

        SleepScore { score, tier, penalties }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::deviceidle::DozeWhitelist;

    fn empty_doze() -> DozeWhitelist {
        DozeWhitelist { user_whitelisted: vec![], system_whitelisted: vec![] }
    }

    #[test]
    fn perfect_device_scores_100() {
        let calc = SleepScoreCalc {
            wakelocks: &[],
            alarms: &[],
            doze: &empty_doze(),
            sensors: &[],
        };
        let s = calc.compute();
        assert_eq!(s.score, 100);
        assert_eq!(s.tier, SleepTier::Excellent);
        assert!(s.penalties.is_empty());
    }
}
