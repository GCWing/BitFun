//! Portable post-call hook routing decisions.

use serde_json::Value;

/// Hook categories that concrete runtime integrations may execute after a
/// successful tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostCallHookKind {
    DeepReviewSharedContextToolUse,
}

pub const fn successful_tool_post_call_hooks() -> [PostCallHookKind; 1] {
    [PostCallHookKind::DeepReviewSharedContextToolUse]
}

pub trait SuccessfulToolPostCallHookExecutor<C> {
    fn record_deep_review_shared_context_tool_use(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &C,
    );
}

pub fn run_successful_tool_post_call_hooks<C, E>(
    tool_name: &str,
    input: &Value,
    context: &C,
    executor: &mut E,
) where
    E: SuccessfulToolPostCallHookExecutor<C>,
{
    for hook in successful_tool_post_call_hooks() {
        match hook {
            PostCallHookKind::DeepReviewSharedContextToolUse => {
                executor.record_deep_review_shared_context_tool_use(tool_name, input, context);
            }
        }
    }
}
