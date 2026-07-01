use super::{LaunchReviewAgentTool, TaskTool};
use crate::agentic::agents::CustomSubagentConfig;
use crate::agentic::agents::{
    get_agent_registry, Agent, AgentCategory, SubAgentSource, UserContextPolicy,
};
use crate::agentic::deep_review::task_adapter as deep_review_task_adapter;
use crate::agentic::deep_review_policy::{
    DeepReviewBudgetTracker, DeepReviewExecutionPolicy, DeepReviewSubagentRole,
};
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::ToolRuntimeRestrictions;
use async_trait::async_trait;
use bitfun_runtime_ports::DelegationPolicy;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

struct PromptOrderTestAgent {
    id: String,
}

#[async_trait]
impl Agent for PromptOrderTestAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id
    }

    fn description(&self) -> &str {
        "Prompt ordering test agent"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "test_prompt_order_agent"
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        UserContextPolicy::empty()
    }

    fn default_tools(&self) -> Vec<String> {
        vec!["Read".to_string()]
    }
}

fn register_prompt_order_test_subagent(
    id: &str,
    source: SubAgentSource,
    custom_config: Option<CustomSubagentConfig>,
) {
    get_agent_registry().register_agent(
        Arc::new(PromptOrderTestAgent { id: id.to_string() }),
        AgentCategory::SubAgent,
        match source {
            SubAgentSource::Builtin => crate::agentic::agents::AgentSource::Builtin,
            SubAgentSource::Project => crate::agentic::agents::AgentSource::Project,
            SubAgentSource::User => crate::agentic::agents::AgentSource::User,
        },
        Some(source),
        custom_config,
    );
}

fn test_tool_context(agent_type: &str) -> ToolUseContext {
    ToolUseContext {
        tool_call_id: Some("tool-call-1".to_string()),
        agent_type: Some(agent_type.to_string()),
        session_id: Some("session-1".to_string()),
        dialog_turn_id: Some("turn-1".to_string()),
        workspace: None,
        unlocked_collapsed_tools: Vec::new(),
        primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
        custom_data: HashMap::new(),
        computer_use_host: None,
        runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
        runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
    }
}

fn find_agent_block_index(description: &str, agent_id: &str) -> usize {
    description
        .find(&format!("<agent type=\"{}\">", agent_id))
        .unwrap_or_else(|| panic!("expected agent block for {}", agent_id))
}

#[test]
fn task_prompt_guidance_omits_subagent_name_examples() {
    let description = TaskTool::new().render_description();
    assert!(!description.contains("subagent_type=\"Explore\""));
    assert!(!description.contains("subagent_type=\"FileFinder\""));
    assert!(!description.contains("For Explore"));
    assert!(!description.contains("Explore/FileFinder"));
    assert!(!description.contains("file-discovery"));
    assert!(!description.contains("listed investigation"));

    let schema = TaskTool::new().input_schema();
    let subagent_description = schema["properties"]["subagent_type"]["description"]
        .as_str()
        .expect("subagent_type description should be a string");
    assert!(!subagent_description.contains("Explore"));
    assert!(!subagent_description.contains("FileFinder"));
    assert!(!subagent_description.contains("available_agents"));
}

#[test]
fn task_schema_accepts_optional_model_id() {
    let schema = TaskTool::new().input_schema();

    assert_eq!(schema["properties"]["action"]["type"], "string");
    assert_eq!(schema["properties"]["model_id"]["type"], "string");
    assert!(schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str() == Some("action")));
    assert!(!schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str() == Some("model_id")));
}

#[test]
fn task_schema_describes_spawn_context_modes_as_exclusive() {
    let description = TaskTool::new().render_description();
    assert!(description.contains("The two modes are mutually exclusive"));
    assert!(description.contains("do not provide `subagent_type` when `fork_context=true`"));

    let schema = TaskTool::new().input_schema();
    let subagent_description = schema["properties"]["subagent_type"]["description"]
        .as_str()
        .expect("subagent_type description should be a string");
    assert!(subagent_description.contains("Do not provide with fork_context=true"));

    let fork_context_description = schema["properties"]["fork_context"]["description"]
        .as_str()
        .expect("fork_context description should be a string");
    assert!(fork_context_description.contains("do not provide subagent_type"));
}

#[tokio::test]
async fn launch_review_agent_schema_exposes_retry_without_session_or_fork_controls() {
    let context = test_tool_context("DeepReview");
    let schema = LaunchReviewAgentTool::new()
        .input_schema_for_model_with_context(Some(&context))
        .await;

    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["timeout_seconds"]["type"], "integer");
    assert_eq!(schema["properties"]["model_id"]["type"], "string");
    assert_eq!(schema["properties"]["retry"]["type"], "boolean");
    assert_eq!(schema["properties"]["auto_retry"]["type"], "boolean");
    assert_eq!(schema["properties"]["retry_coverage"]["type"], "object");
    assert!(schema["properties"].get("fork_context").is_none());
    assert!(schema["properties"].get("session_id").is_none());
    assert!(schema["properties"].get("run_in_background").is_none());
    assert!(schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str() == Some("subagent_type")));
}

#[test]
fn background_subagent_start_acknowledgement_uses_session_id_only() {
    let message = TaskTool::background_subagent_started_assistant_message("subagent-session-123");

    assert!(message.starts_with("Background subagent started successfully."));
    assert!(message.contains("session_id: \"subagent-session-123\""));
    assert!(message.contains("Avoid polling for status updates."));
    assert!(!message.contains("GeneralPurpose"));
    assert!(!message.contains("<background_task"));
    assert!(!message.contains("bg-subagent-123"));
    assert!(!message.contains("background_task_id="));
}

#[tokio::test]
async fn validate_input_requires_subagent_type_when_not_forking() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "spawn",
                "description": "delegate",
                "prompt": "Inspect the repo"
            }),
            None,
        )
        .await;

    assert!(!validation.result);
    assert!(validation
        .message
        .as_deref()
        .is_some_and(|message| message.contains("subagent_type is required")));
}

