//! Product policy for selecting the least costly sufficient review path.

use serde::{Deserialize, Serialize};

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
