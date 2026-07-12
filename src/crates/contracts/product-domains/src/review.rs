//! Product policy for selecting the least costly sufficient review path.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_REVIEW_SUBJECTS: usize = 16;
const MAX_REVIEW_FOCUS_CHARS: usize = 8_000;
const MAX_REVIEW_CANDIDATE_ID_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewIntent {
    Review,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTargetResolution {
    Resolved,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStrategyLevel {
    Quick,
    Normal,
    Deep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewLevel {
    L1,
    L2,
    L3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewExecutionMode {
    Standard,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewQualityDecisionReason {
    RiskScore,
    ExplicitStrict,
    UnresolvedTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTargetFacts {
    pub resolution: ReviewTargetResolution,
    pub file_count: u32,
    pub total_lines_changed: Option<u32>,
    pub security_sensitive_file_count: u32,
    pub workspace_area_count: u32,
    pub contract_surface_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQualityDecisionRequest {
    pub intent: ReviewIntent,
    pub target: ReviewTargetFacts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQualityDecision {
    pub level: ReviewLevel,
    pub execution_mode: ReviewExecutionMode,
    pub strategy_level: ReviewStrategyLevel,
    pub reason: ReviewQualityDecisionReason,
    pub score: u32,
    pub requires_consent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ReviewSubjectCandidate {
    Issue {
        id: String,
        web_url: String,
        host: String,
        project_path: String,
        issue_id: String,
    },
    PullRequest {
        id: String,
        web_url: String,
        host: String,
        project_path: String,
        pull_request_id: String,
    },
    GitRange {
        id: String,
        source_ref: String,
        target_ref: String,
    },
    Workspace {
        id: String,
        workspace_path: String,
    },
    ExplicitFiles {
        id: String,
        paths: Vec<String>,
    },
    ExternalReference {
        id: String,
        url: String,
    },
}

impl ReviewSubjectCandidate {
    pub fn id(&self) -> &str {
        match self {
            Self::Issue { id, .. }
            | Self::PullRequest { id, .. }
            | Self::GitRange { id, .. }
            | Self::Workspace { id, .. }
            | Self::ExplicitFiles { id, .. }
            | Self::ExternalReference { id, .. } => id,
        }
    }

    pub fn is_change_set(&self) -> bool {
        matches!(
            self,
            Self::PullRequest { .. }
                | Self::GitRange { .. }
                | Self::Workspace { .. }
                | Self::ExplicitFiles { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    SingleSubject,
    Comparative,
    RequirementTrace,
    MultiSubject,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewSubjectRole {
    Primary,
    Requirement,
    Baseline,
    CandidateImplementation,
    SupportingReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewPlanSubject {
    pub candidate_id: String,
    pub role: ReviewSubjectRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewPlan {
    pub mode: ReviewMode,
    pub subjects: Vec<ReviewPlanSubject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ReviewPlanValidationError {
    EmptyCandidateCatalog,
    TooManyCandidates { actual: usize, maximum: usize },
    TooManyPlanSubjects { actual: usize, maximum: usize },
    InvalidCandidateId,
    DuplicateCandidateCatalogId { candidate_id: String },
    UnknownCandidateId { candidate_id: String },
    MissingCandidateId { candidate_id: String },
    DuplicateCandidateId { candidate_id: String },
    InvalidModeSubjects { mode: ReviewMode, details: String },
}

impl std::fmt::Display for ReviewPlanValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCandidateCatalog => write!(formatter, "review candidate catalog is empty"),
            Self::TooManyCandidates { actual, maximum } => write!(
                formatter,
                "review candidate catalog contains {actual} subjects; at most {maximum} are allowed"
            ),
            Self::TooManyPlanSubjects { actual, maximum } => write!(
                formatter,
                "review plan contains {actual} subjects; at most {maximum} are allowed"
            ),
            Self::InvalidCandidateId => write!(formatter, "invalid review candidate id"),
            Self::DuplicateCandidateCatalogId { candidate_id } => {
                write!(
                    formatter,
                    "review candidate catalog repeats id: {candidate_id}"
                )
            }
            Self::UnknownCandidateId { candidate_id } => {
                write!(formatter, "unknown review candidate id: {candidate_id}")
            }
            Self::MissingCandidateId { candidate_id } => {
                write!(formatter, "review plan omits candidate id: {candidate_id}")
            }
            Self::DuplicateCandidateId { candidate_id } => {
                write!(
                    formatter,
                    "review plan repeats candidate id: {candidate_id}"
                )
            }
            Self::InvalidModeSubjects { mode, details } => {
                write!(formatter, "invalid subjects for {mode:?}: {details}")
            }
        }
    }
}

impl std::error::Error for ReviewPlanValidationError {}

pub fn validate_review_plan(
    candidates: &[ReviewSubjectCandidate],
    plan: &ReviewPlan,
) -> Result<(), ReviewPlanValidationError> {
    validate_candidate_catalog(candidates)?;
    if plan.subjects.len() > MAX_REVIEW_SUBJECTS {
        return Err(ReviewPlanValidationError::TooManyPlanSubjects {
            actual: plan.subjects.len(),
            maximum: MAX_REVIEW_SUBJECTS,
        });
    }

    let candidate_ids: HashSet<&str> = candidates.iter().map(ReviewSubjectCandidate::id).collect();
    let mut referenced_ids = HashSet::with_capacity(plan.subjects.len());

    for subject in &plan.subjects {
        if !valid_candidate_id(&subject.candidate_id) {
            return Err(ReviewPlanValidationError::InvalidCandidateId);
        }
        if !candidate_ids.contains(subject.candidate_id.as_str()) {
            return Err(ReviewPlanValidationError::UnknownCandidateId {
                candidate_id: subject.candidate_id.clone(),
            });
        }
        if !referenced_ids.insert(subject.candidate_id.as_str()) {
            return Err(ReviewPlanValidationError::DuplicateCandidateId {
                candidate_id: subject.candidate_id.clone(),
            });
        }
    }

    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| !referenced_ids.contains(candidate.id()))
    {
        return Err(ReviewPlanValidationError::MissingCandidateId {
            candidate_id: candidate.id().to_owned(),
        });
    }

    let non_supporting_count = plan
        .subjects
        .iter()
        .filter(|subject| subject.role != ReviewSubjectRole::SupportingReference)
        .count();
    let comparison_count = plan
        .subjects
        .iter()
        .filter(|subject| {
            matches!(
                subject.role,
                ReviewSubjectRole::Baseline | ReviewSubjectRole::CandidateImplementation
            )
        })
        .count();
    let requirement_count = plan
        .subjects
        .iter()
        .filter(|subject| subject.role == ReviewSubjectRole::Requirement)
        .count();
    let implementation_count = plan
        .subjects
        .iter()
        .filter(|subject| subject.role == ReviewSubjectRole::CandidateImplementation)
        .count();

    let invalid_details = match plan.mode {
        ReviewMode::SingleSubject if non_supporting_count != 1 => {
            Some("single_subject requires exactly one non-supporting subject")
        }
        ReviewMode::Comparative if comparison_count < 2 => {
            Some("comparative requires at least two baseline or candidate subjects")
        }
        ReviewMode::RequirementTrace if requirement_count == 0 || implementation_count == 0 => {
            Some("requirement_trace requires requirement and candidate implementation subjects")
        }
        ReviewMode::MultiSubject if non_supporting_count < 2 => {
            Some("multi_subject requires at least two non-supporting subjects")
        }
        _ => None,
    };

    if let Some(details) = invalid_details {
        return Err(ReviewPlanValidationError::InvalidModeSubjects {
            mode: plan.mode,
            details: details.to_owned(),
        });
    }

    Ok(())
}

pub fn fallback_review_plan(
    candidates: &[ReviewSubjectCandidate],
    focus: &str,
) -> Result<ReviewPlan, ReviewPlanValidationError> {
    validate_candidate_catalog(candidates)?;

    let has_issue = candidates
        .iter()
        .any(|candidate| matches!(candidate, ReviewSubjectCandidate::Issue { .. }));
    let has_change_set = candidates.iter().any(ReviewSubjectCandidate::is_change_set);
    let change_set_count = candidates
        .iter()
        .filter(|candidate| candidate.is_change_set())
        .count();
    let normalized_focus = focus
        .chars()
        .take(MAX_REVIEW_FOCUS_CHARS)
        .collect::<String>()
        .to_lowercase();
    let comparison_requested = has_comparison_language(&normalized_focus);
    let requirement_trace_requested = has_requirement_trace_language(&normalized_focus);

    if candidates.len() == 1 {
        return Ok(ReviewPlan {
            mode: ReviewMode::SingleSubject,
            subjects: fallback_subjects(candidates, |_| ReviewSubjectRole::Primary),
        });
    }

    if change_set_count >= 2 && comparison_requested {
        let mut comparison_index = 0;
        return Ok(ReviewPlan {
            mode: ReviewMode::Comparative,
            subjects: fallback_subjects(candidates, |candidate| {
                if !candidate.is_change_set() {
                    return ReviewSubjectRole::SupportingReference;
                }
                let role = if comparison_index == 0 {
                    ReviewSubjectRole::Baseline
                } else {
                    ReviewSubjectRole::CandidateImplementation
                };
                comparison_index += 1;
                role
            }),
        });
    }

    if candidates.len() >= 2 && comparison_requested {
        return Ok(ReviewPlan {
            mode: ReviewMode::Comparative,
            subjects: candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| ReviewPlanSubject {
                    candidate_id: candidate.id().to_owned(),
                    role: if index == 0 {
                        ReviewSubjectRole::Baseline
                    } else {
                        ReviewSubjectRole::CandidateImplementation
                    },
                })
                .collect(),
        });
    }

    if has_issue && has_change_set && requirement_trace_requested {
        return Ok(ReviewPlan {
            mode: ReviewMode::RequirementTrace,
            subjects: fallback_subjects(candidates, |candidate| match candidate {
                ReviewSubjectCandidate::Issue { .. } => ReviewSubjectRole::Requirement,
                candidate if candidate.is_change_set() => {
                    ReviewSubjectRole::CandidateImplementation
                }
                _ => ReviewSubjectRole::SupportingReference,
            }),
        });
    }

    Ok(ReviewPlan {
        mode: ReviewMode::MultiSubject,
        subjects: fallback_subjects(candidates, |_| ReviewSubjectRole::Primary),
    })
}

fn validate_candidate_catalog(
    candidates: &[ReviewSubjectCandidate],
) -> Result<(), ReviewPlanValidationError> {
    if candidates.is_empty() {
        return Err(ReviewPlanValidationError::EmptyCandidateCatalog);
    }
    if candidates.len() > MAX_REVIEW_SUBJECTS {
        return Err(ReviewPlanValidationError::TooManyCandidates {
            actual: candidates.len(),
            maximum: MAX_REVIEW_SUBJECTS,
        });
    }

    let mut candidate_ids = HashSet::with_capacity(candidates.len());
    for candidate in candidates {
        if !valid_candidate_id(candidate.id()) {
            return Err(ReviewPlanValidationError::InvalidCandidateId);
        }
        if !candidate_ids.insert(candidate.id()) {
            return Err(ReviewPlanValidationError::DuplicateCandidateCatalogId {
                candidate_id: candidate.id().to_owned(),
            });
        }
    }

    Ok(())
}

fn valid_candidate_id(candidate_id: &str) -> bool {
    !candidate_id.trim().is_empty()
        && candidate_id.len() <= MAX_REVIEW_CANDIDATE_ID_BYTES
        && !candidate_id.chars().any(char::is_control)
}

fn fallback_subjects(
    candidates: &[ReviewSubjectCandidate],
    mut role_for: impl FnMut(&ReviewSubjectCandidate) -> ReviewSubjectRole,
) -> Vec<ReviewPlanSubject> {
    candidates
        .iter()
        .map(|candidate| ReviewPlanSubject {
            candidate_id: candidate.id().to_owned(),
            role: role_for(candidate),
        })
        .collect()
}

fn has_comparison_language(focus: &str) -> bool {
    [
        "比较", "对比", "优劣", "区别", "差异", "比較", "對比", "優劣", "區別", "差異",
    ]
    .iter()
    .any(|term| focus.contains(term))
        || focus.contains("trade-off")
        || focus
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|word| {
                matches!(
                    word,
                    "compare"
                        | "compares"
                        | "compared"
                        | "comparing"
                        | "comparison"
                        | "comparative"
                        | "versus"
                        | "vs"
                        | "tradeoff"
                        | "tradeoffs"
                )
            })
}

fn has_requirement_trace_language(focus: &str) -> bool {
    [
        "对照 issue",
        "對照 issue",
        "对照需求",
        "對照需求",
        "是否满足该需求",
        "是否滿足該需求",
        "实现该 issue",
        "實現該 issue",
        "实现该需求",
        "實現該需求",
        "需求覆盖",
        "需求覆蓋",
        "验收标准",
        "驗收標準",
        "需求追踪",
        "需求追蹤",
        "需求追溯",
    ]
    .iter()
    .any(|term| focus.contains(term))
        || [
            "against the issue",
            "against the requirement",
            "against the requirements",
            "implements the requirement",
            "implements the requirements",
            "satisfies the requirement",
            "satisfies the requirements",
            "requirement coverage",
            "trace requirements",
            "acceptance criteria",
            "traceability",
            "implements the issue",
            "satisfies the issue",
        ]
        .iter()
        .any(|term| focus.contains(term))
}

pub fn decide_review_quality(request: ReviewQualityDecisionRequest) -> ReviewQualityDecision {
    let score = review_risk_score(&request.target);

    if request.intent == ReviewIntent::Strict {
        return decision(
            ReviewLevel::L3,
            ReviewStrategyLevel::Deep,
            ReviewQualityDecisionReason::ExplicitStrict,
            score,
        );
    }

    if request.target.resolution != ReviewTargetResolution::Resolved {
        return decision(
            ReviewLevel::L1,
            ReviewStrategyLevel::Quick,
            ReviewQualityDecisionReason::UnresolvedTarget,
            score,
        );
    }

    let risk_floor = if request.target.security_sensitive_file_count > 0
        || request.target.contract_surface_changed
        || request.target.workspace_area_count > 1
        || (request.target.file_count > 0 && request.target.total_lines_changed.is_none())
    {
        ReviewStrategyLevel::Normal
    } else {
        ReviewStrategyLevel::Quick
    };
    let strategy = strategy_for_score(score).max(risk_floor);

    strategy_decision(strategy, ReviewQualityDecisionReason::RiskScore, score)
}

fn review_risk_score(target: &ReviewTargetFacts) -> u32 {
    target
        .file_count
        .saturating_add(target.total_lines_changed.unwrap_or_default() / 100)
        .saturating_add(target.security_sensitive_file_count.saturating_mul(3))
        .saturating_add(
            target
                .workspace_area_count
                .saturating_sub(1)
                .saturating_mul(2),
        )
        .saturating_add(u32::from(target.contract_surface_changed).saturating_mul(2))
}

fn strategy_for_score(score: u32) -> ReviewStrategyLevel {
    match score {
        0..=5 => ReviewStrategyLevel::Quick,
        6..=20 => ReviewStrategyLevel::Normal,
        _ => ReviewStrategyLevel::Deep,
    }
}

fn strategy_decision(
    strategy: ReviewStrategyLevel,
    reason: ReviewQualityDecisionReason,
    score: u32,
) -> ReviewQualityDecision {
    let level = match strategy {
        ReviewStrategyLevel::Quick => ReviewLevel::L1,
        ReviewStrategyLevel::Normal => ReviewLevel::L2,
        ReviewStrategyLevel::Deep => ReviewLevel::L3,
    };
    decision(level, strategy, reason, score)
}

fn decision(
    level: ReviewLevel,
    strategy_level: ReviewStrategyLevel,
    reason: ReviewQualityDecisionReason,
    score: u32,
) -> ReviewQualityDecision {
    let execution_mode = match level {
        ReviewLevel::L1 => ReviewExecutionMode::Standard,
        ReviewLevel::L2 | ReviewLevel::L3 => ReviewExecutionMode::Strict,
    };

    ReviewQualityDecision {
        level,
        execution_mode,
        strategy_level,
        reason,
        score,
        requires_consent: matches!(level, ReviewLevel::L2 | ReviewLevel::L3),
    }
}