#[tokio::test]
async fn validate_input_infers_spawn_without_action_when_subagent_type_present() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "description": "delegate",
                "prompt": "Inspect the repo",
                "subagent_type": "Explore"
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_infers_spawn_without_action_when_forking_context() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "description": "delegate",
                "prompt": "Inspect the repo",
                "fork_context": true
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_accepts_fork_context_with_model_id() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "spawn",
                "description": "delegate",
                "prompt": "Inspect the repo",
                "fork_context": true,
                "model_id": "fast"
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_rejects_fork_context_with_subagent_type_as_mode_conflict() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "spawn",
                "description": "delegate",
                "prompt": "Continue with inherited context",
                "fork_context": true,
                "subagent_type": "Explore"
            }),
            None,
        )
        .await;

    assert!(!validation.result);
    let message = validation
        .message
        .as_deref()
        .expect("validation should explain the conflict");
    assert!(message.contains("subagent_type cannot be combined with fork_context=true"));
    assert!(message.contains("action is spawn"));
}

#[tokio::test]
async fn validate_input_accepts_send_input_session_id_without_subagent_type() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "send_input",
                "description": "continue",
                "prompt": "Continue the previous analysis",
                "session_id": "subagent-session-1"
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_accepts_send_input_with_model_id() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "send_input",
                "description": "continue",
                "prompt": "Continue the previous analysis",
                "session_id": "subagent-session-1",
                "model_id": "fast"
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_infers_send_input_without_action_when_session_id_present() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "description": "continue",
                "prompt": "Continue the previous analysis",
                "session_id": "subagent-session-1"
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_rejects_send_input_with_subagent_type() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "send_input",
                "description": "continue",
                "prompt": "Continue the previous analysis",
                "session_id": "subagent-session-1",
                "subagent_type": "Explore"
            }),
            None,
        )
        .await;

    assert!(!validation.result);
    assert!(validation
        .message
        .as_deref()
        .is_some_and(|message| message.contains("subagent_type is not allowed")));
}

#[tokio::test]
async fn validate_input_rejects_deep_review_retry_fields_for_regular_parent() {
    let context = test_tool_context("agentic");
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "spawn",
                "description": "delegate",
                "prompt": "Inspect the repo",
                "subagent_type": "Explore",
                "retry": true
            }),
            Some(&context),
        )
        .await;

    assert!(!validation.result);
    assert!(validation
        .message
        .as_deref()
        .is_some_and(|message| message.contains("only allowed for DeepReview")));
}

#[tokio::test]
async fn validate_input_rejects_timeout_for_regular_parent() {
    let context = test_tool_context("agentic");
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "spawn",
                "description": "delegate",
                "prompt": "Inspect the repo",
                "subagent_type": "Explore",
                "timeout_seconds": 30
            }),
            Some(&context),
        )
        .await;

    assert!(!validation.result);
    assert!(validation
        .message
        .as_deref()
        .is_some_and(|message| message.contains("timeout_seconds is only allowed for DeepReview")));
}

#[tokio::test]
async fn launch_review_agent_rejects_task_context_controls() {
    let context = test_tool_context("DeepReview");
    for field in ["action", "fork_context", "session_id", "run_in_background"] {
        let mut input = json!({
            "description": "delegate",
            "prompt": "Review security-sensitive files",
            "subagent_type": "ReviewSecurity"
        });
        input[field] = match field {
            "action" => json!("spawn"),
            "session_id" => json!("subagent-session-1"),
            _ => json!(false),
        };

        let validation = LaunchReviewAgentTool::new()
            .validate_input(&input, Some(&context))
            .await;

        assert!(
            !validation.result,
            "{field} should be rejected for LaunchReviewAgent"
        );
        assert!(validation
            .message
            .as_deref()
            .is_some_and(|message| message.contains("LaunchReviewAgent")));
    }
}

#[tokio::test]
async fn launch_review_agent_accepts_optional_model_id() {
    let context = test_tool_context("DeepReview");
    let validation = LaunchReviewAgentTool::new()
        .validate_input(
            &json!({
                "description": "Security review",
                "prompt": "Review security-sensitive files",
                "subagent_type": "ReviewSecurity",
                "model_id": "fast"
            }),
            Some(&context),
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn launch_review_agent_rejects_non_string_model_id() {
    let context = test_tool_context("DeepReview");
    let validation = LaunchReviewAgentTool::new()
        .validate_input(
            &json!({
                "description": "Security review",
                "prompt": "Review security-sensitive files",
                "subagent_type": "ReviewSecurity",
                "model_id": 7
            }),
            Some(&context),
        )
        .await;

    assert!(!validation.result);
    assert!(validation
        .message
        .as_deref()
        .is_some_and(|message| message.contains("model_id must be a string")));
}

#[tokio::test]
async fn validate_input_accepts_cancel_with_session_id_only() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "cancel",
                "session_id": "subagent-session-1"
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_accepts_cancel_with_description() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "cancel",
                "session_id": "subagent-session-1",
                "description": "cancel task"
            }),
            None,
        )
        .await;

    assert!(validation.result);
}

#[tokio::test]
async fn validate_input_rejects_cancel_with_prompt() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "cancel",
                "session_id": "subagent-session-1",
                "prompt": "Stop this work"
            }),
            None,
        )
        .await;

    assert!(!validation.result);
    assert!(validation
        .message
        .as_deref()
        .is_some_and(|message| message.contains("prompt is not allowed")));
}

#[tokio::test]
async fn task_tool_stays_available_without_enabled_subagents() {
    assert!(
        TaskTool::new().is_available_in_context(None).await,
        "Task should remain prompt-visible even when no fresh subagents are currently available"
    );
}

#[tokio::test]
async fn validate_input_rejects_fork_context_conflicting_fields() {
    let validation = TaskTool::new()
        .validate_input(
            &json!({
                "action": "spawn",
                "description": "delegate",
                "prompt": "Continue with inherited context",
                "fork_context": true,
                "session_id": "subagent-session-1"
            }),
            None,
        )
        .await;

    assert!(!validation.result);
    assert!(validation
        .message
        .as_deref()
        .is_some_and(|message| message.contains("session_id is not allowed")));
}

