//! Post-action verification and smart retry logic.

use crate::util::errors::BitFunError;
use serde::{Deserialize, Serialize};

/// Verification result after an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub verified: bool,
    pub visual_change_detected: bool,
    pub change_percentage: f32,
    pub suggestion: Option<String>,
}

/// Retry strategy for failed actions
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    pub max_attempts: u32,
    pub current_attempt: u32,
    pub should_retry: bool,
    pub retry_delay_ms: u64,
}

impl RetryStrategy {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            current_attempt: 0,
            should_retry: true,
            retry_delay_ms: 500,
        }
    }

    pub fn next_attempt(&mut self) -> bool {
        self.current_attempt += 1;
        self.should_retry = self.current_attempt < self.max_attempts;
        self.should_retry
    }

    pub fn is_exhausted(&self) -> bool {
        self.current_attempt >= self.max_attempts
    }
}

/// Compare two screenshot hashes to detect visual changes
pub fn detect_visual_change(hash_before: u64, hash_after: u64) -> VerificationResult {
    let changed = hash_before != hash_after;

    // Simple change detection based on hash difference
    let change_pct = if changed { 100.0 } else { 0.0 };

    VerificationResult {
        verified: changed,
        visual_change_detected: changed,
        change_percentage: change_pct,
        suggestion: if !changed {
            Some("No visual change detected. Action may have failed or UI did not update. Consider: 1) Retry the action, 2) Verify element is clickable, 3) Try keyboard shortcut instead.".to_string())
        } else {
            None
        },
    }
}

/// Determine if an action should be retried based on error type
pub fn should_retry_action(error: &BitFunError, action_type: &str) -> bool {
    let error_msg = error.to_string().to_lowercase();

    // Retry on transient errors
    if error_msg.contains("timeout")
        || error_msg.contains("not found")
        || error_msg.contains("element moved")
        || error_msg.contains("stale")
    {
        return true;
    }

    // Don't retry on permission or configuration errors
    if error_msg.contains("permission")
        || error_msg.contains("not enabled")
        || error_msg.contains("not available")
    {
        return false;
    }

    // Retry click/locate actions by default
    matches!(action_type, "click" | "click_element" | "locate")
}

/// Generate retry suggestion based on failure context
pub fn generate_retry_suggestion(action_type: &str, attempt: u32) -> String {
    match action_type {
        "click" | "click_element" => {
            if attempt == 1 {
                "First retry: Taking fresh screenshot to verify element position.".to_string()
            } else {
                "Retry failed. Try: 1) Use accessibility tree (click_element), 2) Use keyboard shortcut, 3) Verify element is visible and clickable.".to_string()
            }
        }
        "locate" => {
            "Element not found. Try: 1) Broaden search criteria (use filter_combine: 'any'), 2) Use only role_substring or title_contains, 3) Verify app is focused.".to_string()
        }
        _ => format!("Retry attempt {} for action: {}", attempt, action_type),
    }
}
