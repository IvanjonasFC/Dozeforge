//! Miscategorized app detector.
//!
//! Cross-references `usage_stats.lastTimeUsed` with the current standby bucket.
//! If an app hasn't been opened in >3 days but Android still has it in ACTIVE
//! or WORKING_SET, the system is wasting power on it. We surface those.

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::parsers::usage_stats::UsageEntry;
use crate::parsers::{PackageName, StandbyAssignment, StandbyBucket};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiscategorizedApp {
    pub package: PackageName,
    pub current_bucket: StandbyBucket,
    pub recommended_bucket: StandbyBucket,
    pub days_since_used: i64,
    pub reason: String,
}

pub struct MiscategorizedDetector<'a> {
    pub usage: &'a [UsageEntry],
    pub standby: &'a [StandbyAssignment],
    /// Threshold (days) above which Active/WorkingSet is considered wrong.
    pub days_threshold: i64,
}

impl<'a> Default for MiscategorizedDetector<'a> {
    fn default() -> Self {
        Self {
            usage: &[],
            standby: &[],
            days_threshold: 3,
        }
    }
}

impl<'a> MiscategorizedDetector<'a> {
    pub fn new(usage: &'a [UsageEntry], standby: &'a [StandbyAssignment]) -> Self {
        Self { usage, standby, days_threshold: 3 }
    }

    pub fn run(&self) -> Vec<MiscategorizedApp> {
        let now = Utc::now();
        let mut out = Vec::new();

        for assignment in self.standby {
            // Only flag apps currently in privileged buckets.
            let is_privileged = matches!(
                assignment.bucket,
                StandbyBucket::Active | StandbyBucket::WorkingSet
            );
            if !is_privileged {
                continue;
            }

            let usage = self.usage.iter().find(|u| u.package == assignment.package);
            let last_used = usage.and_then(|u| u.last_time_used);

            let days_since = match last_used {
                Some(ts) => (now - ts).num_days(),
                None => 999, // Never used in usagestats window
            };

            if days_since >= self.days_threshold {
                // Suggest a bucket proportional to staleness.
                let recommended = match days_since {
                    0..=2 => continue, // shouldn't happen due to threshold
                    3..=7 => StandbyBucket::Frequent,
                    8..=30 => StandbyBucket::Rare,
                    _ => StandbyBucket::Restricted,
                };

                let reason = match last_used {
                    Some(_) => format!(
                        "in {:?} bucket but last used {} day(s) ago",
                        assignment.bucket, days_since
                    ),
                    None => format!(
                        "in {:?} bucket but no recent usage recorded",
                        assignment.bucket
                    ),
                };

                out.push(MiscategorizedApp {
                    package: assignment.package.clone(),
                    current_bucket: assignment.bucket,
                    recommended_bucket: recommended,
                    days_since_used: days_since,
                    reason,
                });
            }
        }

        out.sort_by(|a, b| b.days_since_used.cmp(&a.days_since_used));
        out
    }
}