#[tokio::test]
async fn call_impl_rejects_nested_subagent_delegation() {
    let policy = DelegationPolicy::top_level().spawn_child();
    let context = ToolUseContext {
        tool_call_id: Some("tool-call-1".to_string()),
        agent_type: Some("agentic".to_string()),
        session_id: Some("session-1".to_string()),
        dialog_turn_id: Some("turn-1".to_string()),
        workspace: None,
        unlocked_collapsed_tools: Vec::new(),
        primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
        custom_data: HashMap::from([
            (
                "delegation_allow_subagent_spawn".to_string(),
                json!(policy.allow_subagent_spawn),
            ),
            (
                "delegation_nesting_depth".to_string(),
                json!(policy.nesting_depth),
            ),
        ]),
        computer_use_host: None,
        runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
        runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
    };

    let error = TaskTool::new()
        .call_impl(
            &json!({
                "action": "spawn",
                "description": "delegate",
                "prompt": "Inspect the repo",
                "subagent_type": "Explore"
            }),
            &context,
        )
        .await
        .expect_err("nested subagent delegation should be rejected");

    assert!(error
        .to_string()
        .contains("Recursive subagent delegation is blocked. Use direct tools instead."));
}

#[test]
fn deep_review_policy_allows_only_configured_team_members() {
    let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
        "extra_subagent_ids": [
            "ExtraReviewer",
            "DeepReview",
            "ReviewFixer",
            "ReviewJudge",
            "ReviewBusinessLogic"
        ]
    })));

    assert_eq!(
        policy.classify_subagent("ReviewBusinessLogic").unwrap(),
        DeepReviewSubagentRole::Reviewer
    );
    assert_eq!(
        policy.classify_subagent("ExtraReviewer").unwrap(),
        DeepReviewSubagentRole::Reviewer
    );
    assert_eq!(
        policy.classify_subagent("ReviewJudge").unwrap(),
        DeepReviewSubagentRole::Judge
    );
    assert!(policy.classify_subagent("ReviewFixer").is_err());
    assert!(policy.classify_subagent("CodeReview").is_err());
    assert!(policy.classify_subagent("DeepReview").is_err());
}

#[test]
fn resolve_subagent_timeout_uses_session_execution_timeout_as_floor() {
    assert_eq!(
        TaskTool::resolve_subagent_timeout_seconds(Some(300), Some(1200)),
        Some(1200)
    );
    assert_eq!(
        TaskTool::resolve_subagent_timeout_seconds(None, Some(1200)),
        Some(1200)
    );
    assert_eq!(
        TaskTool::resolve_subagent_timeout_seconds(Some(1800), Some(1200)),
        Some(1800)
    );
    assert_eq!(
        TaskTool::resolve_subagent_timeout_seconds(Some(300), None),
        Some(300)
    );
    assert_eq!(TaskTool::resolve_subagent_timeout_seconds(None, None), None);
}

#[test]
fn deep_review_policy_caps_reviewer_and_judge_timeouts() {
    let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
        "reviewer_timeout_seconds": 300,
        "judge_timeout_seconds": 240
    })));

    assert_eq!(
        policy.effective_timeout_seconds(DeepReviewSubagentRole::Reviewer, Some(900)),
        Some(300)
    );
    assert_eq!(
        policy.effective_timeout_seconds(DeepReviewSubagentRole::Reviewer, None),
        Some(300)
    );
    assert_eq!(
        policy.effective_timeout_seconds(DeepReviewSubagentRole::Judge, Some(900)),
        Some(240)
    );
}

#[test]
fn deep_review_cancelled_reviewer_result_tells_parent_not_to_relaunch() {
    let result = LaunchReviewAgentTool::deep_review_cancelled_reviewer_tool_result(
        "ReviewArchitecture",
        "Subagent task has been cancelled",
        42,
    );

    let ToolResult::Result {
        data,
        result_for_assistant,
        image_attachments,
    } = result
    else {
        panic!("cancelled reviewer should return a structured tool result");
    };

    assert_eq!(data["status"], "cancelled");
    assert_eq!(data["reason"], "Subagent task has been cancelled");
    assert_eq!(data["duration"], 42);
    assert!(image_attachments.is_none());

    let assistant_message = result_for_assistant.expect("assistant message should be present");
    assert!(assistant_message.contains("status=\"cancelled\""));
    assert!(assistant_message.contains("do not relaunch it automatically"));
}

#[tokio::test]
async fn description_with_context_filters_restricted_subagents_by_parent_agent() {
    let agentic_context = ToolUseContext {
        tool_call_id: None,
        agent_type: Some("agentic".to_string()),
        session_id: None,
        dialog_turn_id: None,
        workspace: None,
        unlocked_collapsed_tools: Vec::new(),
        primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
        custom_data: HashMap::new(),
        computer_use_host: None,
        runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
        runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
    };
    let deep_review_context = ToolUseContext {
        agent_type: Some("DeepReview".to_string()),
        ..agentic_context.clone()
    };

    let agentic_description =
        TaskTool::build_available_agents_context_section(Some(&agentic_context))
            .await
            .expect("agentic available agents should render");
    assert!(agentic_description.contains("<agent type=\"Explore\">"));
    assert!(!agentic_description.contains("<agent type=\"ReviewSecurity\">"));
    assert!(!agentic_description.contains("<agent type=\"ResearchSpecialist\">"));

    let deep_review_description =
        TaskTool::build_available_agents_context_section(Some(&deep_review_context))
            .await
            .expect("deep review available agents should render");
    assert!(deep_review_description.contains("<agent type=\"ReviewSecurity\">"));
    assert!(!deep_review_description.contains("<agent type=\"ResearchSpecialist\">"));
}

#[tokio::test]
async fn prompt_stability_description_with_context_renders_available_agents_in_stable_order() {
    let context = ToolUseContext {
        tool_call_id: None,
        agent_type: Some("agentic".to_string()),
        session_id: None,
        dialog_turn_id: None,
        workspace: None,
        unlocked_collapsed_tools: Vec::new(),
        primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
        custom_data: HashMap::new(),
        computer_use_host: None,
        runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
        runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
    };

    let builtin_a = "AAAPromptOrderBuiltin";
    let builtin_z = "ZZZPromptOrderBuiltin";
    let project_a = "AAAPromptOrderProject";
    let project_z = "ZZZPromptOrderProject";
    register_prompt_order_test_subagent(builtin_z, SubAgentSource::Builtin, None);
    register_prompt_order_test_subagent(builtin_a, SubAgentSource::Builtin, None);
    register_prompt_order_test_subagent(
        project_z,
        SubAgentSource::Project,
        Some(CustomSubagentConfig {
            model: "fast".to_string(),
        }),
    );
    register_prompt_order_test_subagent(
        project_a,
        SubAgentSource::Project,
        Some(CustomSubagentConfig {
            model: "fast".to_string(),
        }),
    );

    let description = TaskTool::build_available_agents_context_section(Some(&context))
        .await
        .expect("available agents should render");

    let builtin_a_index = find_agent_block_index(&description, builtin_a);
    let builtin_z_index = find_agent_block_index(&description, builtin_z);
    let project_a_index = find_agent_block_index(&description, project_a);
    let project_z_index = find_agent_block_index(&description, project_z);

    assert!(
        builtin_a_index < builtin_z_index,
        "builtin subagents should be sorted alphabetically"
    );
    assert!(
        builtin_z_index < project_a_index,
        "builtin subagents should render before project subagents"
    );
    assert!(
        project_a_index < project_z_index,
        "project subagents should be sorted alphabetically"
    );
}

