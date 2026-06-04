use bitfun_agent_runtime::post_call_hooks::{
    run_successful_tool_post_call_hooks, SuccessfulToolPostCallHookExecutor,
};
use serde_json::{json, Value};

#[derive(Default)]
struct RecordingExecutor {
    calls: Vec<(String, Value, String)>,
}

impl SuccessfulToolPostCallHookExecutor<&str> for RecordingExecutor {
    fn record_deep_review_shared_context_tool_use(
        &mut self,
        tool_name: &str,
        input: &Value,
        context: &&str,
    ) {
        self.calls
            .push((tool_name.to_string(), input.clone(), (*context).to_string()));
    }
}

#[test]
fn successful_tool_post_call_executor_runs_deep_review_measurement_route() {
    let mut executor = RecordingExecutor::default();
    run_successful_tool_post_call_hooks(
        "Read",
        &json!({ "file_path": "src/lib.rs" }),
        &"review-context",
        &mut executor,
    );

    assert_eq!(
        executor.calls,
        vec![(
            "Read".to_string(),
            json!({ "file_path": "src/lib.rs" }),
            "review-context".to_string()
        )]
    );
}
