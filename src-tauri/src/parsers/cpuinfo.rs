//! Parsers for `top -b -n 1 -o PID,S,%CPU,ARGS`.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::{Error, Result};

use super::{CpuSample, PackageName, Parser};

static TOP_ROW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(?P<pid>\d+)\s+(?P<state>\S)\s+(?P<cpu>\d+(?:\.\d+)?)\s+(?P<args>.+)$")
        .expect("top row regex compiles")
});

pub struct TopParser;

impl Parser for TopParser {
    type Output = Vec<CpuSample>;

    fn parse(&self, input: &str) -> Result<Vec<CpuSample>> {
        let mut samples = Vec::new();
        let mut seen_header = false;

        for line in input.lines() {
            let line_trim = line.trim();
            if line_trim.is_empty() { continue; }
            if !seen_header {
                if line.contains("PID") && line.contains("%CPU") {
                    seen_header = true;
                }
                continue;
            }

            let Some(caps) = TOP_ROW.captures(line) else { continue };
            let pid: u32 = caps["pid"].parse().unwrap_or(0);
            let state = caps["state"].chars().next().unwrap_or('?');
            let cpu: f32 = caps["cpu"].parse().unwrap_or(0.0);
            let args = caps["args"].trim().to_string();
            let package = derive_package(&args);

            samples.push(CpuSample { pid, package, args, cpu_percent: cpu, state });
        }

        if samples.is_empty() {
            return Err(Error::Parse {
                parser_name: "top",
                reason: "no rows parsed; header not found or output truncated".into(),
            });
        }

        Ok(samples)
    }
}

fn derive_package(args: &str) -> Option<PackageName> {
    let first = args.split_whitespace().next()?;
    let pkg = first.split(':').next()?.trim();
    if pkg.contains('.') && !pkg.starts_with('/') {
        Some(PackageName::from(pkg))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Tasks: 800
  PID S %CPU ARGS
 1234 S 12.5 com.example.heavy
 1235 Z  0.0 [zombie-proc]
 1236 R  3.1 com.google.android.gms:persistent
";

    #[test]
    fn parses_top_rows_with_state_and_cpu() {
        let parser = TopParser;
        let samples = parser.parse(SAMPLE).expect("parse ok");
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0].pid, 1234);
        assert_eq!(samples[0].cpu_percent, 12.5);
        assert_eq!(samples[0].package.as_ref().unwrap().as_str(), "com.example.heavy");
        assert!(samples[1].package.is_none());
        assert_eq!(samples[2].package.as_ref().unwrap().as_str(), "com.google.android.gms");
    }
}