#[test]
fn deep_review_policy_saturates_oversized_numeric_limits() {
    let policy = DeepReviewExecutionPolicy::from_config_value(Some(&json!({
        "reviewer_timeout_seconds": u64::MAX,
        "judge_timeout_seconds": u64::MAX
    })));

    assert_eq!(policy.reviewer_timeout_seconds, 3600);
    assert_eq!(policy.judge_timeout_seconds, 3600);
}

#[test]
fn deep_review_budget_tracker_caps_judge_per_turn() {
    let policy = DeepReviewExecutionPolicy::default();
    let tracker = DeepReviewBudgetTracker::default();

    tracker
        .record_task(
            "turn-1",
            &policy,
            DeepReviewSubagentRole::Judge,
            "ReviewJudge",
            false,
        )
        .unwrap();
    assert!(tracker
        .record_task(
            "turn-1",
            &policy,
            DeepReviewSubagentRole::Judge,
            "ReviewJudge",
            false,
        )
        .is_err());

    tracker
        .record_task(
            "turn-2",
            &policy,
            DeepReviewSubagentRole::Judge,
            "ReviewJudge",
            false,
        )
        .unwrap();
}

#[test]
fn deep_review_concurrency_policy_blocks_reviewer_at_cap() {
    use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    // 0 active -> allowed
    assert!(policy
        .check_launch_allowed(0, DeepReviewSubagentRole::Reviewer, false)
        .is_ok());
    // 1 active -> allowed
    assert!(policy
        .check_launch_allowed(1, DeepReviewSubagentRole::Reviewer, false)
        .is_ok());
    // 2 active (at cap) -> blocked
    assert!(policy
        .check_launch_allowed(2, DeepReviewSubagentRole::Reviewer, false)
        .is_err());
}

#[test]
fn deep_review_concurrency_policy_returns_structured_cap_rejection() {
    use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let violation = policy
        .check_launch_allowed(2, DeepReviewSubagentRole::Reviewer, false)
        .expect_err("reviewer launch at cap should be rejected");
    let message = format!(
        "DeepReview concurrency policy violation: {}",
        violation.to_tool_error_message()
    );

    assert!(message.contains("deep_review_concurrency_cap_reached"));
    assert!(message.contains("Maximum parallel reviewer instances reached"));
}

#[tokio::test]
async fn deep_review_capacity_queue_waits_while_active_reviewer_is_running() {
    use crate::agentic::deep_review_policy::{
        deep_review_capacity_skip_count, deep_review_concurrency_cap_rejection_count,
        deep_review_effective_parallel_instances, try_begin_deep_review_active_reviewer,
        DeepReviewConcurrencyPolicy,
    };

    let turn_id = "turn-queue-active-wait";
    let tool_id = "tool-queue-active-wait";
    let occupied_a = try_begin_deep_review_active_reviewer(turn_id, 2)
        .expect("precondition should occupy first reviewer capacity");
    let occupied_b = try_begin_deep_review_active_reviewer(turn_id, 2)
        .expect("precondition should occupy second reviewer capacity");
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 0,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let turn_id_owned = turn_id.to_string();
    let tool_id_owned = tool_id.to_string();

    let handle = tokio::spawn(async move {
        deep_review_task_adapter::wait_for_reviewer_admission(
            "session-queue-active-wait",
            &turn_id_owned,
            &tool_id_owned,
            "ReviewSecurity",
            &policy,
            false,
            None,
        )
        .await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    assert!(
        !handle.is_finished(),
        "active Deep Review reviewers should keep the queued reviewer alive"
    );

    drop(occupied_a);
    drop(occupied_b);

    let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
        .await
        .expect("queue should become ready after active reviewers finish")
        .expect("spawned wait should not panic")
        .expect("queue wait should resolve");

    match outcome {
        super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
        super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
            panic!("active Deep Review reviewers should not cause a queue-expired skip");
        }
    }
    assert_eq!(deep_review_capacity_skip_count(turn_id), 0);
    assert_eq!(deep_review_concurrency_cap_rejection_count(turn_id), 0);
    assert_eq!(deep_review_effective_parallel_instances(turn_id, 2), 2);
}

#[tokio::test]
async fn deep_review_capacity_queue_starts_later_batch_when_reviewer_capacity_frees() {
    use crate::agentic::deep_review::task_adapter::DeepReviewLaunchBatchInfo;
    use crate::agentic::deep_review_policy::{
        deep_review_capacity_skip_count, deep_review_effective_parallel_instances,
        try_begin_deep_review_active_reviewer_for_launch_batch, DeepReviewConcurrencyPolicy,
    };

    let turn_id = "turn-launch-batch-fill-free-slot";
    let tool_id = "tool-launch-batch-fill-free-slot";
    let occupied_a =
        try_begin_deep_review_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-a"))
            .expect("launch batch admission should not fail")
            .expect("first batch reviewer should start");
    let occupied_b =
        try_begin_deep_review_active_reviewer_for_launch_batch(turn_id, 2, 1, Some("packet-b"))
            .expect("launch batch admission should not fail")
            .expect("second first-batch reviewer should start");
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 0,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let launch_batch_info = DeepReviewLaunchBatchInfo {
        packet_id: Some("packet-b".to_string()),
        launch_batch: 2,
    };
    let turn_id_owned = turn_id.to_string();
    let tool_id_owned = tool_id.to_string();

    let handle = tokio::spawn(async move {
        LaunchReviewAgentTool::wait_for_deep_review_reviewer_admission(
            "session-launch-batch-queue-wait",
            &turn_id_owned,
            &tool_id_owned,
            "ReviewSecurity",
            &policy,
            false,
            Some(&launch_batch_info),
        )
        .await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    assert!(
        !handle.is_finished(),
        "later launch batch should wait while reviewer capacity is full"
    );
    drop(occupied_a);

    let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
        .await
        .expect("later launch batch should become ready as soon as reviewer capacity frees")
        .expect("spawned wait should not panic")
        .expect("queue wait should resolve");

    match outcome {
        super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
        super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
            panic!("later launch batch should not expire after reviewer capacity frees");
        }
    }
    drop(occupied_b);
    assert_eq!(deep_review_capacity_skip_count(turn_id), 0);
    assert_eq!(deep_review_effective_parallel_instances(turn_id, 2), 2);
}

#[tokio::test]
async fn deep_review_capacity_queue_cancel_control_skips_waiting_reviewer() {
    use crate::agentic::deep_review_policy::{
        apply_deep_review_queue_control, deep_review_capacity_skip_count,
        try_begin_deep_review_active_reviewer, DeepReviewConcurrencyPolicy,
        DeepReviewQueueControlAction,
    };

    let turn_id = "turn-queue-cancel";
    let tool_id = "tool-queue-cancel";
    let _occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
        .expect("precondition should occupy reviewer capacity");
    apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Cancel);
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 1,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };

    let outcome = deep_review_task_adapter::wait_for_reviewer_admission(
        "session-queue-cancel",
        turn_id,
        tool_id,
        "ReviewSecurity",
        &policy,
        false,
        None,
    )
    .await
    .expect("queue wait should resolve");

    match outcome {
        super::DeepReviewQueueWaitOutcome::Skipped {
            queue_elapsed_ms, ..
        } => {
            assert!(queue_elapsed_ms < 100);
        }
        super::DeepReviewQueueWaitOutcome::Ready { .. } => {
            panic!("cancelled queue control should skip the waiting reviewer");
        }
    }
    assert_eq!(deep_review_capacity_skip_count(turn_id), 1);
}

