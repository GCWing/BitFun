#![cfg(feature = "hook-function-runtime")]

use async_trait::async_trait;
use openbitfun_runtime_ports::{
    HookFunctionAvailability, HookFunctionBeforeRequest, HookFunctionBeforeResult,
    HookFunctionCancelRequest, HookFunctionCancelResult, HookFunctionDisposeRequest,
    HookFunctionDisposeResult, HookFunctionGeneration, HookFunctionRegistrationBatch,
    HookFunctionRegistrationSink, HookFunctionReverseAsk, HookFunctionReverseReply,
    HookFunctionReverseSink, HookFunctionRuntime, HookFunctionStartRequest,
    HookFunctionToolContext, HookFunctionToolRequest, HookFunctionToolResult, PortResult,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct CapturingRegistrationSink {
    batches: Mutex<Vec<HookFunctionRegistrationBatch>>,
}

#[async_trait]
impl HookFunctionRegistrationSink for CapturingRegistrationSink {
    async fn publish_generation(&self, batch: HookFunctionRegistrationBatch) -> PortResult<()> {
        self.batches.lock().expect("batch lock").push(batch);
        Ok(())
    }
}

struct RejectingReverseSink;

#[async_trait]
impl HookFunctionReverseSink for RejectingReverseSink {
    async fn metadata(
        &self,
        _update: openbitfun_runtime_ports::HookFunctionReverseMetadata,
    ) -> PortResult<()> {
        Ok(())
    }

    async fn ask(&self, request: HookFunctionReverseAsk) -> PortResult<HookFunctionReverseReply> {
        Ok(HookFunctionReverseReply::Reject {
            feedback: Some(format!("{} denied", request.permission)),
        })
    }
}

struct ContractRuntime;

#[async_trait]
impl HookFunctionRuntime for ContractRuntime {
    fn availability(&self) -> HookFunctionAvailability {
        HookFunctionAvailability::Available
    }

    async fn start(
        &self,
        request: HookFunctionStartRequest,
        registrations: Arc<dyn HookFunctionRegistrationSink>,
        _reverse: Arc<dyn HookFunctionReverseSink>,
        _deadline: Duration,
    ) -> PortResult<HookFunctionGeneration> {
        registrations
            .publish_generation(HookFunctionRegistrationBatch {
                generation: request.generation.clone(),
                config: request.config,
                config_contributors: Vec::new(),
                config_contributions: Vec::new(),
                diagnostics: Vec::new(),
                hooks: Vec::new(),
                tools: Vec::new(),
            })
            .await?;
        Ok(request.generation)
    }

    async fn transform_tool_before(
        &self,
        request: HookFunctionBeforeRequest,
        _deadline: Duration,
    ) -> PortResult<HookFunctionBeforeResult> {
        Ok(HookFunctionBeforeResult { args: request.args })
    }

    async fn execute_tool(
        &self,
        request: HookFunctionToolRequest,
        _deadline: Duration,
    ) -> PortResult<HookFunctionToolResult> {
        Ok(HookFunctionToolResult {
            output: json!({ "received": request.args }),
            attachments: Vec::new(),
        })
    }

    async fn transform_tool_after(
        &self,
        request: openbitfun_runtime_ports::HookFunctionAfterRequest,
        _deadline: Duration,
    ) -> PortResult<openbitfun_runtime_ports::HookFunctionAfterResult> {
        Ok(request.output)
    }

    async fn cancel(
        &self,
        _request: HookFunctionCancelRequest,
        _deadline: Duration,
    ) -> PortResult<HookFunctionCancelResult> {
        Ok(HookFunctionCancelResult { stopped: true })
    }

    async fn dispose(
        &self,
        _request: HookFunctionDisposeRequest,
        _deadline: Duration,
    ) -> PortResult<HookFunctionDisposeResult> {
        Ok(HookFunctionDisposeResult { closed: true })
    }
}

#[tokio::test]
async fn one_runtime_starts_a_complete_generation_before_invocation() {
    let runtime = ContractRuntime;
    let generation = HookFunctionGeneration {
        instance_id: "instance-a".to_string(),
        generation_key: "generation-a".to_string(),
        revision: "revision-a".to_string(),
    };
    let registrations = Arc::new(CapturingRegistrationSink::default());
    let started = runtime
        .start(
            HookFunctionStartRequest {
                generation: generation.clone(),
                project_id: "project-a".to_string(),
                project_worktree: "C:/workspace".to_string(),
                project_created_at_ms: 42,
                config: json!({ "agent": {} }).as_object().unwrap().clone(),
                directory: "C:/workspace".to_string(),
                worktree: "C:/workspace".to_string(),
                plugins: Vec::new(),
                configuration_fingerprint: None,
                expected_content_digests: BTreeMap::new(),
                expected_review_digest: None,
            },
            registrations.clone(),
            Arc::new(RejectingReverseSink),
            Duration::from_secs(1),
        )
        .await
        .expect("generation starts");

    assert_eq!(started, generation);
    let batches = registrations.batches.lock().expect("batch lock");
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].generation, generation);
    assert_eq!(batches[0].config["agent"], json!({}));
}

#[tokio::test]
async fn typed_before_and_tool_results_preserve_generation_and_payloads() {
    let runtime = ContractRuntime;
    let generation = HookFunctionGeneration {
        instance_id: "instance-a".to_string(),
        generation_key: "generation-a".to_string(),
        revision: "revision-a".to_string(),
    };
    let before = runtime
        .transform_tool_before(
            HookFunctionBeforeRequest {
                generation: generation.clone(),
                tool_name: "write".to_string(),
                session_id: "session-a".to_string(),
                call_id: "call-a".to_string(),
                args: json!({ "path": "src/main.rs" }),
            },
            Duration::from_secs(1),
        )
        .await
        .expect("before transform");
    assert_eq!(before.args, json!({ "path": "src/main.rs" }));

    let result = runtime
        .execute_tool(
            HookFunctionToolRequest {
                generation,
                execution_id: "execution-a".to_string(),
                registration_id: "registration-a".to_string(),
                args: before.args,
                context: HookFunctionToolContext {
                    session_id: "session-a".to_string(),
                    message_id: "message-a".to_string(),
                    agent: "agent-a".to_string(),
                    call_id: Some("call-a".to_string()),
                },
            },
            Duration::from_secs(1),
        )
        .await
        .expect("tool result");
    assert_eq!(result.output["received"]["path"], "src/main.rs");
    assert!(result.attachments.is_empty());
}
