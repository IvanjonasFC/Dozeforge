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
const READ_SCRIPT: &str = r#"for d in battery bms main-battery; do
  base=/sys/class/power_supply/$d
  [ -d "$base" ] || continue
  echo "_base=$base"
  for f in cycle_count charge_full charge_full_design capacity temp voltage_now status health; do
    v=$(cat "$base/$f" 2>/dev/null)
    [ -n "$v" ] && echo "$f=$v"
  done
  break
done"#;

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
                    // Microvolts -> volts
                    if let Ok(raw) = value.parse::<u64>() {
                        h.voltage_v = Some(raw as f32 / 1_000_000.0);
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
            let line = line.trim();
            if let Some(v) = line.strip_prefix("level: ") {
                h.level_percent = v.trim().parse().ok();
            } else if let Some(v) = line.strip_prefix("temperature: ") {
                if let Ok(raw) = v.trim().parse::<i32>() {
                    h.temperature_c = Some(raw as f32 / 10.0);
                }
            } else if let Some(v) = line.strip_prefix("voltage: ") {
                if let Ok(raw) = v.trim().parse::<u64>() {
                    h.voltage_v = Some(raw as f32 / 1000.0); // already in mV here
                }
            } else if let Some(v) = line.strip_prefix("status: ") {
                h.status = Some(decode_dumpsys_status(v.trim()));
            } else if let Some(v) = line.strip_prefix("cycle count: ") {
                // Android 14+ exposes this on some devices
                h.cycle_count = v.trim().parse().ok();
            }
        }
        h.source = Some("dumpsys battery".to_string());
        h
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
}