#[tokio::test]
async fn deep_review_capacity_queue_records_one_runtime_wait_when_ready() {
    use crate::agentic::deep_review_policy::{
        deep_review_runtime_diagnostics_snapshot, try_begin_deep_review_active_reviewer,
        DeepReviewConcurrencyPolicy,
    };

    let turn_id = "turn-queue-ready-diagnostics";
    let tool_id = "tool-queue-ready-diagnostics";
    let occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
        .expect("precondition should occupy reviewer capacity");
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 1,
        stagger_seconds: 0,
        max_queue_wait_seconds: 1,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let turn_id_owned = turn_id.to_string();
    let tool_id_owned = tool_id.to_string();

    let handle = tokio::spawn(async move {
        deep_review_task_adapter::wait_for_reviewer_admission(
            "session-queue-ready-diagnostics",
            &turn_id_owned,
            &tool_id_owned,
            "ReviewSecurity",
            &policy,
            false,
            None,
        )
        .await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    drop(occupied);

    let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
        .await
        .expect("queue should become ready after capacity frees")
        .expect("spawned wait should not panic")
        .expect("queue wait should resolve");
    match outcome {
        super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
        super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
            panic!("freed capacity should allow the queued reviewer to run");
        }
    }

    let diagnostics = deep_review_runtime_diagnostics_snapshot(turn_id)
        .expect("runtime diagnostics should record terminal queue wait");
    assert_eq!(diagnostics.queue_wait_count, 1);
    assert_eq!(
        diagnostics.queue_wait_total_ms,
        diagnostics.queue_wait_max_ms
    );
}

#[tokio::test]
async fn deep_review_capacity_queue_pause_does_not_expire_until_continued() {
    use crate::agentic::deep_review_policy::{
        apply_deep_review_queue_control, try_begin_deep_review_active_reviewer,
        DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
    };

    let turn_id = "turn-queue-pause";
    let tool_id = "tool-queue-pause";
    let occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
        .expect("precondition should occupy reviewer capacity");
    apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Pause);
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 1,
        stagger_seconds: 0,
        max_queue_wait_seconds: 0,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let turn_id_owned = turn_id.to_string();
    let tool_id_owned = tool_id.to_string();

    let handle = tokio::spawn(async move {
        deep_review_task_adapter::wait_for_reviewer_admission(
            "session-queue-pause",
            &turn_id_owned,
            &tool_id_owned,
            "ReviewSecurity",
            &policy,
            false,
            None,
        )
        .await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    assert!(
        !handle.is_finished(),
        "paused queue wait should not expire while user pause is active"
    );

    apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Continue);
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    assert!(
        !handle.is_finished(),
        "continued queue wait should stay alive while reviewer capacity is still active"
    );
    drop(occupied);

    let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
        .await
        .expect("continued queue wait should finish")
        .expect("spawned wait should not panic")
        .expect("queue wait should resolve");
    match outcome {
        super::DeepReviewQueueWaitOutcome::Ready { .. } => {}
        super::DeepReviewQueueWaitOutcome::Skipped { .. } => {
            panic!("continued queue wait should run after reviewer capacity frees");
        }
    }
}

#[tokio::test]
async fn deep_review_capacity_queue_skip_optional_skips_optional_waiter() {
    use crate::agentic::deep_review_policy::{
        apply_deep_review_queue_control, try_begin_deep_review_active_reviewer,
        DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
    };

    let turn_id = "turn-queue-skip-optional";
    let tool_id = "tool-queue-skip-optional";
    let _occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
        .expect("precondition should occupy reviewer capacity");
    apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::SkipOptional);
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 1,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };

    let outcome = deep_review_task_adapter::wait_for_reviewer_admission(
        "session-queue-skip-optional",
        turn_id,
        tool_id,
        "ReviewCustom",
        &policy,
        true,
        None,
    )
    .await
    .expect("queue wait should resolve");

    match outcome {
        super::DeepReviewQueueWaitOutcome::Skipped {
            queue_elapsed_ms, ..
        } => {
            assert!(queue_elapsed_ms < 100);
        }
        super::DeepReviewQueueWaitOutcome::Ready { .. } => {
            panic!("optional queue control should skip optional reviewer");
        }
    }
}

