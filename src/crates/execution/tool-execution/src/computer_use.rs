//! Computer Use optimization: action verification, loop detection, and retry logic.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum actions to track in history
const MAX_HISTORY_SIZE: usize = 50;

/// Loop detection window (check last N actions)
const LOOP_DETECTION_WINDOW: usize = 10;

/// Maximum identical action sequences before triggering loop detection
const MAX_LOOP_REPETITIONS: usize = 3;

/// Action record for history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    pub timestamp_ms: u64,
    pub action_type: String,
    pub action_params: String,
    pub success: bool,
    pub screenshot_hash: Option<u64>,
}

/// Loop detection result
#[derive(Debug, Clone)]
pub struct LoopDetectionResult {
    pub is_loop: bool,
    pub pattern_length: usize,
    pub repetitions: usize,
    pub suggestion: String,
}

/// Computer Use session optimizer
#[derive(Debug)]
pub struct ComputerUseOptimizer {
    action_history: VecDeque<ActionRecord>,
    last_screenshot_hash: Option<u64>,
}

impl ComputerUseOptimizer {
    pub fn new() -> Self {
        Self {
            action_history: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            last_screenshot_hash: None,
        }
    }

    /// Record an action in history
    pub fn record_action(&mut self, action_type: String, action_params: String, success: bool) {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let record = ActionRecord {
            timestamp_ms,
            action_type,
            action_params,
            success,
            screenshot_hash: self.last_screenshot_hash,
        };

        self.action_history.push_back(record);
        if self.action_history.len() > MAX_HISTORY_SIZE {
            self.action_history.pop_front();
        }
    }

    /// Update screenshot hash for visual change detection
    pub fn update_screenshot_hash(&mut self, hash: u64) {
        self.last_screenshot_hash = Some(hash);
    }

    /// Detect if agent is stuck in a loop
    pub fn detect_loop(&self) -> LoopDetectionResult {
        if self.action_history.len() < LOOP_DETECTION_WINDOW {
            return LoopDetectionResult {
                is_loop: false,
                pattern_length: 0,
                repetitions: 0,
                suggestion: String::new(),
            };
        }

        // Check for repeating action patterns
        for pattern_len in 2..=5 {
            if let Some(result) = self.check_pattern_repetition(pattern_len) {
                if result.repetitions >= MAX_LOOP_REPETITIONS {
                    return result;
                }
            }
        }

        // Check for screenshot stagnation (same view, different actions)
        if self.check_screenshot_stagnation() {
            return LoopDetectionResult {
                is_loop: true,
                pattern_length: 0,
                repetitions: 0,
                suggestion: "Screen state unchanged after multiple actions. Try: 1) Use `key_chord` (Enter, Escape, Tab) instead of mouse, 2) Use `click_element` or `move_to_text` for precise targeting instead of screenshot drill, 3) Verify app is focused.".to_string(),
            };
        }

        // Check for excessive mouse usage without keyboard
        if self.check_excessive_mouse_usage() {
            return LoopDetectionResult {
                is_loop: true,
                pattern_length: 0,
                repetitions: 0,
                suggestion: "Detected heavy mouse usage without keyboard. Consider: 1) Use `key_chord` with Enter/Escape/Tab/Space instead of clicking buttons, 2) Use `move_to_text` (OCR) instead of screenshot-based targeting, 3) Use `click_element` (accessibility tree) when possible.".to_string(),
            };
        }

        // Check for screenshot → mouse_move → click pattern without using precise coordinates
        if self.check_screenshot_mouse_pattern() {
            return LoopDetectionResult {
                is_loop: true,
                pattern_length: 0,
                repetitions: 0,
                suggestion: "Detected screenshot + mouse move pattern. Use `move_to_text` for visible text or `click_element` for accessibility elements instead of estimating from JPEG. Use `global_center_x/y` from prior tool results with `use_screen_coordinates: true`.".to_string(),
            };
        }

        // Check for repeated move_to_text failures without trying keyboard navigation
        if self.check_repeated_move_to_text_failures() {
            return LoopDetectionResult {
                is_loop: true,
                pattern_length: 0,
                repetitions: 0,
                suggestion: "Detected repeated move_to_text failures. Try: 1) Use `key_chord` with Tab/Shift+Tab to navigate focus instead of OCR, 2) Try a shorter substring in `move_to_text`, 3) Verify you're targeting the correct window/app.".to_string(),
            };
        }

        // Check for screenshot → mouse_move loop without any clicks or progress
        if self.check_screenshot_mouse_loop() {
            return LoopDetectionResult {
                is_loop: true,
                pattern_length: 0,
                repetitions: 0,
                suggestion: "Detected screenshot → mouse_move loop without progress. Stop guessing coordinates! Try: 1) Use `key_chord` with Tab to navigate focus, 2) Use `move_to_text` with a visible text target, 3) Verify the correct app is focused.".to_string(),
            };
        }

        LoopDetectionResult {
            is_loop: false,
            pattern_length: 0,
            repetitions: 0,
            suggestion: String::new(),
        }
    }

