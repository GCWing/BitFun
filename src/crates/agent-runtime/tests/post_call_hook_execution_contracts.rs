use bitfun_agent_runtime::post_call_hooks::{
    resolve_deep_review_shared_context_tool_use, run_successful_tool_post_call_hooks,
    DeepReviewSharedContextToolUseFacts, SuccessfulToolPostCallHookExecutor,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

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

fn reviewer_custom_data() -> HashMap<String, Value> {
    HashMap::from([
        (
            "deep_review_subagent_role".to_string(),
            Value::String("reviewer".to_string()),
        ),
        (
            "deep_review_parent_dialog_turn_id".to_string(),
            Value::String("turn-parent".to_string()),
        ),
        (
            "deep_review_subagent_type".to_string(),
            Value::String("ReviewSecurity".to_string()),
        ),
    ])
}

#[test]
fn deep_review_measurement_decision_normalizes_local_paths_and_filters_non_reviewers() {
    let custom_data = reviewer_custom_data();
    let record = resolve_deep_review_shared_context_tool_use(DeepReviewSharedContextToolUseFacts {
        tool_name: "Read",
        input: &json!({ "file_path": "C:/repo/src/lib.rs" }),
        custom_data: &custom_data,
        workspace_root: Some(Path::new("C:/repo")),
        is_remote: false,
        agent_type: Some("fallback"),
    })
    .expect("reviewer Read should record");

    assert_eq!(record.parent_turn_id, "turn-parent");
    assert_eq!(record.subagent_type, "ReviewSecurity");
    assert_eq!(record.tool_name, "Read");
    assert_eq!(record.measured_path, "src/lib.rs");

    let mut non_reviewer = custom_data.clone();
    non_reviewer.insert(
        "deep_review_subagent_role".to_string(),
        Value::String("planner".to_string()),
    );
    assert!(
        resolve_deep_review_shared_context_tool_use(DeepReviewSharedContextToolUseFacts {
            tool_name: "Read",
            input: &json!({ "file_path": "C:/repo/src/lib.rs" }),
            custom_data: &non_reviewer,
            workspace_root: Some(Path::new("C:/repo")),
            is_remote: false,
            agent_type: Some("fallback"),
        },)
        .is_none()
    );
}

#[test]
fn deep_review_measurement_decision_preserves_remote_paths_and_ignores_runtime_uri() {
    let custom_data = reviewer_custom_data();
    let remote = resolve_deep_review_shared_context_tool_use(DeepReviewSharedContextToolUseFacts {
        tool_name: "GetFileDiff",
        input: &json!({ "file_path": "/workspace/src/lib.rs" }),
        custom_data: &custom_data,
        workspace_root: Some(Path::new("/workspace")),
        is_remote: true,
        agent_type: Some("fallback"),
    })
    .expect("remote path should record without local relativization");
    assert_eq!(remote.measured_path, "/workspace/src/lib.rs");

    assert!(
        resolve_deep_review_shared_context_tool_use(DeepReviewSharedContextToolUseFacts {
            tool_name: "Read",
            input: &json!({ "file_path": "bitfun://runtime/session/output.txt" }),
            custom_data: &custom_data,
            workspace_root: Some(Path::new("/workspace")),
            is_remote: false,
            agent_type: Some("fallback"),
        },)
        .is_none()
    );
}