#[test]
fn deep_review_concurrency_policy_blocks_judge_with_active_reviewers() {
    use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

    let policy = DeepReviewConcurrencyPolicy::default();
    // 1 active reviewer -> judge blocked
    assert!(policy
        .check_launch_allowed(1, DeepReviewSubagentRole::Judge, false)
        .is_err());
    // 0 active reviewers, no judge pending -> judge allowed
    assert!(policy
        .check_launch_allowed(0, DeepReviewSubagentRole::Judge, false)
        .is_ok());
    // 0 active reviewers, judge already pending -> blocked
    assert!(policy
        .check_launch_allowed(0, DeepReviewSubagentRole::Judge, true)
        .is_err());
}

#[test]
fn deep_review_retry_guidance_includes_budget_info() {
    // Verify that the retry budget tracking functions work correctly
    // for the retry guidance injected in task_tool.
    use crate::agentic::deep_review_policy::{
        deep_review_max_retries_per_role, deep_review_retries_used,
    };

    // Default max retries should be 1
    assert_eq!(deep_review_max_retries_per_role("nonexistent-turn"), 1);

    // Retries used for a nonexistent turn should be 0
    assert_eq!(
        deep_review_retries_used("nonexistent-turn", "ReviewSecurity"),
        0
    );
}

#[test]
fn deep_review_retry_guidance_uses_manifest_policy_limit() {
    use crate::agentic::deep_review_policy::DeepReviewExecutionPolicy;

    let manifest = serde_json::json!({
        "reviewMode": "deep",
        "executionPolicy": {
            "maxRetriesPerRole": 2
        }
    });
    let policy = DeepReviewExecutionPolicy::default().with_run_manifest_execution_policy(&manifest);

    assert_eq!(
        LaunchReviewAgentTool::deep_review_retry_guidance_max_retries(
            Some(&policy),
            "nonexistent-turn"
        ),
        2
    );
}

#[test]
fn deep_review_retry_guidance_only_applies_to_initial_reviewer_timeout() {
    assert!(
        LaunchReviewAgentTool::should_emit_deep_review_retry_guidance(
            true,
            false,
            Some(DeepReviewSubagentRole::Reviewer)
        )
    );
    assert!(!LaunchReviewAgentTool::should_emit_deep_review_retry_guidance(true, false, None));
    assert!(
        !LaunchReviewAgentTool::should_emit_deep_review_retry_guidance(
            true,
            false,
            Some(DeepReviewSubagentRole::Judge)
        )
    );
    assert!(
        !LaunchReviewAgentTool::should_emit_deep_review_retry_guidance(
            true,
            true,
            Some(DeepReviewSubagentRole::Reviewer)
        )
    );
    assert!(
        !LaunchReviewAgentTool::should_emit_deep_review_retry_guidance(
            false,
            false,
            Some(DeepReviewSubagentRole::Reviewer)
        )
    );
}

#[test]
fn deep_review_auto_retry_requires_review_team_opt_in() {
    use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 4,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };

    let violation = LaunchReviewAgentTool::ensure_deep_review_auto_retry_allowed(
        &policy,
        "turn-auto-retry-disabled",
    )
    .expect_err("auto retry must be disabled by default");

    assert_eq!(violation.code, "deep_review_auto_retry_disabled");
    assert_eq!(
        LaunchReviewAgentTool::auto_retry_suppression_reason(violation.code),
        "auto_retry_disabled"
    );
}

#[test]
fn deep_review_auto_retry_opt_in_allows_guarded_admission() {
    use crate::agentic::deep_review_policy::DeepReviewConcurrencyPolicy;

    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 4,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: true,
        auto_retry_elapsed_guard_seconds: 180,
    };

    LaunchReviewAgentTool::ensure_deep_review_auto_retry_allowed(
        &policy,
        "turn-auto-retry-enabled",
    )
    .expect("opted-in auto retry should pass the admission gate before budget checks");
}

#[test]
fn deep_review_retry_rejects_missing_structured_coverage() {
    let manifest = json!({
        "workPackets": [
            {
                "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                "phase": "reviewer",
                "subagentId": "ReviewSecurity",
                "timeoutSeconds": 600,
                "assignedScope": {
                    "files": [
                        "src/crates/assembly/core/src/auth.rs",
                        "src/crates/assembly/core/src/token.rs"
                    ]
                }
            }
        ]
    });
    let input = json!({
        "retry": true
    });

    let violation = LaunchReviewAgentTool::ensure_deep_review_retry_coverage(
        &input,
        "ReviewSecurity",
        Some(&manifest),
    )
    .expect_err("missing retry coverage should be rejected");

    assert_eq!(violation.code, "deep_review_retry_missing_coverage");
}

#[test]
fn deep_review_retry_rejects_broad_scope() {
    let manifest = json!({
        "workPackets": [
            {
                "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                "phase": "reviewer",
                "subagentId": "ReviewSecurity",
                "timeoutSeconds": 600,
                "assignedScope": {
                    "files": [
                        "src/crates/assembly/core/src/auth.rs",
                        "src/crates/assembly/core/src/token.rs"
                    ]
                }
            }
        ]
    });
    let input = json!({
        "retry": true,
        "timeout_seconds": 300,
        "retry_coverage": {
            "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
            "source_status": "partial_timeout",
            "covered_files": [
                "src/crates/assembly/core/src/auth.rs"
            ],
            "retry_scope_files": [
                "src/crates/assembly/core/src/auth.rs",
                "src/crates/assembly/core/src/token.rs"
            ]
        }
    });

    let violation = LaunchReviewAgentTool::ensure_deep_review_retry_coverage(
        &input,
        "ReviewSecurity",
        Some(&manifest),
    )
    .expect_err("retrying the full packet should be rejected");

    assert_eq!(violation.code, "deep_review_retry_scope_not_reduced");
}