    fn check_pattern_repetition(&self, pattern_len: usize) -> Option<LoopDetectionResult> {
        let recent: Vec<_> = self
            .action_history
            .iter()
            .rev()
            .take(LOOP_DETECTION_WINDOW)
            .collect();
        if recent.len() < pattern_len * MAX_LOOP_REPETITIONS {
            return None;
        }

        let pattern: Vec<_> = recent
            .iter()
            .take(pattern_len)
            .map(|r| &r.action_type)
            .collect();
        let mut reps = 1;

        for chunk in recent.chunks(pattern_len).skip(1) {
            if chunk.len() != pattern_len {
                break;
            }
            let chunk_types: Vec<_> = chunk.iter().map(|r| &r.action_type).collect();
            if chunk_types == pattern {
                reps += 1;
            } else {
                break;
            }
        }

        if reps >= MAX_LOOP_REPETITIONS {
            Some(LoopDetectionResult {
                is_loop: true,
                pattern_length: pattern_len,
                repetitions: reps,
                suggestion: format!(
                    "Detected repeating pattern of {} actions (repeated {} times). Try: 1) Use `key_chord` (Enter/Escape/Tab/Space) instead of mouse clicks, 2) Use `click_element` (accessibility tree) or `move_to_text` (OCR) instead of vision-based targeting, 3) Take a fresh screenshot to verify current state.",
                    pattern_len, reps
                ),
            })
        } else {
            None
        }
    }

    fn check_screenshot_stagnation(&self) -> bool {
        let recent: Vec<_> = self.action_history.iter().rev().take(6).collect();
        if recent.len() < 6 {
            return false;
        }

        // Check if last 6 actions had same screenshot hash (no visual change)
        if let Some(first_hash) = recent[0].screenshot_hash {
            recent
                .iter()
                .skip(1)
                .all(|r| r.screenshot_hash == Some(first_hash))
        } else {
            false
        }
    }

    /// Detect excessive mouse usage without any keyboard actions
    fn check_excessive_mouse_usage(&self) -> bool {
        let recent: Vec<_> = self.action_history.iter().rev().take(10).collect();
        if recent.len() < 10 {
            return false;
        }

        let mouse_actions = ["click", "mouse_move", "scroll", "drag", "pointer_move_rel"];
        let has_keyboard = recent
            .iter()
            .any(|r| r.action_type == "key_chord" || r.action_type == "type_text");

        let mouse_count = recent
            .iter()
            .filter(|r| mouse_actions.contains(&r.action_type.as_str()))
            .count();

        // If 8+ of last 10 actions are mouse and no keyboard usage
        !has_keyboard && mouse_count >= 8
    }

    /// Detect screenshot → mouse_move → click pattern without precise coordinates
    fn check_screenshot_mouse_pattern(&self) -> bool {
        let recent: Vec<_> = self.action_history.iter().rev().take(12).collect();
        if recent.len() < 9 {
            return false;
        }

        let mut screenshot_count = 0;
        let mut mouse_move_count = 0;
        let mut has_move_to_text = false;
        let mut has_click_element = false;

        for action in &recent {
            match action.action_type.as_str() {
                "screenshot" => screenshot_count += 1,
                "mouse_move" => mouse_move_count += 1,
                "move_to_text" => has_move_to_text = true,
                "click_element" => has_click_element = true,
                _ => {}
            }
        }

        // If we have many screenshots + mouse moves but no move_to_text/click_element
        screenshot_count >= 3 && mouse_move_count >= 2 && !has_move_to_text && !has_click_element
    }

    /// Detect repeated move_to_text failures without trying keyboard navigation
    fn check_repeated_move_to_text_failures(&self) -> bool {
        let recent: Vec<_> = self.action_history.iter().rev().take(8).collect();
        if recent.len() < 5 {
            return false;
        }

        let mut move_to_text_failures = 0;
        let mut has_keyboard = false;

        for action in &recent {
            if action.action_type == "move_to_text" && !action.success {
                move_to_text_failures += 1;
            }
            if action.action_type == "key_chord" {
                has_keyboard = true;
            }
        }

        // 3+ move_to_text failures and no keyboard attempts
        move_to_text_failures >= 3 && !has_keyboard
    }

