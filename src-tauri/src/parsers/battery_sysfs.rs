//! Battery health from kernel sysfs with cascading fallbacks.
//!
//! Different vendors expose battery data under different paths:
//!   - AOSP/Pixel: /sys/class/power_supply/battery/
//!   - Qualcomm:   /sys/class/power_supply/bms/ (cycle_count specifically)
//!   - Samsung:    /sys/class/power_supply/battery/ (mostly compatible)
//!
//! We probe in order and fall back to `dumpsys battery` (less accurate but
//! universally available).

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryHealth {
    /// Manufacturing cycles. `None` if not exposed by the kernel.
    pub cycle_count: Option<u32>,
    /// Current full-charge capacity in µAh (microampere-hours).
    pub charge_full_uah: Option<u64>,
    /// Original design capacity in µAh.
    pub charge_full_design_uah: Option<u64>,
    /// Computed health %: `charge_full / charge_full_design * 100`. `None` if missing pieces.
    pub health_percent: Option<f32>,
    /// Live capacity 0-100.
    pub level_percent: Option<u8>,
    /// Temperature in degrees Celsius.
    pub temperature_c: Option<f32>,
    /// Voltage in volts.
    pub voltage_v: Option<f32>,
    /// "Charging" | "Discharging" | "Full" | "Not charging".
    pub status: Option<String>,
    /// "Good" | "Overheat" | "Dead" | "Over voltage" | "Unspecified failure" | "Cold".
    pub health_status: Option<String>,
    /// Source path used to read cycle_count.
    pub source: Option<String>,
}

/// One-shot multi-attribute reader. Returns a single shell command that
/// echoes `key=value` pairs for all attributes we want.
///
/// Robustness: vendors split battery attributes across DIFFERENT nodes
/// (e.g. Qualcomm keeps `cycle_count` under `bms` while `capacity` lives under
/// `battery`). So instead of picking one node and stopping, we read each field
/// from whichever candidate node exposes it first. We also accept common field
/// name aliases (`batt_temp`, `temp_now`, `charge_counter`).
const READ_SCRIPT: &str = r#"psbase=/sys/class/power_supply
read_field() {
  for d in battery bms main-battery qcom-battery max-battery battery-main mtk-battery; do
    for name in "$@"; do
      v=$(cat "$psbase/$d/$name" 2>/dev/null)
      if [ -n "$v" ]; then echo "$1=$v"; return; fi
    done
  done
}
echo "_base=$psbase"
# First arg is the canonical key we emit; the rest are vendor aliases:
#   Samsung: cycle_count -> battery_cycle ; MediaTek: temp -> batt_temp
#   Qualcomm/BMS variants use charge_full_design / charge_full under bms.
read_field cycle_count battery_cycle batt_cycle
read_field charge_full charge_full_uah
read_field charge_full_design charge_full_design_uah
read_field capacity
read_field temp batt_temp temp_now battery_temp
read_field voltage_now voltage_batt batt_vol
read_field status
read_field health batt_health"#;

impl BatteryHealth {
    /// Overlay sysfs-derived fields onto a `dumpsys battery` baseline. sysfs is
    /// preferred for the health metrics it uniquely exposes (cycle count, full /
    /// design charge, health %); for fields both sources provide we only fill
    /// gaps so we never regress a value dumpsys already gave us. This is what
    /// makes the battery card work across vendors: worst case you still get the
    /// universal dumpsys data, best case sysfs enriches it.
    pub fn merge_from_sysfs(&mut self, s: BatteryHealth) {
        if s.cycle_count.is_some() { self.cycle_count = s.cycle_count; }
        if s.charge_full_uah.is_some() { self.charge_full_uah = s.charge_full_uah; }
        if s.charge_full_design_uah.is_some() { self.charge_full_design_uah = s.charge_full_design_uah; }
        if s.health_percent.is_some() { self.health_percent = s.health_percent; }
        if self.level_percent.is_none() { self.level_percent = s.level_percent; }
        if self.temperature_c.is_none() { self.temperature_c = s.temperature_c; }
        if self.voltage_v.is_none() { self.voltage_v = s.voltage_v; }
        if self.status.is_none() { self.status = s.status; }
        if self.health_status.is_none() { self.health_status = s.health_status; }
        // Reflect the richest source actually used for health metrics.
        if s.cycle_count.is_some() || s.charge_full_uah.is_some() {
            self.source = s.source;
        }
    }
}

pub struct BatterySysfsParser;