#[test]
fn deep_review_retry_rejects_timeout_that_is_not_lowered() {
    let manifest = json!({
        "workPackets": [
            {
                "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                "phase": "reviewer",
                "subagentId": "ReviewSecurity",
                "timeoutSeconds": 600,
                "assignedScope": {
                    "files": [
                        "src/crates/assembly/core/src/auth.rs",
                        "src/crates/assembly/core/src/token.rs"
                    ]
                }
            }
        ]
    });
    let input = json!({
        "retry": true,
        "timeout_seconds": 600,
        "retry_coverage": {
            "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
            "source_status": "partial_timeout",
            "covered_files": [
                "src/crates/assembly/core/src/auth.rs"
            ],
            "retry_scope_files": [
                "src/crates/assembly/core/src/token.rs"
            ]
        }
    });

    let violation = LaunchReviewAgentTool::ensure_deep_review_retry_coverage(
        &input,
        "ReviewSecurity",
        Some(&manifest),
    )
    .expect_err("retry timeout must be lower than source timeout");

    assert_eq!(violation.code, "deep_review_retry_timeout_not_reduced");
}

#[test]
fn deep_review_retry_rejects_non_queueable_capacity_reason() {
    let manifest = json!({
        "workPackets": [
            {
                "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                "phase": "reviewer",
                "subagentId": "ReviewSecurity",
                "timeoutSeconds": 600,
                "assignedScope": {
                    "files": [
                        "src/crates/assembly/core/src/auth.rs",
                        "src/crates/assembly/core/src/token.rs"
                    ]
                }
            }
        ]
    });
    let input = json!({
        "retry": true,
        "retry_coverage": {
            "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
            "source_status": "capacity_skipped",
            "capacity_reason": "auth_error",
            "covered_files": [],
            "retry_scope_files": [
                "src/crates/assembly/core/src/token.rs"
            ]
        }
    });

    let violation = LaunchReviewAgentTool::ensure_deep_review_retry_coverage(
        &input,
        "ReviewSecurity",
        Some(&manifest),
    )
    .expect_err("non-queueable capacity failures must fail fast");

    assert_eq!(violation.code, "deep_review_retry_non_retryable_status");
}

#[test]
fn deep_review_provider_capacity_error_builds_capacity_skipped_payload_and_lowers_effective_cap() {
    use crate::agentic::deep_review_policy::{
        deep_review_effective_concurrency_snapshot, DeepReviewConcurrencyPolicy,
    };
    use crate::util::BitFunError;

    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 3,
        stagger_seconds: 0,
        max_queue_wait_seconds: 30,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let turn_id = "turn-provider-capacity-skip";
    let decision = LaunchReviewAgentTool::deep_review_capacity_decision_for_provider_error(
        &BitFunError::ai("Provider error: provider=openai, code=429, message=rate limit exceeded"),
    );
    assert!(decision.queueable);
    let reason = decision
        .reason
        .expect("provider rate limit should surface as capacity_skipped");
    let (data, assistant_message) =
        LaunchReviewAgentTool::deep_review_capacity_skip_result_for_provider_queue_outcome(
            reason,
            turn_id,
            "ReviewSecurity",
            &policy,
            42,
            0,
            None,
        );

    assert_eq!(data["status"], "capacity_skipped");
    assert_eq!(data["queue_skip_reason"], "provider_rate_limit");
    assert_eq!(data["effective_parallel_instances"], 2);
    assert!(assistant_message.contains("status=\"capacity_skipped\""));
    assert!(assistant_message.contains("reason=\"provider_rate_limit\""));
    assert_eq!(
        deep_review_effective_concurrency_snapshot(turn_id, 3).effective_parallel_instances,
        2
    );
}

#[test]
fn deep_review_provider_quota_error_is_not_capacity_skipped() {
    use crate::util::BitFunError;

    let decision = LaunchReviewAgentTool::deep_review_capacity_decision_for_provider_error(
        &BitFunError::ai("Provider error: provider=glm, code=1113, message=insufficient quota"),
    );

    assert!(
        !decision.queueable,
        "quota errors should remain fail-fast instead of entering capacity queue flow"
    );
}

