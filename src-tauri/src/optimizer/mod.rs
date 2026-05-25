//! Action executor and one-click profile-based optimisation.

pub mod actions;
pub mod bloatware;
pub mod exclusions;
pub mod executor;
pub mod profile;

pub use actions::{OptimizationAction, OptimizationOutcome, OptimizationReport};
pub use exclusions::Exclusions;
pub use executor::Executor;
pub use profile::{Profile, ProfileBuilder, ProfilePreview, ProfileSummary};
