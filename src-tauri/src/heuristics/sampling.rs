//! Continuous CPU sampler with p50 / p95 aggregation.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::debug;

use crate::adb::command::DEFAULT_TIMEOUT;
use crate::adb::{AdbClient, DeviceSerial};
use crate::error::Result;
use crate::parsers::cpuinfo::TopParser;
use crate::parsers::{PackageName, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuAggregate {
    pub package: Option<PackageName>,
    pub pid: u32,
    pub samples: u32,
    pub p50: f32,
    pub p95: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SamplingConfig {
    pub interval: Duration,
    pub total_samples: u32,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(2),
            total_samples: 15,
        }
    }
}

pub struct CpuSampler<'a> {
    pub client: &'a AdbClient,
    pub serial: &'a DeviceSerial,
    pub config: SamplingConfig,
}

impl<'a> CpuSampler<'a> {
    pub async fn run(&self) -> Result<Vec<CpuAggregate>> {
        let mut per_pid: HashMap<u32, ProcessAccumulator> = HashMap::new();
        let parser = TopParser;

        for n in 0..self.config.total_samples {
            debug!(target: "dozeforge::sampling", "tick {} of {}", n + 1, self.config.total_samples);
            let raw = self
                .client
                .invoker
                .shell(self.serial, "top -b -n 1 -o PID,S,%CPU,ARGS", DEFAULT_TIMEOUT)
                .await?;
            let samples = parser.parse(&raw)?;
            for s in samples {
                let acc = per_pid.entry(s.pid).or_insert(ProcessAccumulator {
                    pid: s.pid,
                    package: s.package.clone(),
                    values: Vec::with_capacity(self.config.total_samples as usize),
                });
                acc.values.push(s.cpu_percent);
                if acc.package.is_none() { acc.package = s.package; }
            }
            if n + 1 < self.config.total_samples {
                sleep(self.config.interval).await;
            }
        }

        let mut out: Vec<CpuAggregate> = per_pid
            .into_values()
            .map(|mut acc| {
                acc.values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let n = acc.values.len();
                let p50 = percentile(&acc.values, 0.50);
                let p95 = percentile(&acc.values, 0.95);
                let max = *acc.values.last().unwrap_or(&0.0);
                CpuAggregate { package: acc.package, pid: acc.pid, samples: n as u32, p50, p95, max }
            })
            .collect();
        out.sort_by(|a, b| b.p95.partial_cmp(&a.p95).unwrap_or(std::cmp::Ordering::Equal));
        Ok(out)
    }
}

struct ProcessAccumulator {
    pid: u32,
    package: Option<PackageName>,
    values: Vec<f32>,
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() { return 0.0; }
    let n = sorted.len();
    if n == 1 { return sorted[0]; }
    let pos = q * (n as f32 - 1.0);
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    let frac = pos - (lo as f32);
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_basic() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile(&v, 0.5) - 3.0).abs() < 0.001);
        assert!((percentile(&v, 0.95) - 4.8).abs() < 0.001);
        assert!((percentile(&v, 1.0) - 5.0).abs() < 0.001);
    }
}
