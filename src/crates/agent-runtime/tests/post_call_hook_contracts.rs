use bitfun_agent_runtime::post_call_hooks::{successful_tool_post_call_hooks, PostCallHookKind};

#[test]
fn successful_tool_call_routes_to_shared_context_measurement_hook() {
    assert_eq!(
        successful_tool_post_call_hooks(),
        [PostCallHookKind::DeepReviewSharedContextToolUse]
    );
}
