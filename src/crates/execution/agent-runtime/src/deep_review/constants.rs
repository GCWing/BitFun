//! Deep Review agent type and role constants.

pub const DEEP_REVIEW_AGENT_TYPE: &str = "DeepReview";
pub const REVIEW_JUDGE_AGENT_TYPE: &str = "ReviewJudge";
pub const REVIEW_FIXER_AGENT_TYPE: &str = "ReviewFixer";
pub const REVIEWER_BUSINESS_LOGIC_AGENT_TYPE: &str = "ReviewBusinessLogic";
pub const REVIEWER_PERFORMANCE_AGENT_TYPE: &str = "ReviewPerformance";
pub const REVIEWER_SECURITY_AGENT_TYPE: &str = "ReviewSecurity";
pub const REVIEWER_ARCHITECTURE_AGENT_TYPE: &str = "ReviewArchitecture";
pub const REVIEWER_FRONTEND_AGENT_TYPE: &str = "ReviewFrontend";

pub const CORE_REVIEWER_AGENT_TYPES: [&str; 4] = [
    REVIEWER_BUSINESS_LOGIC_AGENT_TYPE,
    REVIEWER_PERFORMANCE_AGENT_TYPE,
    REVIEWER_SECURITY_AGENT_TYPE,
    REVIEWER_ARCHITECTURE_AGENT_TYPE,
];

pub const CONDITIONAL_REVIEWER_AGENT_TYPES: [&str; 1] = [REVIEWER_FRONTEND_AGENT_TYPE];

pub(crate) const DEFAULT_REVIEWER_FILE_SPLIT_THRESHOLD: usize = 20;
pub(crate) const DEFAULT_MAX_SAME_ROLE_INSTANCES: usize = 3;
pub(crate) const DEFAULT_MAX_RETRIES_PER_ROLE: usize = 1;
