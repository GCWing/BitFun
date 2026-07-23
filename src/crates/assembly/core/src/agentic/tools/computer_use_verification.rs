pub use tool_runtime::computer_use::{
    detect_visual_change, generate_retry_suggestion, RetryStrategy, VerificationResult,
};

use crate::util::errors::BitFunError;

pub fn should_retry_action(error: &BitFunError, action_type: &str) -> bool {
    tool_runtime::computer_use::should_retry_action_message(&error.to_string(), action_type)
}
