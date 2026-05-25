use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMem {
    pub package: String,
    pub pid: u32,
    pub pss_kb: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemInfo {
    pub total_ram_kb: u64,
    pub free_ram_kb: u64,
    pub used_ram_kb: u64,
    pub top_processes: Vec<ProcessMem>,
}

pub struct MemInfoParser;

impl MemInfoParser {
    pub fn parse(input: &str) -> Result<MemInfo> {
        let mut info = MemInfo::default();
        let mut parsing_procs = false;

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }

            if line.starts_with("Total PSS by process:") {
                parsing_procs = true;
                continue;
            }
            if line.starts_with("Total PSS by OOM adjustment:") {
                parsing_procs = false;
                continue;
            }

            if parsing_procs {
                // Example: "1000000K: com.example.app (pid 1234 / activities)"
                if let Some((kb_str, rest)) = line.split_once("K: ") {
                    if let Ok(kb) = kb_str.trim().replace(",", "").parse::<u64>() {
                        let mut parts = rest.split(" (pid ");
                        let pkg = parts.next().unwrap_or("").trim().to_string();
                        let pid = if let Some(pid_str) = parts.next() {
                            pid_str.split(' ').next().unwrap_or("").parse::<u32>().unwrap_or(0)
                        } else {
                            0
                        };
                        
                        if !pkg.is_empty() && info.top_processes.len() < 20 {
                            info.top_processes.push(ProcessMem { package: pkg, pid, pss_kb: kb });
                        }
                    }
                }
            }

            if line.starts_with("Total RAM:") {
                if let Some(kb_str) = line.strip_prefix("Total RAM:") {
                    let num = kb_str.split('K').next().unwrap_or("").trim().replace(",", "");
                    info.total_ram_kb = num.parse().unwrap_or(0);
                }
            } else if line.starts_with("Free RAM:") {
                if let Some(kb_str) = line.strip_prefix("Free RAM:") {
                    let num = kb_str.split('K').next().unwrap_or("").trim().replace(",", "");
                    info.free_ram_kb = num.parse().unwrap_or(0);
                }
            } else if line.starts_with("Used RAM:") {
                if let Some(kb_str) = line.strip_prefix("Used RAM:") {
                    let num = kb_str.split('K').next().unwrap_or("").trim().replace(",", "");
                    info.used_ram_kb = num.parse().unwrap_or(0);
                }
            }
        }
        
        Ok(info)
    }
}