impl BatterySysfsParser {
    /// Builds the single ADB shell command to fetch all sysfs values at once.
    pub fn read_script() -> &'static str {
        READ_SCRIPT
    }

    /// Parses the output of the read script into a `BatteryHealth`.
    pub fn parse(input: &str) -> Result<BatteryHealth> {
        let mut h = BatteryHealth::default();
        for line in input.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key {
                "_base" => h.source = Some(value.to_string()),
                "cycle_count" => h.cycle_count = value.parse().ok(),
                "charge_full" => h.charge_full_uah = value.parse().ok(),
                "charge_full_design" => h.charge_full_design_uah = value.parse().ok(),
                "capacity" => h.level_percent = value.parse::<u8>().ok(),
                "temp" => {
                    // Most kernels report temp in 0.1°C units (e.g. 325 = 32.5°C).
                    if let Ok(raw) = value.parse::<i32>() {
                        h.temperature_c = Some(raw as f32 / 10.0);
                    }
                }
                "voltage_now" => {
                    // Usually microvolts, but some vendor aliases report mV.
                    // Normalise by magnitude so both work.
                    if let Ok(raw) = value.parse::<u64>() {
                        h.voltage_v = Some(if raw > 100_000 {
                            raw as f32 / 1_000_000.0
                        } else {
                            raw as f32 / 1000.0
                        });
                    }
                }
                "status" => h.status = Some(value.to_string()),
                "health" => h.health_status = Some(value.to_string()),
                _ => {}
            }
        }

        if let (Some(full), Some(design)) = (h.charge_full_uah, h.charge_full_design_uah) {
            if design > 0 {
                h.health_percent = Some((full as f32 / design as f32) * 100.0);
            }
        }

        Ok(h)
    }

    /// Parses `dumpsys battery` as the fallback path. Used when sysfs is
    /// inaccessible (rare but happens on heavily customized ROMs).
    pub fn parse_dumpsys(input: &str) -> BatteryHealth {
        let mut h = BatteryHealth::default();
        for line in input.lines() {
            // Tolerant key/value split: handles varying indentation and the
            // capitalisation differences between ROMs ("Cycle count" vs
            // "cycle count", "Charge counter", etc.).
            let Some((key, value)) = line.split_once(':') else { continue };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key.as_str() {
                "level" => h.level_percent = value.parse().ok(),
                "temperature" => {
                    if let Ok(raw) = value.parse::<i32>() {
                        h.temperature_c = Some(raw as f32 / 10.0);
                    }
                }
                "voltage" => {
                    if let Ok(raw) = value.parse::<u64>() {
                        // dumpsys reports mV; some ROMs report µV (7-digit) — normalise.
                        h.voltage_v = Some(if raw > 100_000 {
                            raw as f32 / 1_000_000.0
                        } else {
                            raw as f32 / 1000.0
                        });
                    }
                }
                "status" => h.status = Some(decode_dumpsys_status(value)),
                "health" => h.health_status = Some(decode_dumpsys_health(value)),
                "cycle count" => h.cycle_count = value.parse().ok(),
                _ => {}
            }
        }
        h.source = Some("dumpsys battery".to_string());
        h
    }
}

impl BatterySysfsParser {
    /// Extract battery capacity estimates from `dumpsys batterystats`. These are
    /// readable WITHOUT root on most devices (including ones that lock the sysfs
    /// charge_full_design node, like Nothing/Qualcomm), so they're our health
    /// fallback. Returns `(estimated_mah, last_learned_mah)`:
    ///   Estimated battery capacity: 4716 mAh   ← design/nominal (power_profile)
    ///   Last learned battery capacity: 4698 mAh ← current learned full capacity
    pub fn parse_batterystats_capacity(input: &str) -> (Option<f64>, Option<f64>) {
        let mut estimated = None;
        let mut learned = None;
        for line in input.lines() {
            let l = line.trim();
            let grab = |s: &str| -> Option<f64> {
                s.trim()
                    .split_whitespace()
                    .next()
                    .and_then(|t| t.parse::<f64>().ok())
            };
            if let Some(v) = l.strip_prefix("Estimated battery capacity:") {
                estimated = grab(v);
            } else if let Some(v) = l.strip_prefix("Last learned battery capacity:") {
                learned = grab(v);
            }
        }
        (estimated, learned)
    }
}

fn decode_dumpsys_health(raw: &str) -> String {
    match raw {
        "1" => "Unknown".into(),
        "2" => "Good".into(),
        "3" => "Overheat".into(),
        "4" => "Dead".into(),
        "5" => "Over voltage".into(),
        "6" => "Unspecified failure".into(),
        "7" => "Cold".into(),
        other => other.into(),
    }
}

