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
    pub fn record_action(
        &mut self,
        action_type: String,
        action_params: String,
        success: bool,
    ) {
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
                suggestion: "Screen state unchanged after multiple actions. Try a different approach or use accessibility tree instead of vision.".to_string(),
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
        let recent: Vec<_> = self.action_history.iter().rev().take(LOOP_DETECTION_WINDOW).collect();
        if recent.len() < pattern_len * MAX_LOOP_REPETITIONS {
            return None;
        }

        let pattern: Vec<_> = recent.iter().take(pattern_len).map(|r| &r.action_type).collect();
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
                    "Detected repeating pattern of {} actions (repeated {} times). Try: 1) Use accessibility tree (click_element/locate) instead of vision, 2) Use keyboard shortcuts instead of mouse, 3) Take a fresh screenshot to verify current state.",
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
            recent.iter().skip(1).all(|r| r.screenshot_hash == Some(first_hash))
        } else {
            false
        }
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
