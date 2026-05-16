//! Generic subagent runtime primitives.
//!
//! This module is intentionally smaller than a scheduler.  It may contain
//! shared mechanics that are already proven generic across hidden subagent
//! execution paths, but it must not import Deep Review modules or define
//! Deep Review product policy.

pub(crate) mod queue_timing;