fn decode_dumpsys_status(raw: &str) -> String {
    match raw {
        "1" => "Unknown".into(),
        "2" => "Charging".into(),
        "3" => "Discharging".into(),
        "4" => "Not charging".into(),
        "5" => "Full".into(),
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pixel_like_output() {
        let input = "\
_base=/sys/class/power_supply/battery
cycle_count=128
charge_full=4250000
charge_full_design=5000000
capacity=87
temp=312
voltage_now=4012000
status=Discharging
health=Good
";
        let h = BatterySysfsParser::parse(input).unwrap();
        assert_eq!(h.cycle_count, Some(128));
        assert_eq!(h.charge_full_uah, Some(4_250_000));
        assert_eq!(h.charge_full_design_uah, Some(5_000_000));
        assert_eq!(h.level_percent, Some(87));
        assert!((h.temperature_c.unwrap() - 31.2).abs() < 0.001);
        assert!((h.voltage_v.unwrap() - 4.012).abs() < 0.001);
        assert!((h.health_percent.unwrap() - 85.0).abs() < 0.001);
    }

    #[test]
    fn handles_missing_cycle_count() {
        let input = "_base=/sys/class/power_supply/battery\ncapacity=50\n";
        let h = BatterySysfsParser::parse(input).unwrap();
        assert_eq!(h.cycle_count, None);
        assert_eq!(h.level_percent, Some(50));
    }

    #[test]
    fn sysfs_voltage_normalises_uv_and_mv() {
        let uv = BatterySysfsParser::parse("_base=x\nvoltage_now=4012000\n").unwrap();
        assert!((uv.voltage_v.unwrap() - 4.012).abs() < 0.001);
        // Vendor alias reporting millivolts (e.g. some Samsung nodes).
        let mv = BatterySysfsParser::parse("_base=x\nvoltage_now=4012\n").unwrap();
        assert!((mv.voltage_v.unwrap() - 4.012).abs() < 0.001);
    }

    #[test]
    fn dumpsys_baseline_parses_core_fields() {
        // Typical `dumpsys battery` (e.g. Nothing OS) — the universal fallback
        // that must always populate level / temp / status / health.
        let input = "\
Current Battery Service state:
  AC powered: false
  status: 3
  health: 2
  present: true
  level: 63
  scale: 100
  voltage: 4051
  temperature: 298
  technology: Li-ion
";
        let h = BatterySysfsParser::parse_dumpsys(input);
        assert_eq!(h.level_percent, Some(63));
        assert_eq!(h.status.as_deref(), Some("Discharging"));
        assert_eq!(h.health_status.as_deref(), Some("Good"));
        assert!((h.temperature_c.unwrap() - 29.8).abs() < 0.001);
        assert!((h.voltage_v.unwrap() - 4.051).abs() < 0.001);
    }

    #[test]
    fn batterystats_capacity_fallback_parses() {
        // Real header lines from `dumpsys batterystats` (readable without root).
        let input = "\
  Estimated battery capacity: 4716 mAh
  Last learned battery capacity: 4698 mAh
  Min learned battery capacity: 4692 mAh
  Max learned battery capacity: 4698 mAh
";
        let (est, learned) = BatterySysfsParser::parse_batterystats_capacity(input);
        assert_eq!(est, Some(4716.0));
        assert_eq!(learned, Some(4698.0));
        // Health from these ≈ 99.6%.
        let pct = learned.unwrap() / est.unwrap() * 100.0;
        assert!((pct - 99.6).abs() < 0.2, "got {pct}");
    }

    #[test]
    fn batterystats_capacity_absent_is_none() {
        let (est, learned) = BatterySysfsParser::parse_batterystats_capacity("no capacity here\n");
        assert!(est.is_none() && learned.is_none());
    }

    #[test]
    fn sysfs_merge_enriches_dumpsys_baseline() {
        // Simulate the real flow: dumpsys gives level/status, sysfs adds cycles
        // and design charge → health %. Nothing regresses.
        let mut base = BatterySysfsParser::parse_dumpsys(
            "level: 80\nstatus: 3\ntemperature: 300\nvoltage: 4100\n",
        );
        let sys = BatterySysfsParser::parse(
            "_base=/sys/class/power_supply\ncycle_count=210\ncharge_full=4200000\ncharge_full_design=5000000\n",
        )
        .unwrap();
        base.merge_from_sysfs(sys);
        assert_eq!(base.level_percent, Some(80)); // kept from dumpsys
        assert_eq!(base.cycle_count, Some(210)); // added from sysfs
        assert!((base.health_percent.unwrap() - 84.0).abs() < 0.001);
    }
}