    /// Detect screenshot → mouse_move loop without any clicks or progress
    fn check_screenshot_mouse_loop(&self) -> bool {
        let recent: Vec<_> = self.action_history.iter().rev().take(10).collect();
        if recent.len() < 6 {
            return false;
        }

        let mut screenshot_count = 0;
        let mut mouse_move_count = 0;
        let mut has_click = false;
        let mut has_keyboard = false;
        let mut has_move_to_text = false;

        for action in &recent {
            match action.action_type.as_str() {
                "screenshot" => screenshot_count += 1,
                "mouse_move" => mouse_move_count += 1,
                "click" => has_click = true,
                "key_chord" | "type_text" => has_keyboard = true,
                "move_to_text" => has_move_to_text = true,
                _ => {}
            }
        }

        // Many screenshots + mouse moves, but no clicks/keyboard/move_to_text
        screenshot_count >= 3
            && mouse_move_count >= 2
            && !has_click
            && !has_keyboard
            && !has_move_to_text
    }

    /// Get action history for backtracking
    pub fn get_history(&self) -> Vec<ActionRecord> {
        self.action_history.iter().cloned().collect()
    }

    /// Clear history (for new task)
    pub fn clear_history(&mut self) {
        self.action_history.clear();
        self.last_screenshot_hash = None;
    }
}

impl Default for ComputerUseOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple hash function for screenshot comparison
pub fn hash_screenshot_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in bytes.iter().step_by(1000) {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

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

    VerificationResult {
        verified: changed,
        visual_change_detected: changed,
        change_percentage: if changed { 100.0 } else { 0.0 },
        suggestion: if !changed {
            Some("No visual change detected. Action may have failed or UI did not update. Consider: 1) Retry the action, 2) Verify element is clickable, 3) Try keyboard shortcut instead.".to_string())
        } else {
            None
        },
    }
}

/// Determine if an action should be retried based on provider-neutral error text.
pub fn should_retry_action_message(error_message: &str, action_type: &str) -> bool {
    let error_msg = error_message.to_lowercase();

    if error_msg.contains("timeout")
        || error_msg.contains("not found")
        || error_msg.contains("element moved")
        || error_msg.contains("stale")
    {
        return true;
    }

    if error_msg.contains("permission")
        || error_msg.contains("not enabled")
        || error_msg.contains("not available")
    {
        return false;
    }

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

#[cfg(test)]
mod tests {
    use super::{
        detect_visual_change, generate_retry_suggestion, should_retry_action_message,
        ComputerUseOptimizer, RetryStrategy,
    };

    #[test]
    fn detects_repeated_action_loop() {
        let mut optimizer = ComputerUseOptimizer::new();
        for _ in 0..3 {
            optimizer.record_action("screenshot".to_string(), "{}".to_string(), true);
            optimizer.record_action("mouse_move".to_string(), "{}".to_string(), true);
        }
        for _ in 0..4 {
            optimizer.record_action("screenshot".to_string(), "{}".to_string(), true);
        }

        let result = optimizer.detect_loop();

        assert!(result.is_loop);
        assert!(!result.suggestion.is_empty());
    }

    #[test]
    fn hashes_screenshots_deterministically() {
        let bytes = b"abcdef0123456789";

        assert_eq!(
            super::hash_screenshot_bytes(bytes),
            super::hash_screenshot_bytes(bytes)
        );
        assert_ne!(
            super::hash_screenshot_bytes(bytes),
            super::hash_screenshot_bytes(b"z")
        );
    }

    #[test]
    fn verification_reports_visual_change_without_host_dependencies() {
        let changed = detect_visual_change(1, 2);
        let unchanged = detect_visual_change(1, 1);

        assert!(changed.verified);
        assert_eq!(changed.change_percentage, 100.0);
        assert!(!unchanged.verified);
        assert!(unchanged.suggestion.is_some());
    }

    #[test]
    fn retry_strategy_preserves_attempt_semantics() {
        let mut retry = RetryStrategy::new(2);

        assert!(retry.next_attempt());
        assert!(!retry.next_attempt());
        assert!(retry.is_exhausted());
    }

    #[test]
    fn retry_decision_uses_error_text_without_core_error_type() {
        assert!(should_retry_action_message("operation timeout", "drag"));
        assert!(!should_retry_action_message("permission denied", "click"));
        assert!(should_retry_action_message(
            "other failure",
            "click_element"
        ));
        assert!(!should_retry_action_message("other failure", "type_text"));
    }

    #[test]
    fn retry_suggestion_matches_existing_action_copy() {
        assert!(generate_retry_suggestion("click", 1).contains("fresh screenshot"));
        assert!(generate_retry_suggestion("locate", 2).contains("Broaden search criteria"));
    }
}