#[tokio::test]
async fn deep_review_provider_capacity_queue_retries_when_active_reviewer_frees_capacity() {
    use crate::agentic::deep_review::task_adapter::DeepReviewProviderQueueWaitOutcome;
    use crate::agentic::deep_review_policy::{
        try_begin_deep_review_active_reviewer, DeepReviewCapacityQueueReason,
        DeepReviewConcurrencyPolicy,
    };

    let turn_id = "turn-provider-queue-active-release";
    let tool_id = "tool-provider-queue-active-release";
    let occupied = try_begin_deep_review_active_reviewer(turn_id, 2)
        .expect("precondition should occupy another reviewer slot");
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let turn_id_owned = turn_id.to_string();
    let tool_id_owned = tool_id.to_string();

    let handle = tokio::spawn(async move {
        LaunchReviewAgentTool::wait_for_deep_review_provider_capacity_retry(
            "session-provider-queue-active-release",
            &turn_id_owned,
            &tool_id_owned,
            "ReviewSecurity",
            &policy,
            DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            60,
            false,
        )
        .await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    assert!(
        !handle.is_finished(),
        "provider queue should keep waiting while no additional reviewer capacity freed"
    );
    drop(occupied);

    let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
        .await
        .expect("provider queue should wake when another active reviewer frees capacity")
        .expect("spawned wait should not panic");

    match outcome {
        DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
            queue_elapsed_ms,
            early_capacity_probe,
        } => {
            assert!(
                queue_elapsed_ms < 500,
                "early capacity wake should not wait for the full backoff window"
            );
            assert!(
                early_capacity_probe,
                "active reviewer release should be marked as an early provider capacity probe"
            );
        }
        DeepReviewProviderQueueWaitOutcome::Skipped { .. } => {
            panic!("provider queue should retry after active reviewer capacity frees")
        }
    }
}

#[tokio::test]
async fn deep_review_provider_retry_after_wait_ignores_active_reviewer_release() {
    use crate::agentic::deep_review::task_adapter::DeepReviewProviderQueueWaitOutcome;
    use crate::agentic::deep_review_policy::{
        try_begin_deep_review_active_reviewer, DeepReviewCapacityQueueReason,
        DeepReviewConcurrencyPolicy,
    };

    let turn_id = "turn-provider-retry-after-hard-wait";
    let tool_id = "tool-provider-retry-after-hard-wait";
    let occupied = try_begin_deep_review_active_reviewer(turn_id, 2)
        .expect("precondition should occupy another reviewer slot");
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 1,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let turn_id_owned = turn_id.to_string();
    let tool_id_owned = tool_id.to_string();

    let handle = tokio::spawn(async move {
        LaunchReviewAgentTool::wait_for_deep_review_provider_capacity_retry(
            "session-provider-retry-after-hard-wait",
            &turn_id_owned,
            &tool_id_owned,
            "ReviewSecurity",
            &policy,
            DeepReviewCapacityQueueReason::RetryAfter,
            1,
            false,
        )
        .await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    drop(occupied);
    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
    assert!(
        !handle.is_finished(),
        "retry-after waits should not be interrupted by local reviewer capacity release"
    );

    let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(1500), handle)
        .await
        .expect("retry-after wait should eventually finish")
        .expect("spawned wait should not panic");

    match outcome {
        DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
            early_capacity_probe,
            ..
        } => {
            assert!(
                !early_capacity_probe,
                "retry-after completion should be a natural cooldown retry"
            );
        }
        DeepReviewProviderQueueWaitOutcome::Skipped { .. } => {
            panic!("retry-after wait should retry after its bounded cooldown")
        }
    }
}

#[tokio::test]
async fn deep_review_provider_capacity_queue_cancel_control_skips_retry() {
    use crate::agentic::deep_review::task_adapter::{
        DeepReviewProviderQueueWaitOutcome, DeepReviewQueueWaitSkipReason,
    };
    use crate::agentic::deep_review_policy::{
        apply_deep_review_queue_control, deep_review_runtime_diagnostics_snapshot,
        DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
    };

    let turn_id = "turn-provider-queue-cancel";
    let tool_id = "tool-provider-queue-cancel";
    apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Cancel);
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 60,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };

    let outcome = LaunchReviewAgentTool::wait_for_deep_review_provider_capacity_retry(
        "session-provider-queue-cancel",
        turn_id,
        tool_id,
        "ReviewSecurity",
        &policy,
        DeepReviewCapacityQueueReason::ProviderRateLimit,
        60,
        false,
    )
    .await;

    match outcome {
        DeepReviewProviderQueueWaitOutcome::Skipped {
            queue_elapsed_ms,
            skip_reason,
        } => {
            assert!(queue_elapsed_ms < 100);
            assert_eq!(skip_reason, DeepReviewQueueWaitSkipReason::UserCancelled);
        }
        DeepReviewProviderQueueWaitOutcome::ReadyToRetry { .. } => {
            panic!("cancelled provider queue should not retry")
        }
    }

    let diagnostics = deep_review_runtime_diagnostics_snapshot(turn_id)
        .expect("provider queue should record diagnostics");
    assert_eq!(diagnostics.provider_capacity_queue_count, 1);
    assert_eq!(
        diagnostics
            .provider_capacity_queue_reason_counts
            .get("provider_rate_limit"),
        Some(&1)
    );
}

#[tokio::test]
async fn deep_review_provider_capacity_queue_pause_does_not_count_against_wait() {
    use crate::agentic::deep_review::task_adapter::DeepReviewProviderQueueWaitOutcome;
    use crate::agentic::deep_review_policy::{
        apply_deep_review_queue_control, DeepReviewCapacityQueueReason,
        DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
    };

    let turn_id = "turn-provider-queue-pause";
    let tool_id = "tool-provider-queue-pause";
    apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Pause);
    let policy = DeepReviewConcurrencyPolicy {
        max_parallel_instances: 2,
        stagger_seconds: 0,
        max_queue_wait_seconds: 1,
        batch_extras_separately: true,
        allow_bounded_auto_retry: false,
        auto_retry_elapsed_guard_seconds: 180,
    };
    let turn_id_owned = turn_id.to_string();
    let tool_id_owned = tool_id.to_string();

    let handle = tokio::spawn(async move {
        LaunchReviewAgentTool::wait_for_deep_review_provider_capacity_retry(
            "session-provider-queue-pause",
            &turn_id_owned,
            &tool_id_owned,
            "ReviewSecurity",
            &policy,
            DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
            1,
            false,
        )
        .await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
    assert!(
        !handle.is_finished(),
        "paused provider queue should not expire before continue"
    );

    apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Continue);
    let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(1500), handle)
        .await
        .expect("continued provider queue should finish")
        .expect("spawned wait should not panic");

    match outcome {
        DeepReviewProviderQueueWaitOutcome::ReadyToRetry {
            queue_elapsed_ms, ..
        } => {
            assert!(queue_elapsed_ms >= 900);
        }
        DeepReviewProviderQueueWaitOutcome::Skipped { .. } => {
            panic!("continued provider queue should retry after bounded wait")
        }
    }
}

#[test]
fn deep_review_retry_accepts_reduced_partial_timeout_scope() {
    let manifest = json!({
        "workPackets": [
            {
                "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                "phase": "reviewer",
                "subagentId": "ReviewSecurity",
                "timeoutSeconds": 600,
                "assignedScope": {
                    "files": [
                        "src/crates/assembly/core/src/auth.rs",
                        "src/crates/assembly/core/src/token.rs"
                    ]
                }
            }
        ]
    });
    let input = json!({
        "retry": true,
        "timeout_seconds": 300,
        "retry_coverage": {
            "source_packet_id": "reviewer:ReviewSecurity:group-1-of-1",
            "source_status": "partial_timeout",
            "covered_files": [
                "src/crates/assembly/core/src/auth.rs"
            ],
            "retry_scope_files": [
                "src/crates/assembly/core/src/token.rs"
            ]
        }
    });

    let retry_scope = LaunchReviewAgentTool::ensure_deep_review_retry_coverage(
        &input,
        "ReviewSecurity",
        Some(&manifest),
    )
    .expect("reduced retry scope should be accepted");

    assert_eq!(retry_scope, vec!["src/crates/assembly/core/src/token.rs"]);
}

#[test]
fn deep_review_retry_scope_prompt_prepend_bounds_review_files() {
    let prompt = LaunchReviewAgentTool::prompt_with_deep_review_retry_scope(
        "Continue the security review.",
        &["src/crates/assembly/core/src/token.rs".to_string()],
    );

    assert!(prompt.starts_with("<deep_review_retry_scope>"));
    assert!(prompt.contains("Review only the following retry_scope_files"));
    assert!(prompt.contains("- src/crates/assembly/core/src/token.rs"));
    assert!(prompt.ends_with("Continue the security review."));
}
