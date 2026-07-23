//! Deep Review agent type and role constants.

pub const DEEP_REVIEW_AGENT_TYPE: &str = "DeepReview";
pub const REVIEW_JUDGE_AGENT_TYPE: &str = "ReviewJudge";
pub const REVIEW_FIXER_AGENT_TYPE: &str = "ReviewFixer";
pub const REVIEW_WORKER_AGENT_TYPE: &str = "ReviewWorker";

/// Non-discoverable compatibility ids for persisted sessions and manifests.
/// Direct historical invocations resolve to ReviewWorker and still pass the
/// same DeepReview visibility, manifest, read-only, and budget gates.
pub const LEGACY_REVIEW_WORKER_AGENT_TYPES: [&str; 6] = [
    "ReviewBusinessLogic",
    "ReviewPerformance",
    "ReviewSecurity",
    "ReviewArchitecture",
    "ReviewFrontend",
    "ReviewGeneral",
];
pub(crate) const MANAGED_REVIEW_MAX_FILES_PER_BATCH: usize = 40;
pub(crate) const MANAGED_REVIEW_MAX_BATCHES: usize = 8;
pub(crate) const MANAGED_REVIEW_MAX_PARALLEL_INSTANCES: usize = 2;
pub(crate) const MANAGED_REVIEW_MAX_WORKER_TIMEOUT_SECONDS: u64 = 120;

pub const CORE_REVIEWER_AGENT_TYPES: [&str; 1] = [REVIEW_WORKER_AGENT_TYPE];

pub const CONDITIONAL_REVIEWER_AGENT_TYPES: [&str; 0] = [];

pub fn canonical_review_worker_agent_type(agent_type: &str) -> &str {
    if LEGACY_REVIEW_WORKER_AGENT_TYPES.contains(&agent_type) {
        REVIEW_WORKER_AGENT_TYPE
    } else {
        agent_type
    }
}

pub fn is_review_worker_agent_type(agent_type: &str) -> bool {
    canonical_review_worker_agent_type(agent_type) == REVIEW_WORKER_AGENT_TYPE
}

pub(crate) const DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD: usize = 20;
pub(crate) const DEFAULT_MAX_SAME_ROLE_INSTANCES: usize = 3;
pub(crate) const DEFAULT_MAX_RETRIES_PER_ROLE: usize = 1;
