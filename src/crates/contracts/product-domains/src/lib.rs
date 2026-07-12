//! Product domain owner crate.
//!
//! Product subdomains live here when they can be compiled without depending on
//! the full BitFun core runtime assembly.

pub mod canvas;
pub mod review;

pub use review::{
    fallback_review_plan, validate_review_plan, ReviewMode, ReviewPlan, ReviewPlanSubject,
    ReviewPlanValidationError, ReviewSubjectCandidate, ReviewSubjectRole, MAX_REVIEW_SUBJECTS,
};

#[cfg(feature = "plugin-source")]
pub mod plugin_source;

#[cfg(feature = "miniapp")]
pub mod miniapp;

#[cfg(feature = "function-agents")]
pub mod function_agents;
