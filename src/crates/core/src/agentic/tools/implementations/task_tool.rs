use crate::agentic::agents::{get_agent_registry, AgentInfo};
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::deep_review_policy::{
    classify_deep_review_capacity_error, clear_deep_review_queue_control_for_tool,
    deep_review_active_reviewer_count, deep_review_effective_concurrency_snapshot,
    deep_review_effective_parallel_instances, deep_review_has_judge_been_launched,
    deep_review_max_retries_per_role, deep_review_queue_control_snapshot,
    load_default_deep_review_policy, record_deep_review_capacity_skip,
    record_deep_review_effective_concurrency_capacity_error,
    record_deep_review_effective_concurrency_success, record_deep_review_task_budget,
    try_begin_deep_review_active_reviewer, DeepReviewActiveReviewerGuard,
    DeepReviewCapacityQueueReason, DeepReviewConcurrencyPolicy, DeepReviewExecutionPolicy,
    DeepReviewIncrementalCache, DeepReviewPolicyViolation, DeepReviewRunManifestGate,
    DeepReviewSubagentRole, DEEP_REVIEW_AGENT_TYPE,
};
use crate::agentic::events::{
    DeepReviewQueueReason, DeepReviewQueueState, DeepReviewQueueStatus, ErrorCategory,
};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::agentic::tools::pipeline::SubagentParentInfo;
use crate::agentic::tools::InputValidator;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::warn;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tokio::time::{sleep, Duration, Instant};

pub struct TaskTool;

const LARGE_TASK_PROMPT_SOFT_LINE_LIMIT: usize = 180;
const LARGE_TASK_PROMPT_SOFT_BYTE_LIMIT: usize = 16 * 1024;
#[cfg(test)]
const DEEP_REVIEW_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(not(test))]
const DEEP_REVIEW_QUEUE_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeepReviewQueueWaitSkipReason {
    QueueExpired,
    UserCancelled,
    OptionalSkipped,
}

enum DeepReviewQueueWaitOutcome {
    Ready {
        guard: DeepReviewActiveReviewerGuard<'static>,
    },
    Skipped {
        queue_elapsed_ms: u64,
        skip_reason: DeepReviewQueueWaitSkipReason,
    },
}

impl Default for TaskTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskTool {
    pub fn new() -> Self {
        Self
    }

    fn string_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
        keys.iter().find_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
    }

    fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
        keys.iter().find_map(|key| value.get(*key))
    }

    fn u64_for_any_key(value: &Value, keys: &[&str]) -> Option<u64> {
        keys.iter()
            .find_map(|key| value.get(*key).and_then(Value::as_u64))
    }

    fn string_array_for_any_key(
        value: &Value,
        keys: &[&str],
    ) -> Result<Vec<String>, DeepReviewPolicyViolation> {
        let Some(array) = Self::value_for_any_key(value, keys).and_then(Value::as_array) else {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_missing_coverage",
                format!("Retry coverage requires array field '{}'", keys[0]),
            ));
        };

        let mut result = Vec::with_capacity(array.len());
        for item in array {
            let Some(path) = item.as_str().map(str::trim).filter(|path| !path.is_empty()) else {
                return Err(DeepReviewPolicyViolation::new(
                    "deep_review_retry_invalid_coverage",
                    format!(
                        "Retry coverage field '{}' must contain non-empty strings",
                        keys[0]
                    ),
                ));
            };
            result.push(path.to_string());
        }

        Ok(result)
    }

    fn work_packets_from_manifest(run_manifest: Option<&Value>) -> Option<&Vec<Value>> {
        run_manifest?
            .get("workPackets")
            .or_else(|| run_manifest?.get("work_packets"))?
            .as_array()
    }

    fn packet_id_from_description(description: Option<&str>) -> Option<String> {
        let description = description?;
        let start = description.find("[packet ")? + "[packet ".len();
        let packet_id = description[start..].split(']').next()?.trim();
        (!packet_id.is_empty()).then(|| packet_id.to_string())
    }

    fn packet_belongs_to_subagent(packet: &Value, subagent_type: &str) -> bool {
        Self::string_for_any_key(
            packet,
            &["subagentId", "subagent_id", "subagentType", "subagent_type"],
        )
        .is_some_and(|value| value == subagent_type)
    }

    fn packet_id_for_manifest_packet(packet: &Value) -> Option<&str> {
        Self::string_for_any_key(packet, &["packetId", "packet_id"])
    }

    fn deep_review_packet_id_for_cache(
        subagent_type: &str,
        description: Option<&str>,
        run_manifest: Option<&Value>,
    ) -> Option<String> {
        let packets = Self::work_packets_from_manifest(run_manifest)?;

        if let Some(description_packet_id) = Self::packet_id_from_description(description) {
            return packets
                .iter()
                .any(|packet| {
                    Self::packet_id_for_manifest_packet(packet)
                        .is_some_and(|packet_id| packet_id == description_packet_id)
                        && Self::packet_belongs_to_subagent(packet, subagent_type)
                })
                .then_some(description_packet_id);
        }

        let mut matches = packets.iter().filter_map(|packet| {
            if Self::packet_belongs_to_subagent(packet, subagent_type) {
                Self::packet_id_for_manifest_packet(packet).map(str::to_string)
            } else {
                None
            }
        });
        let packet_id = matches.next()?;
        if matches.next().is_some() {
            None
        } else {
            Some(packet_id)
        }
    }

    fn attach_deep_review_cache(run_manifest: &mut Value, cache_value: Option<Value>) {
        if run_manifest.get("deepReviewCache").is_some() {
            return;
        }
        let Some(cache_value) = cache_value else {
            return;
        };
        if let Some(object) = run_manifest.as_object_mut() {
            object.insert("deepReviewCache".to_string(), cache_value);
        }
    }

    fn deep_review_retry_guidance_max_retries(
        effective_policy: Option<&DeepReviewExecutionPolicy>,
        dialog_turn_id: &str,
    ) -> usize {
        effective_policy
            .map(|policy| policy.max_retries_per_role)
            .unwrap_or_else(|| deep_review_max_retries_per_role(dialog_turn_id))
    }

    fn manifest_packet_by_id<'a>(
        run_manifest: Option<&'a Value>,
        packet_id: &str,
        subagent_type: &str,
    ) -> Option<&'a Value> {
        Self::work_packets_from_manifest(run_manifest)?
            .iter()
            .find(|packet| {
                Self::packet_id_for_manifest_packet(packet).is_some_and(|id| id == packet_id)
                    && Self::packet_belongs_to_subagent(packet, subagent_type)
            })
    }

    fn assigned_scope_files_for_packet(
        packet: &Value,
    ) -> Result<Vec<String>, DeepReviewPolicyViolation> {
        let Some(scope) = Self::value_for_any_key(packet, &["assignedScope", "assigned_scope"])
        else {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_missing_packet_scope",
                "Retry source packet is missing assigned_scope",
            ));
        };
        Self::string_array_for_any_key(scope, &["files"])
    }

    fn is_retryable_capacity_reason(reason: &str) -> bool {
        matches!(
            reason,
            "local_concurrency_cap"
                | "provider_rate_limit"
                | "provider_concurrency_limit"
                | "retry_after"
                | "temporary_overload"
        )
    }

    fn ensure_deep_review_retry_coverage(
        input: &Value,
        subagent_type: &str,
        run_manifest: Option<&Value>,
    ) -> Result<Vec<String>, DeepReviewPolicyViolation> {
        let Some(coverage) = Self::value_for_any_key(input, &["retry_coverage", "retryCoverage"])
        else {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_missing_coverage",
                "DeepReview retry requires structured retry_coverage metadata",
            ));
        };
        let packet_id = Self::string_for_any_key(coverage, &["source_packet_id", "sourcePacketId"])
            .ok_or_else(|| {
                DeepReviewPolicyViolation::new(
                    "deep_review_retry_missing_packet_id",
                    "DeepReview retry coverage requires source_packet_id",
                )
            })?;
        let source_status = Self::string_for_any_key(coverage, &["source_status", "sourceStatus"])
            .ok_or_else(|| {
                DeepReviewPolicyViolation::new(
                    "deep_review_retry_missing_status",
                    "DeepReview retry coverage requires source_status",
                )
            })?;
        match source_status {
            "partial_timeout" => {}
            "capacity_skipped" => {
                let capacity_reason =
                    Self::string_for_any_key(coverage, &["capacity_reason", "capacityReason"])
                        .unwrap_or_default();
                if !Self::is_retryable_capacity_reason(capacity_reason) {
                    return Err(DeepReviewPolicyViolation::new(
                        "deep_review_retry_non_retryable_status",
                        format!(
                            "DeepReview retry cannot redispatch non-transient capacity reason '{}'",
                            capacity_reason
                        ),
                    ));
                }
            }
            other => {
                return Err(DeepReviewPolicyViolation::new(
                    "deep_review_retry_non_retryable_status",
                    format!(
                        "DeepReview retry only supports partial_timeout or transient capacity failures, not '{}'",
                        other
                    ),
                ));
            }
        }

        let packet = Self::manifest_packet_by_id(run_manifest, packet_id, subagent_type)
            .ok_or_else(|| {
                DeepReviewPolicyViolation::new(
                    "deep_review_retry_unknown_packet",
                    format!(
                        "DeepReview retry source packet '{}' does not match reviewer '{}'",
                        packet_id, subagent_type
                    ),
                )
            })?;
        let original_files = Self::assigned_scope_files_for_packet(packet)?;
        Self::ensure_deep_review_retry_timeout(input, packet)?;
        let retry_scope_files =
            Self::string_array_for_any_key(coverage, &["retry_scope_files", "retryScopeFiles"])?;
        let covered_files =
            Self::string_array_for_any_key(coverage, &["covered_files", "coveredFiles"])?;
        if retry_scope_files.is_empty() {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_empty_scope",
                "DeepReview retry requires at least one retry_scope_files entry",
            ));
        }

        let original_file_set: HashSet<&str> = original_files.iter().map(String::as_str).collect();
        let mut retry_file_set = HashSet::new();
        for file in &retry_scope_files {
            if !retry_file_set.insert(file.as_str()) {
                return Err(DeepReviewPolicyViolation::new(
                    "deep_review_retry_duplicate_scope_file",
                    format!("DeepReview retry scope repeats file '{}'", file),
                ));
            }
            if !original_file_set.contains(file.as_str()) {
                return Err(DeepReviewPolicyViolation::new(
                    "deep_review_retry_scope_outside_packet",
                    format!(
                        "DeepReview retry file '{}' is outside source packet '{}'",
                        file, packet_id
                    ),
                ));
            }
        }
        if retry_scope_files.len() >= original_files.len() {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_scope_not_reduced",
                "DeepReview retry_scope_files must be smaller than the source packet scope",
            ));
        }

        for file in &covered_files {
            if !original_file_set.contains(file.as_str()) {
                return Err(DeepReviewPolicyViolation::new(
                    "deep_review_retry_coverage_outside_packet",
                    format!(
                        "DeepReview retry covered file '{}' is outside source packet '{}'",
                        file, packet_id
                    ),
                ));
            }
            if retry_file_set.contains(file.as_str()) {
                return Err(DeepReviewPolicyViolation::new(
                    "deep_review_retry_coverage_overlaps_scope",
                    format!(
                        "DeepReview retry covered file '{}' cannot also be in retry_scope_files",
                        file
                    ),
                ));
            }
        }

        Ok(retry_scope_files)
    }

    fn ensure_deep_review_retry_timeout(
        input: &Value,
        packet: &Value,
    ) -> Result<(), DeepReviewPolicyViolation> {
        let retry_timeout_seconds =
            Self::u64_for_any_key(input, &["timeout_seconds", "timeoutSeconds"]).unwrap_or(0);
        if retry_timeout_seconds == 0 {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_timeout_required",
                "DeepReview retry requires a positive timeout_seconds value",
            ));
        }

        let source_timeout_seconds =
            Self::u64_for_any_key(packet, &["timeoutSeconds", "timeout_seconds"]).unwrap_or(0);
        if source_timeout_seconds > 0 && retry_timeout_seconds >= source_timeout_seconds {
            return Err(DeepReviewPolicyViolation::new(
                "deep_review_retry_timeout_not_reduced",
                format!(
                    "DeepReview retry timeout_seconds ({}) must be lower than source timeout ({})",
                    retry_timeout_seconds, source_timeout_seconds
                ),
            ));
        }

        Ok(())
    }

    fn prompt_with_deep_review_retry_scope(prompt: &str, retry_scope_files: &[String]) -> String {
        let mut scoped_prompt = String::new();
        scoped_prompt.push_str("<deep_review_retry_scope>\n");
        scoped_prompt.push_str(
            "This is a bounded DeepReview retry. Review only the following retry_scope_files and treat any other files as background context only:\n",
        );
        for file in retry_scope_files {
            scoped_prompt.push_str("- ");
            scoped_prompt.push_str(file);
            scoped_prompt.push('\n');
        }
        scoped_prompt.push_str("</deep_review_retry_scope>\n\n");
        scoped_prompt.push_str(prompt);
        scoped_prompt
    }

    fn queue_reason_to_event_reason(
        reason: DeepReviewCapacityQueueReason,
    ) -> DeepReviewQueueReason {
        match reason {
            DeepReviewCapacityQueueReason::ProviderRateLimit => {
                DeepReviewQueueReason::ProviderRateLimit
            }
            DeepReviewCapacityQueueReason::ProviderConcurrencyLimit => {
                DeepReviewQueueReason::ProviderConcurrencyLimit
            }
            DeepReviewCapacityQueueReason::RetryAfter => DeepReviewQueueReason::RetryAfter,
            DeepReviewCapacityQueueReason::LocalConcurrencyCap => {
                DeepReviewQueueReason::LocalConcurrencyCap
            }
            DeepReviewCapacityQueueReason::TemporaryOverload => {
                DeepReviewQueueReason::TemporaryOverload
            }
        }
    }

    fn queue_reason_to_snake_case(reason: DeepReviewCapacityQueueReason) -> &'static str {
        match reason {
            DeepReviewCapacityQueueReason::ProviderRateLimit => "provider_rate_limit",
            DeepReviewCapacityQueueReason::ProviderConcurrencyLimit => "provider_concurrency_limit",
            DeepReviewCapacityQueueReason::RetryAfter => "retry_after",
            DeepReviewCapacityQueueReason::LocalConcurrencyCap => "local_concurrency_cap",
            DeepReviewCapacityQueueReason::TemporaryOverload => "temporary_overload",
        }
    }

    fn deep_review_capacity_reason_for_provider_error(
        error: &BitFunError,
    ) -> Option<DeepReviewCapacityQueueReason> {
        let detail = error.error_detail();
        let error_message = error.to_string();
        let code = detail.provider_code.as_deref().unwrap_or_default();
        let message = detail
            .provider_message
            .as_deref()
            .unwrap_or(error_message.as_str());
        let decision = classify_deep_review_capacity_error(code, message, None);
        if decision.queueable {
            return decision.reason;
        }

        match detail.category {
            ErrorCategory::RateLimit => Some(DeepReviewCapacityQueueReason::ProviderRateLimit),
            ErrorCategory::ProviderUnavailable => {
                Some(DeepReviewCapacityQueueReason::TemporaryOverload)
            }
            _ => None,
        }
    }

    fn deep_review_capacity_skip_result_for_provider_reason(
        reason: DeepReviewCapacityQueueReason,
        dialog_turn_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        duration_ms: u128,
    ) -> (Value, String) {
        let snapshot = record_deep_review_effective_concurrency_capacity_error(
            dialog_turn_id,
            conc_policy.max_parallel_instances,
            reason,
            None,
        );
        record_deep_review_capacity_skip(dialog_turn_id);

        let duration_ms = u64::try_from(duration_ms).unwrap_or(u64::MAX);
        let reason_code = Self::queue_reason_to_snake_case(reason);
        let assistant_message = format!(
            "Subagent '{}' was skipped because the provider reported transient DeepReview capacity pressure.\n<queue_result status=\"capacity_skipped\" reason=\"{}\" queue_elapsed_ms=\"0\" />",
            subagent_type, reason_code
        );
        let data = json!({
            "duration": duration_ms,
            "status": "capacity_skipped",
            "queue_elapsed_ms": 0,
            "max_queue_wait_seconds": conc_policy.max_queue_wait_seconds,
            "queue_skip_reason": reason_code,
            "effective_parallel_instances": snapshot.effective_parallel_instances
        });

        (data, assistant_message)
    }

    async fn emit_deep_review_queue_state(
        session_id: &str,
        dialog_turn_id: &str,
        tool_id: &str,
        subagent_type: &str,
        status: DeepReviewQueueStatus,
        reason: Option<DeepReviewCapacityQueueReason>,
        queued_reviewer_count: usize,
        active_reviewer_count: usize,
        optional_reviewer_count: Option<usize>,
        effective_parallel_instances: Option<usize>,
        queue_elapsed_ms: u64,
        max_queue_wait_seconds: u64,
    ) {
        let run_elapsed_ms = matches!(&status, DeepReviewQueueStatus::Running).then_some(0);
        if let Some(coordinator) = get_global_coordinator() {
            coordinator
                .emit_deep_review_queue_state_changed(
                    session_id,
                    dialog_turn_id,
                    DeepReviewQueueState {
                        tool_id: tool_id.to_string(),
                        subagent_type: subagent_type.to_string(),
                        status,
                        reason: reason.map(Self::queue_reason_to_event_reason),
                        queued_reviewer_count,
                        active_reviewer_count: Some(active_reviewer_count),
                        effective_parallel_instances,
                        optional_reviewer_count,
                        queue_elapsed_ms: Some(queue_elapsed_ms),
                        run_elapsed_ms,
                        max_queue_wait_seconds: Some(max_queue_wait_seconds),
                        session_concurrency_high: false,
                    },
                )
                .await;
        }
    }

    async fn wait_for_deep_review_reviewer_capacity(
        session_id: &str,
        dialog_turn_id: &str,
        tool_id: &str,
        subagent_type: &str,
        conc_policy: &DeepReviewConcurrencyPolicy,
        is_optional_reviewer: bool,
    ) -> BitFunResult<DeepReviewQueueWaitOutcome> {
        let decision = classify_deep_review_capacity_error(
            "deep_review_concurrency_cap_reached",
            "Maximum parallel reviewer instances reached",
            None,
        );
        let reason = decision
            .reason
            .unwrap_or(DeepReviewCapacityQueueReason::LocalConcurrencyCap);
        let started_at = Instant::now();
        let max_wait = Duration::from_secs(conc_policy.max_queue_wait_seconds);
        let mut paused_since: Option<Instant> = None;
        let mut paused_total = Duration::ZERO;
        let optional_reviewer_count = is_optional_reviewer.then_some(1);

        loop {
            let now = Instant::now();
            let current_pause_elapsed = paused_since
                .map(|paused_at| now.saturating_duration_since(paused_at))
                .unwrap_or_default();
            let queue_elapsed = now
                .saturating_duration_since(started_at)
                .saturating_sub(paused_total)
                .saturating_sub(current_pause_elapsed);
            let queue_elapsed_ms = u64::try_from(queue_elapsed.as_millis()).unwrap_or(u64::MAX);
            let active_reviewers = deep_review_active_reviewer_count(dialog_turn_id);
            let effective_parallel_instances = deep_review_effective_parallel_instances(
                dialog_turn_id,
                conc_policy.max_parallel_instances,
            );

            let control_snapshot = deep_review_queue_control_snapshot(dialog_turn_id, tool_id);
            if control_snapshot.cancelled
                || (is_optional_reviewer && control_snapshot.skip_optional)
            {
                record_deep_review_capacity_skip(dialog_turn_id);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                Self::emit_deep_review_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::CapacitySkipped,
                    Some(reason),
                    0,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                return Ok(DeepReviewQueueWaitOutcome::Skipped {
                    queue_elapsed_ms,
                    skip_reason: if control_snapshot.cancelled {
                        DeepReviewQueueWaitSkipReason::UserCancelled
                    } else {
                        DeepReviewQueueWaitSkipReason::OptionalSkipped
                    },
                });
            }

            if control_snapshot.paused {
                if paused_since.is_none() {
                    paused_since = Some(now);
                }
                Self::emit_deep_review_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::PausedByUser,
                    Some(reason),
                    1,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                sleep(DEEP_REVIEW_QUEUE_POLL_INTERVAL).await;
                continue;
            }

            if let Some(paused_at) = paused_since.take() {
                paused_total += now.saturating_duration_since(paused_at);
            }

            if let Some(guard) =
                try_begin_deep_review_active_reviewer(dialog_turn_id, effective_parallel_instances)
            {
                let active_reviewer_count = deep_review_active_reviewer_count(dialog_turn_id);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                Self::emit_deep_review_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::Running,
                    None,
                    0,
                    active_reviewer_count,
                    optional_reviewer_count,
                    Some(effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                return Ok(DeepReviewQueueWaitOutcome::Ready { guard });
            }

            if queue_elapsed >= max_wait {
                let snapshot = record_deep_review_effective_concurrency_capacity_error(
                    dialog_turn_id,
                    conc_policy.max_parallel_instances,
                    reason,
                    decision.retry_after_seconds.map(Duration::from_secs),
                );
                record_deep_review_capacity_skip(dialog_turn_id);
                clear_deep_review_queue_control_for_tool(dialog_turn_id, tool_id);
                Self::emit_deep_review_queue_state(
                    session_id,
                    dialog_turn_id,
                    tool_id,
                    subagent_type,
                    DeepReviewQueueStatus::CapacitySkipped,
                    Some(reason),
                    0,
                    active_reviewers,
                    optional_reviewer_count,
                    Some(snapshot.effective_parallel_instances),
                    queue_elapsed_ms,
                    conc_policy.max_queue_wait_seconds,
                )
                .await;
                return Ok(DeepReviewQueueWaitOutcome::Skipped {
                    queue_elapsed_ms,
                    skip_reason: DeepReviewQueueWaitSkipReason::QueueExpired,
                });
            }

            Self::emit_deep_review_queue_state(
                session_id,
                dialog_turn_id,
                tool_id,
                subagent_type,
                DeepReviewQueueStatus::QueuedForCapacity,
                Some(reason),
                1,
                active_reviewers,
                optional_reviewer_count,
                Some(effective_parallel_instances),
                queue_elapsed_ms,
                conc_policy.max_queue_wait_seconds,
            )
            .await;

            let remaining = max_wait.saturating_sub(queue_elapsed);
            sleep(DEEP_REVIEW_QUEUE_POLL_INTERVAL.min(remaining)).await;
        }
    }

    fn format_agent_descriptions(&self, agents: &[AgentInfo]) -> String {
        if agents.is_empty() {
            return String::new();
        }
        let mut out = String::from("<available_agents>\n");
        for agent in agents {
            out.push_str(&format!(
                "<agent type=\"{}\">\n<description>\n{}\n</description>\n<tools>{}</tools>\n</agent>\n",
                agent.id,
                agent.description,
                agent.default_tools.join(", ")
            ));
        }
        out.push_str("</available_agents>");
        out
    }

    fn render_description(&self, agent_descriptions: String) -> String {
        let agent_descriptions = if agent_descriptions.is_empty() {
            "<agents>No agents available</agents>".to_string()
        } else {
            agent_descriptions
        };

        format!(
            r#"Launch a new agent to handle complex, multi-step tasks autonomously. 

The Task tool launches specialized agents (subprocesses) that autonomously handle complex tasks. Each agent type has specific capabilities and tools available to it.

Available agents and the tools they have access to:
{}

When using the Task tool, you must specify `subagent_type` as a top-level tool argument to select which agent type to use. Do not put `subagent_type`, `description`, `workspace_path`, `model_id`, or `timeout_seconds` inside the prompt string.

When NOT to use the Task tool:
- If you want to read a specific file path, use the Read or Glob tool instead of the Task tool, to find the match more quickly
- If you are searching for a specific class definition like "class Foo", use the Glob tool instead, to find the match more quickly
- If you are searching for code within a specific file or set of 2-3 files, use the Read tool instead of the Task tool, to find the match more quickly
- For subagent_type=Explore: do not use it for simple lookups above; reserve it for broad or multi-area exploration where many tool rounds would be needed
- Other tasks that are not related to the agent descriptions above


Usage notes:
- Always include a short description (3-5 words) summarizing what the agent will do
- Provide clear, detailed prompt so the agent can work autonomously and return exactly the information you need.
- If 'workspace_path' is omitted, the task inherits the current workspace by default.
- The 'workspace_path' parameter must still be provided explicitly for the Explore and FileFinder agent.
- Use 'model_id' when a caller needs a specific model or model slot for the subagent. Omit it to use the agent default.
- Use 'timeout_seconds' when you need a hard deadline for the subagent. Omit it or set it to 0 to disable the timeout.
- For DeepReview only, set 'retry' to true when re-dispatching a reviewer after that same reviewer returned partial_timeout or an explicit transient capacity failure in the current turn. Retry calls must include retry_coverage with source_packet_id, source_status, covered_files, and a smaller retry_scope_files list.
- Launch multiple agents concurrently whenever possible, to maximize performance; to do that, use a single message with multiple tool calls
- When the agent is done, it will return a single message back to you.
- The agent's outputs should generally be trusted
- Clearly tell the agent whether you expect it to write code or just to do research (search, file reads, web fetches, etc.), since it is not aware of the user's intent
- If the agent description mentions that it should be used proactively, then you should try your best to use it without the user having to ask for it first. Use your judgement.
- If the user specifies that they want you to run agents "in parallel", you MUST send a single message with multiple Task tool calls. For example, if you need to launch both a code-reviewer agent and a test-runner agent in parallel, send a single message with both tool calls.

Example usage:

<example_agent_descriptions>
"code-reviewer": use this agent after you are done writing a signficant piece of code
"greeting-responder": use this agent when to respond to user greetings with a friendly joke
</example_agent_description>

<example>
user: "Please write a function that checks if a number is prime"
assistant: Sure let me write a function that checks if a number is prime
assistant: First let me use the Write tool to write a function that checks if a number is prime
assistant: I'm going to use the Write tool to write the following code:
<code>
function isPrime(n) {{
  if (n <= 1) return false
  for (let i = 2; i * i <= n; i++) {{
    if (n % i === 0) return false
  }}
  return true
}}
</code>
<commentary>
Since a signficant piece of code was written and the task was completed, now use the code-reviewer agent to review the code
</commentary>
assistant: Now let me use the code-reviewer agent to review the code
assistant: Uses the Task tool to launch the code-reviewer agent 
</example>

<example>
user: "Hello"
<commentary>
Since the user is greeting, use the greeting-responder agent to respond with a friendly joke
</commentary>
assistant: "I'm going to use the Task tool to launch the greeting-responder agent"
</example>"#,
            agent_descriptions
        )
    }

    async fn build_description(&self, workspace_root: Option<&Path>) -> String {
        let agents = self.get_enabled_agents(workspace_root).await;
        let agent_descriptions = self.format_agent_descriptions(&agents);
        self.render_description(agent_descriptions)
    }

    async fn get_enabled_agents(&self, workspace_root: Option<&Path>) -> Vec<AgentInfo> {
        let registry = get_agent_registry();
        if let Some(workspace_root) = workspace_root {
            registry.load_custom_subagents(workspace_root).await;
        }
        registry
            .get_subagents_info(workspace_root)
            .await
            .into_iter()
            .filter(|agent| agent.enabled) // Only return enabled subagents
            .collect()
    }

    async fn get_agents_types(&self, workspace_root: Option<&Path>) -> Vec<String> {
        self.get_enabled_agents(workspace_root)
            .await
            .into_iter()
            .map(|agent| agent.id)
            .collect()
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.build_description(None).await)
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self
            .build_description(context.and_then(|ctx| ctx.workspace_root()))
            .await)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform. Keep it scoped and concise. Do not include top-level Task arguments such as subagent_type inside this string. The 180-line / 16KB guideline is a soft reliability threshold, not a hard cap. For large delegations, split into multiple Task calls with clear ownership, and pass file paths, symbols, constraints, and exact questions instead of pasting large file contents."
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Required top-level agent type id. Use the exact case-sensitive id from the available_agents type attribute, for example Explore, FileFinder, CodeReview, or another listed agent."
                },
                "workspace_path": {
                    "type": "string",
                    "description": "The absolute path of the workspace for this task. If omitted, inherits the current workspace. Explore/FileFinder must provide it explicitly."
                },
                "model_id": {
                    "type": "string",
                    "description": "Optional model ID or model slot alias for this subagent task. Omit it to use the agent default."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Optional timeout for this subagent task in seconds. Use 0 or omit it to disable the timeout."
                },
                "retry": {
                    "type": "boolean",
                    "description": "DeepReview only: true when this Task call is a retry for the same reviewer role after partial_timeout or an explicit transient capacity failure in the current turn."
                },
                "retry_coverage": {
                    "type": "object",
                    "description": "DeepReview retry only: structured coverage metadata proving the retry is bounded. Required when retry=true.",
                    "properties": {
                        "source_packet_id": {
                            "type": "string",
                            "description": "The original reviewer packet_id being retried."
                        },
                        "source_status": {
                            "type": "string",
                            "enum": ["partial_timeout", "capacity_skipped"],
                            "description": "The retryable source status."
                        },
                        "capacity_reason": {
                            "type": "string",
                            "description": "Required for capacity_skipped; must be a transient capacity reason such as local_concurrency_cap, provider_rate_limit, provider_concurrency_limit, retry_after, or temporary_overload."
                        },
                        "covered_files": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Files already covered by the source attempt."
                        },
                        "retry_scope_files": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Smaller file list to retry. Every entry must belong to the source packet and must not overlap covered_files."
                        }
                    },
                    "required": [
                        "source_packet_id",
                        "source_status",
                        "covered_files",
                        "retry_scope_files"
                    ]
                }
            },
            "required": [
                "description",
                "prompt",
                "subagent_type"
            ],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, input: Option<&Value>) -> bool {
        let subagent_type = input
            .and_then(|v| v.get("subagent_type"))
            .and_then(|v| v.as_str());
        match subagent_type {
            Some(id) => get_agent_registry()
                .get_subagent_is_readonly(id)
                .unwrap_or(false),
            None => false,
        }
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let validation = InputValidator::new(input)
            .validate_required("description")
            .validate_required("prompt")
            .validate_required("subagent_type")
            .finish();
        if !validation.result {
            return validation;
        }

        if let Some(prompt) = input.get("prompt").and_then(|value| value.as_str()) {
            let line_count = prompt.lines().count();
            let byte_count = prompt.len();
            if line_count > LARGE_TASK_PROMPT_SOFT_LINE_LIMIT
                || byte_count > LARGE_TASK_PROMPT_SOFT_BYTE_LIMIT
            {
                return ValidationResult {
                    result: true,
                    message: Some(format!(
                        "Large Task prompt: {} lines, {} bytes. This is allowed when necessary, but prefer staged delegation: split large work into multiple Task calls with clear ownership, and pass file paths, symbols, constraints, and exact questions instead of large pasted context.",
                        line_count, byte_count
                    )),
                    error_code: None,
                    meta: Some(json!({
                        "large_task_prompt": true,
                        "line_count": line_count,
                        "byte_count": byte_count,
                        "soft_line_limit": LARGE_TASK_PROMPT_SOFT_LINE_LIMIT,
                        "soft_byte_limit": LARGE_TASK_PROMPT_SOFT_BYTE_LIMIT
                    })),
                };
            }
        }

        validation
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        if let Some(description) = input.get("description").and_then(|v| v.as_str()) {
            if options.verbose {
                format!("Creating task: {}", description)
            } else {
                format!("Task: {}", description)
            }
        } else {
            "Creating task".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let start_time = std::time::Instant::now();

        // description is only used for frontend display
        let description = input
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);

        let mut prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                BitFunError::tool(
                    "Required parameters: subagent_type, prompt, description. Missing prompt"
                        .to_string(),
                )
            })?
            .to_string();

        let subagent_type = input
            .get("subagent_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("Required parameters: subagent_type, prompt, description. Missing subagent_type".to_string()))?
            .to_string();
        let workspace_root = context.workspace_root();
        let all_agent_types = self.get_agents_types(workspace_root).await;
        if !all_agent_types.contains(&subagent_type) {
            return Err(BitFunError::tool(format!(
                "subagent_type {} is not valid, must be one of: {}",
                subagent_type,
                all_agent_types.join(", ")
            )));
        }

        let requested_workspace_path = input
            .get("workspace_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let model_id = match input.get("model_id") {
            Some(value) => {
                let value = value
                    .as_str()
                    .ok_or_else(|| BitFunError::tool("model_id must be a string".to_string()))?;
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_string())
            }
            None => None,
        };
        let mut timeout_seconds = match input.get("timeout_seconds") {
            Some(value) => {
                let parsed = value.as_u64().ok_or_else(|| {
                    BitFunError::tool("timeout_seconds must be a non-negative integer".to_string())
                })?;
                (parsed > 0).then_some(parsed)
            }
            None => None,
        };
        let is_retry = input.get("retry").and_then(Value::as_bool).unwrap_or(false);
        let current_workspace_path = context
            .workspace_root()
            .map(|path| path.to_string_lossy().into_owned());
        if subagent_type == "Explore" || subagent_type == "FileFinder" {
            let workspace_path = requested_workspace_path
                .as_deref()
                .or(current_workspace_path.as_deref())
                .ok_or_else(|| {
                    BitFunError::tool(
                        "workspace_path is required for Explore/FileFinder agent".to_string(),
                    )
                })?;

            if workspace_path.is_empty() {
                return Err(BitFunError::tool(
                    "workspace_path cannot be empty for Explore/FileFinder agent".to_string(),
                ));
            }

            // For remote workspaces, skip local filesystem validation — the path
            // exists on the remote server, not locally.
            if !context.is_remote() {
                let path = std::path::Path::new(&workspace_path);
                if !path.exists() {
                    return Err(BitFunError::tool(format!(
                        "workspace_path '{}' does not exist",
                        workspace_path
                    )));
                }
                if !path.is_dir() {
                    return Err(BitFunError::tool(format!(
                        "workspace_path '{}' is not a directory",
                        workspace_path
                    )));
                }
            }

            prompt.push_str(&format!(
                "\n\nThe workspace you need to explore: {workspace_path}"
            ));
        }
        let effective_workspace_path = requested_workspace_path
            .clone()
            .or(current_workspace_path)
            .ok_or_else(|| {
                BitFunError::tool(
                    "workspace_path is required when the current workspace is unavailable"
                        .to_string(),
                )
            })?;

        let session_id = if let Some(session_id) = &context.session_id {
            session_id.clone()
        } else {
            return Err(BitFunError::tool(
                "session_id is required in context".to_string(),
            ));
        };

        // Get parent tool ID (tool_call_id)
        let tool_call_id = if let Some(tool_id) = &context.tool_call_id {
            tool_id.clone()
        } else {
            return Err(BitFunError::tool(
                "tool_call_id is required in context".to_string(),
            ));
        };

        // Get parent dialog turn ID (dialog_turn_id)
        let dialog_turn_id = if let Some(turn_id) = &context.dialog_turn_id {
            turn_id.clone()
        } else {
            return Err(BitFunError::tool(
                "dialog_turn_id is required in context".to_string(),
            ));
        };
        let mut deep_review_effective_policy: Option<DeepReviewExecutionPolicy> = None;
        let mut deep_review_active_guard: Option<DeepReviewActiveReviewerGuard<'static>> = None;
        let mut deep_review_reviewer_configured_max_parallel_instances: Option<usize> = None;
        let mut deep_review_concurrency_policy: Option<DeepReviewConcurrencyPolicy> = None;
        let mut deep_review_is_optional_reviewer = false;
        let mut deep_review_retry_scope_files: Option<Vec<String>> = None;
        let mut deep_review_subagent_role: Option<DeepReviewSubagentRole> = None;

        // Get global coordinator
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;

        if context
            .agent_type
            .as_deref()
            .map(str::trim)
            .is_some_and(|agent_type| agent_type == DEEP_REVIEW_AGENT_TYPE)
        {
            let base_policy = load_default_deep_review_policy().await.map_err(|error| {
                BitFunError::tool(format!(
                    "Failed to load DeepReview execution policy: {}",
                    error
                ))
            })?;
            let mut run_manifest = context.custom_data.get("deep_review_run_manifest").cloned();
            if let Some(workspace) = context.workspace.as_ref() {
                let session_storage_path = workspace.session_storage_path();
                match coordinator
                    .get_session_manager()
                    .load_session_metadata(&session_storage_path, &session_id)
                    .await
                {
                    Ok(Some(metadata)) => {
                        if run_manifest.is_none() {
                            run_manifest = metadata.deep_review_run_manifest;
                        }
                        if let Some(run_manifest) = run_manifest.as_mut() {
                            Self::attach_deep_review_cache(
                                run_manifest,
                                metadata.deep_review_cache,
                            );
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        warn!(
                            "Failed to load DeepReview session metadata for run-manifest policy: session_id={}, error={}",
                            session_id, error
                        );
                    }
                }
            }
            let policy = if let Some(manifest) = run_manifest.as_ref() {
                base_policy.with_run_manifest_execution_policy(manifest)
            } else {
                base_policy
            };
            deep_review_effective_policy = Some(policy.clone());
            let role = policy
                .classify_subagent(&subagent_type)
                .map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            deep_review_subagent_role = Some(role);
            if let Some(gate) = run_manifest
                .as_ref()
                .and_then(DeepReviewRunManifestGate::from_value)
            {
                gate.ensure_active(&subagent_type).map_err(|violation| {
                    BitFunError::tool(format!(
                        "DeepReview Task policy violation: {}",
                        violation.to_tool_error_message()
                    ))
                })?;
            }
            if is_retry && role == DeepReviewSubagentRole::Reviewer {
                deep_review_retry_scope_files = Some(
                    Self::ensure_deep_review_retry_coverage(
                        input,
                        &subagent_type,
                        run_manifest.as_ref(),
                    )
                    .map_err(|violation| {
                        BitFunError::tool(format!(
                            "DeepReview Task policy violation: {}",
                            violation.to_tool_error_message()
                        ))
                    })?,
                );
            }
            let is_readonly = get_agent_registry()
                .get_subagent_is_readonly(&subagent_type)
                .unwrap_or(false);
            if !is_readonly {
                return Err(BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    json!({
                        "code": "deep_review_subagent_not_readonly",
                        "message": format!(
                            "DeepReview review-phase subagent '{}' must be read-only",
                            subagent_type
                        )
                    })
                )));
            }
            let is_review = get_agent_registry()
                .get_subagent_is_review(&subagent_type)
                .unwrap_or(false);
            if !is_review {
                return Err(BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    json!({
                        "code": "deep_review_subagent_not_review",
                        "message": format!(
                            "DeepReview review-phase subagent '{}' must be marked for review",
                            subagent_type
                        )
                    })
                )));
            }
            timeout_seconds = policy.effective_timeout_seconds(role, timeout_seconds);

            // Check incremental review cache before queueing. A cache hit does
            // not consume runtime reviewer capacity or reviewer timeout.
            if role == DeepReviewSubagentRole::Reviewer && !is_retry {
                if let Some(cache_value) =
                    run_manifest.as_ref().and_then(|m| m.get("deepReviewCache"))
                {
                    let cache = DeepReviewIncrementalCache::from_value(cache_value);
                    if cache.matches_manifest(run_manifest.as_ref().unwrap_or(&Value::Null)) {
                        if let Some(packet_id) = Self::deep_review_packet_id_for_cache(
                            &subagent_type,
                            description.as_deref(),
                            run_manifest.as_ref(),
                        ) {
                            if let Some(cached_output) = cache.get_packet(&packet_id) {
                                let cached_result = format!(
                                    "Subagent '{}' result (from incremental review cache):\n<result source=\"cache\">\n{}\n</result>",
                                    subagent_type, cached_output
                                );
                                return Ok(vec![ToolResult::ok(
                                    json!({ "cached": true, "packet_id": packet_id }),
                                    Some(cached_result),
                                )]);
                            }
                        }
                    }
                }
            }

            // Enforce dynamic concurrency policy from the run manifest.
            let conc_policy = policy
                .concurrency_policy_from_manifest(run_manifest.as_ref().unwrap_or(&Value::Null));
            deep_review_concurrency_policy = Some(conc_policy.clone());
            match role {
                DeepReviewSubagentRole::Reviewer => {
                    deep_review_reviewer_configured_max_parallel_instances =
                        Some(conc_policy.max_parallel_instances);
                    let effective_parallel_instances = deep_review_effective_parallel_instances(
                        &dialog_turn_id,
                        conc_policy.max_parallel_instances,
                    );
                    let is_optional_reviewer = policy
                        .extra_subagent_ids
                        .iter()
                        .any(|id| id == &subagent_type);
                    deep_review_is_optional_reviewer = is_optional_reviewer;
                    if let Some(guard) = try_begin_deep_review_active_reviewer(
                        &dialog_turn_id,
                        effective_parallel_instances,
                    ) {
                        deep_review_active_guard = Some(guard);
                    } else {
                        match Self::wait_for_deep_review_reviewer_capacity(
                            &session_id,
                            &dialog_turn_id,
                            &tool_call_id,
                            &subagent_type,
                            &conc_policy,
                            is_optional_reviewer,
                        )
                        .await?
                        {
                            DeepReviewQueueWaitOutcome::Ready { guard } => {
                                deep_review_active_guard = Some(guard);
                            }
                            DeepReviewQueueWaitOutcome::Skipped {
                                queue_elapsed_ms,
                                skip_reason,
                            } => {
                                let queue_skip_reason = match skip_reason {
                                    DeepReviewQueueWaitSkipReason::QueueExpired => "queue_expired",
                                    DeepReviewQueueWaitSkipReason::UserCancelled => {
                                        "user_cancelled"
                                    }
                                    DeepReviewQueueWaitSkipReason::OptionalSkipped => {
                                        "optional_skipped"
                                    }
                                };
                                let assistant_message = match skip_reason {
                                    DeepReviewQueueWaitSkipReason::QueueExpired => format!(
                                        "Subagent '{}' was skipped because the DeepReview capacity queue reached its maximum wait ({}s).\n<queue_result status=\"capacity_skipped\" reason=\"local_concurrency_cap\" queue_elapsed_ms=\"{}\" />",
                                        subagent_type,
                                        conc_policy.max_queue_wait_seconds,
                                        queue_elapsed_ms
                                    ),
                                    DeepReviewQueueWaitSkipReason::UserCancelled => format!(
                                        "Subagent '{}' was skipped because the DeepReview capacity queue was cancelled by the user.\n<queue_result status=\"capacity_skipped\" reason=\"user_cancelled\" queue_elapsed_ms=\"{}\" />",
                                        subagent_type, queue_elapsed_ms
                                    ),
                                    DeepReviewQueueWaitSkipReason::OptionalSkipped => format!(
                                        "Subagent '{}' was skipped because optional DeepReview queued reviewers were skipped by the user.\n<queue_result status=\"capacity_skipped\" reason=\"optional_skipped\" queue_elapsed_ms=\"{}\" />",
                                        subagent_type, queue_elapsed_ms
                                    ),
                                };
                                return Ok(vec![ToolResult::Result {
                                    data: json!({
                                        "duration": start_time.elapsed().as_millis(),
                                        "status": "capacity_skipped",
                                        "queue_elapsed_ms": queue_elapsed_ms,
                                        "max_queue_wait_seconds": conc_policy.max_queue_wait_seconds,
                                        "queue_skip_reason": queue_skip_reason,
                                        "effective_parallel_instances": deep_review_effective_concurrency_snapshot(
                                            &dialog_turn_id,
                                            conc_policy.max_parallel_instances,
                                        ).effective_parallel_instances
                                    }),
                                    result_for_assistant: Some(assistant_message),
                                    image_attachments: None,
                                }]);
                            }
                        }
                    }
                }
                DeepReviewSubagentRole::Judge => {
                    let active_reviewers = deep_review_active_reviewer_count(&dialog_turn_id);
                    let judge_pending = deep_review_has_judge_been_launched(&dialog_turn_id);
                    conc_policy
                        .check_launch_allowed(active_reviewers, role, judge_pending)
                        .map_err(|violation| {
                            BitFunError::tool(format!(
                                "DeepReview concurrency policy violation: {}",
                                violation.to_tool_error_message()
                            ))
                        })?;
                }
            }
            record_deep_review_task_budget(
                &dialog_turn_id,
                &policy,
                role,
                &subagent_type,
                is_retry,
            )
            .map_err(|violation| {
                BitFunError::tool(format!(
                    "DeepReview Task policy violation: {}",
                    violation.to_tool_error_message()
                ))
            })?;
        }

        if let Some(retry_scope_files) = deep_review_retry_scope_files.as_ref() {
            prompt = Self::prompt_with_deep_review_retry_scope(&prompt, retry_scope_files);
        }

        let parent_info = SubagentParentInfo {
            tool_call_id: tool_call_id.clone(),
            session_id: session_id.clone(),
            dialog_turn_id: dialog_turn_id.clone(),
        };
        let subagent_context = deep_review_subagent_role.map(|role| {
            let mut values = HashMap::new();
            values.insert(
                "deep_review_subagent_role".to_string(),
                match role {
                    DeepReviewSubagentRole::Reviewer => "reviewer",
                    DeepReviewSubagentRole::Judge => "judge",
                }
                .to_string(),
            );
            values.insert(
                "deep_review_subagent_type".to_string(),
                subagent_type.clone(),
            );
            values
        });
        let result = match coordinator
            .execute_subagent(
                subagent_type.clone(),
                prompt,
                parent_info,
                Some(effective_workspace_path.clone()),
                subagent_context,
                context.cancellation_token.as_ref(),
                model_id,
                timeout_seconds,
            )
            .await
        {
            Ok(result) => result,
            Err(error) => {
                if matches!(
                    deep_review_subagent_role,
                    Some(DeepReviewSubagentRole::Reviewer)
                ) {
                    if let (Some(reason), Some(conc_policy)) = (
                        Self::deep_review_capacity_reason_for_provider_error(&error),
                        deep_review_concurrency_policy.as_ref(),
                    ) {
                        drop(deep_review_active_guard.take());
                        let (data, assistant_message) =
                            Self::deep_review_capacity_skip_result_for_provider_reason(
                                reason,
                                &dialog_turn_id,
                                &subagent_type,
                                conc_policy,
                                start_time.elapsed().as_millis(),
                            );
                        let effective_parallel_instances = data
                            .get("effective_parallel_instances")
                            .and_then(Value::as_u64)
                            .and_then(|value| usize::try_from(value).ok());
                        Self::emit_deep_review_queue_state(
                            &session_id,
                            &dialog_turn_id,
                            &tool_call_id,
                            &subagent_type,
                            DeepReviewQueueStatus::CapacitySkipped,
                            Some(reason),
                            0,
                            deep_review_active_reviewer_count(&dialog_turn_id),
                            deep_review_is_optional_reviewer.then_some(1),
                            effective_parallel_instances,
                            0,
                            conc_policy.max_queue_wait_seconds,
                        )
                        .await;
                        return Ok(vec![ToolResult::Result {
                            data,
                            result_for_assistant: Some(assistant_message),
                            image_attachments: None,
                        }]);
                    }
                }
                return Err(error);
            }
        };
        if !result.is_partial_timeout() {
            if let Some(configured_max_parallel_instances) =
                deep_review_reviewer_configured_max_parallel_instances
            {
                record_deep_review_effective_concurrency_success(
                    &dialog_turn_id,
                    configured_max_parallel_instances,
                );
            }
        }
        drop(deep_review_active_guard);

        let duration = start_time.elapsed().as_millis();
        let status = if result.is_partial_timeout() {
            "partial_timeout"
        } else {
            "completed"
        };

        // Build retry hint for deep review reviewer timeouts.
        let retry_hint = if result.is_partial_timeout() && !is_retry {
            let retries_used = crate::agentic::deep_review_policy::deep_review_retries_used(
                &dialog_turn_id,
                &subagent_type,
            );
            let max_retries = Self::deep_review_retry_guidance_max_retries(
                deep_review_effective_policy.as_ref(),
                &dialog_turn_id,
            );
            if max_retries > 0 && retries_used < max_retries {
                format!(
                    "\n\n<retry_guidance>This reviewer timed out. You may retry with 'retry: true' only if you can provide retry_coverage with source_packet_id, source_status='partial_timeout', covered_files, and a smaller retry_scope_files list. Retries used: {}/{}.</retry_guidance>",
                    retries_used, max_retries
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let result_for_assistant = if result.is_partial_timeout() {
            format!(
                "Subagent '{}' timed out with partial result:\n<partial_result status=\"partial_timeout\">\n{}\n</partial_result>{}",
                subagent_type, result.text, retry_hint
            )
        } else {
            format!(
                "Subagent '{}' completed successfully with result:\n<result>\n{}\n</result>",
                subagent_type, result.text
            )
        };
        let mut data = json!({
            "duration": duration,
            "status": status
        });
        if result.is_partial_timeout() {
            data["partial_output"] = json!(result.text);
            if let Some(reason) = result.reason.as_deref() {
                data["reason"] = json!(reason);
            }
            if let Some(event_id) = result.ledger_event_id() {
                data["ledger_event_id"] = json!(event_id);
            }
        }

        Ok(vec![ToolResult::Result {
            data,
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::TaskTool;
    use crate::agentic::deep_review_policy::{
        DeepReviewBudgetTracker, DeepReviewExecutionPolicy, DeepReviewSubagentRole,
    };
    use crate::agentic::tools::framework::Tool;
    use serde_json::json;

    #[test]
    fn task_schema_accepts_optional_model_id() {
        let schema = TaskTool::new().input_schema();

        assert_eq!(schema["properties"]["model_id"]["type"], "string");
        assert!(!schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some("model_id")));
    }

    #[test]
    fn task_schema_requires_top_level_subagent_type_and_rejects_extra_fields() {
        let schema = TaskTool::new().input_schema();

        assert_eq!(schema["additionalProperties"], false);
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some("subagent_type")));
        assert!(schema["properties"]["subagent_type"]["description"]
            .as_str()
            .unwrap()
            .contains("top-level"));
        assert!(schema["properties"]["prompt"]["description"]
            .as_str()
            .unwrap()
            .contains("Do not include top-level Task arguments"));
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
        };
        // 0 active → allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Reviewer, false)
            .is_ok());
        // 1 active → allowed
        assert!(policy
            .check_launch_allowed(1, DeepReviewSubagentRole::Reviewer, false)
            .is_ok());
        // 2 active (at cap) → blocked
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
    async fn deep_review_capacity_queue_skips_after_max_wait() {
        use crate::agentic::deep_review_policy::{
            deep_review_capacity_skip_count, deep_review_concurrency_cap_rejection_count,
            deep_review_effective_parallel_instances, try_begin_deep_review_active_reviewer,
            DeepReviewConcurrencyPolicy,
        };

        let _occupied_a = try_begin_deep_review_active_reviewer("turn-queue-skip", 2)
            .expect("precondition should occupy first reviewer capacity");
        let _occupied_b = try_begin_deep_review_active_reviewer("turn-queue-skip", 2)
            .expect("precondition should occupy second reviewer capacity");
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 2,
            stagger_seconds: 0,
            max_queue_wait_seconds: 0,
            batch_extras_separately: true,
        };

        let outcome = TaskTool::wait_for_deep_review_reviewer_capacity(
            "session-queue-skip",
            "turn-queue-skip",
            "tool-queue-skip",
            "ReviewSecurity",
            &policy,
            false,
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
                panic!("occupied capacity should skip with maxQueueWaitSeconds=0");
            }
        }
        assert_eq!(deep_review_capacity_skip_count("turn-queue-skip"), 1);
        assert_eq!(
            deep_review_concurrency_cap_rejection_count("turn-queue-skip"),
            0
        );
        assert_eq!(
            deep_review_effective_parallel_instances("turn-queue-skip", 2),
            1
        );
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
        };

        let outcome = TaskTool::wait_for_deep_review_reviewer_capacity(
            "session-queue-cancel",
            turn_id,
            tool_id,
            "ReviewSecurity",
            &policy,
            false,
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
    async fn deep_review_capacity_queue_pause_does_not_expire_until_continued() {
        use crate::agentic::deep_review_policy::{
            apply_deep_review_queue_control, try_begin_deep_review_active_reviewer,
            DeepReviewConcurrencyPolicy, DeepReviewQueueControlAction,
        };

        let turn_id = "turn-queue-pause";
        let tool_id = "tool-queue-pause";
        let _occupied = try_begin_deep_review_active_reviewer(turn_id, 1)
            .expect("precondition should occupy reviewer capacity");
        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Pause);
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 1,
            stagger_seconds: 0,
            max_queue_wait_seconds: 0,
            batch_extras_separately: true,
        };
        let turn_id_owned = turn_id.to_string();
        let tool_id_owned = tool_id.to_string();

        let handle = tokio::spawn(async move {
            TaskTool::wait_for_deep_review_reviewer_capacity(
                "session-queue-pause",
                &turn_id_owned,
                &tool_id_owned,
                "ReviewSecurity",
                &policy,
                false,
            )
            .await
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
        assert!(
            !handle.is_finished(),
            "paused queue wait should not expire while user pause is active"
        );

        apply_deep_review_queue_control(turn_id, tool_id, DeepReviewQueueControlAction::Continue);
        let outcome = tokio::time::timeout(tokio::time::Duration::from_millis(500), handle)
            .await
            .expect("continued queue wait should finish")
            .expect("spawned wait should not panic")
            .expect("queue wait should resolve");
        match outcome {
            super::DeepReviewQueueWaitOutcome::Skipped {
                queue_elapsed_ms, ..
            } => {
                assert!(queue_elapsed_ms < 100);
            }
            super::DeepReviewQueueWaitOutcome::Ready { .. } => {
                panic!("occupied capacity should skip after pause is continued");
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
        apply_deep_review_queue_control(
            turn_id,
            tool_id,
            DeepReviewQueueControlAction::SkipOptional,
        );
        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 1,
            stagger_seconds: 0,
            max_queue_wait_seconds: 60,
            batch_extras_separately: true,
        };

        let outcome = TaskTool::wait_for_deep_review_reviewer_capacity(
            "session-queue-skip-optional",
            turn_id,
            tool_id,
            "ReviewCustom",
            &policy,
            true,
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
        // 1 active reviewer → judge blocked
        assert!(policy
            .check_launch_allowed(1, DeepReviewSubagentRole::Judge, false)
            .is_err());
        // 0 active reviewers, no judge pending → judge allowed
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Judge, false)
            .is_ok());
        // 0 active reviewers, judge already pending → blocked
        assert!(policy
            .check_launch_allowed(0, DeepReviewSubagentRole::Judge, true)
            .is_err());
    }

    #[test]
    fn deep_review_incremental_cache_hit_returns_cached_result() {
        use crate::agentic::deep_review_policy::DeepReviewIncrementalCache;

        let mut cache = DeepReviewIncrementalCache::new("fp-test-123");
        cache.store_packet("ReviewSecurity", "Found 2 security issues");

        // Cache hit
        let result = cache.get_packet("ReviewSecurity");
        assert_eq!(result, Some("Found 2 security issues"));

        // Cache miss
        assert_eq!(cache.get_packet("ReviewPerformance"), None);
    }

    #[test]
    fn deep_review_incremental_cache_fingerprint_mismatch_skips() {
        use crate::agentic::deep_review_policy::DeepReviewIncrementalCache;

        let cache = DeepReviewIncrementalCache::new("fp-old");
        let manifest = serde_json::json!({
            "incrementalReviewCache": {
                "fingerprint": "fp-new"
            }
        });
        // Fingerprint mismatch → cache should not match
        assert!(!cache.matches_manifest(&manifest));
    }

    #[test]
    fn deep_review_cache_packet_id_prefers_task_description_packet() {
        let manifest = serde_json::json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity"
                },
                {
                    "packetId": "reviewer:ReviewSecurity:group-2-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity"
                }
            ]
        });

        assert_eq!(
            TaskTool::deep_review_packet_id_for_cache(
                "ReviewSecurity",
                Some("Security review [packet reviewer:ReviewSecurity:group-2-of-2]"),
                Some(&manifest),
            ),
            Some("reviewer:ReviewSecurity:group-2-of-2".to_string())
        );
    }

    #[test]
    fn deep_review_cache_packet_id_uses_unique_manifest_packet() {
        let manifest = serde_json::json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewBusinessLogic",
                    "phase": "reviewer",
                    "subagentId": "ReviewBusinessLogic"
                }
            ]
        });

        assert_eq!(
            TaskTool::deep_review_packet_id_for_cache(
                "ReviewBusinessLogic",
                Some("Logic review"),
                Some(&manifest),
            ),
            Some("reviewer:ReviewBusinessLogic".to_string())
        );
    }

    #[test]
    fn deep_review_cache_packet_id_does_not_guess_split_packets() {
        let manifest = serde_json::json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewPerformance:group-1-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewPerformance"
                },
                {
                    "packetId": "reviewer:ReviewPerformance:group-2-of-2",
                    "phase": "reviewer",
                    "subagentId": "ReviewPerformance"
                }
            ]
        });

        assert_eq!(
            TaskTool::deep_review_packet_id_for_cache(
                "ReviewPerformance",
                Some("Performance review"),
                Some(&manifest),
            ),
            None
        );
    }

    #[test]
    fn deep_review_cache_packet_id_ignores_description_for_other_subagent() {
        let manifest = serde_json::json!({
            "workPackets": [
                {
                    "packetId": "reviewer:ReviewSecurity:group-1-of-1",
                    "phase": "reviewer",
                    "subagentId": "ReviewSecurity"
                }
            ]
        });

        assert_eq!(
            TaskTool::deep_review_packet_id_for_cache(
                "ReviewPerformance",
                Some("Performance review [packet reviewer:ReviewSecurity:group-1-of-1]"),
                Some(&manifest),
            ),
            None
        );
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
        let policy =
            DeepReviewExecutionPolicy::default().with_run_manifest_execution_policy(&manifest);

        assert_eq!(
            TaskTool::deep_review_retry_guidance_max_retries(Some(&policy), "nonexistent-turn"),
            2
        );
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
                            "src/crates/core/src/auth.rs",
                            "src/crates/core/src/token.rs"
                        ]
                    }
                }
            ]
        });
        let input = json!({
            "retry": true
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
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
                            "src/crates/core/src/auth.rs",
                            "src/crates/core/src/token.rs"
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
                    "src/crates/core/src/auth.rs"
                ],
                "retry_scope_files": [
                    "src/crates/core/src/auth.rs",
                    "src/crates/core/src/token.rs"
                ]
            }
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
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
                            "src/crates/core/src/auth.rs",
                            "src/crates/core/src/token.rs"
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
                    "src/crates/core/src/auth.rs"
                ],
                "retry_scope_files": [
                    "src/crates/core/src/token.rs"
                ]
            }
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
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
                            "src/crates/core/src/auth.rs",
                            "src/crates/core/src/token.rs"
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
                    "src/crates/core/src/token.rs"
                ]
            }
        });

        let violation =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
                .expect_err("non-queueable capacity failures must fail fast");

        assert_eq!(violation.code, "deep_review_retry_non_retryable_status");
    }

    #[test]
    fn deep_review_provider_capacity_error_builds_capacity_skipped_payload_and_lowers_effective_cap(
    ) {
        use crate::agentic::deep_review_policy::{
            deep_review_effective_concurrency_snapshot, DeepReviewConcurrencyPolicy,
        };
        use crate::util::BitFunError;

        let policy = DeepReviewConcurrencyPolicy {
            max_parallel_instances: 3,
            stagger_seconds: 0,
            max_queue_wait_seconds: 30,
            batch_extras_separately: true,
        };
        let turn_id = "turn-provider-capacity-skip";
        let reason = TaskTool::deep_review_capacity_reason_for_provider_error(&BitFunError::ai(
            "Provider error: provider=openai, code=429, message=rate limit exceeded",
        ))
        .expect("provider rate limit should surface as capacity_skipped");
        let (data, assistant_message) =
            TaskTool::deep_review_capacity_skip_result_for_provider_reason(
                reason,
                turn_id,
                "ReviewSecurity",
                &policy,
                42,
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

        let reason = TaskTool::deep_review_capacity_reason_for_provider_error(&BitFunError::ai(
            "Provider error: provider=glm, code=1113, message=insufficient quota",
        ));

        assert!(
            reason.is_none(),
            "quota errors should remain fail-fast instead of entering capacity queue flow"
        );
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
                            "src/crates/core/src/auth.rs",
                            "src/crates/core/src/token.rs"
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
                    "src/crates/core/src/auth.rs"
                ],
                "retry_scope_files": [
                    "src/crates/core/src/token.rs"
                ]
            }
        });

        let retry_scope =
            TaskTool::ensure_deep_review_retry_coverage(&input, "ReviewSecurity", Some(&manifest))
                .expect("reduced retry scope should be accepted");

        assert_eq!(retry_scope, vec!["src/crates/core/src/token.rs"]);
    }

    #[test]
    fn deep_review_retry_scope_prompt_prepend_bounds_review_files() {
        let prompt = TaskTool::prompt_with_deep_review_retry_scope(
            "Continue the security review.",
            &["src/crates/core/src/token.rs".to_string()],
        );

        assert!(prompt.starts_with("<deep_review_retry_scope>"));
        assert!(prompt.contains("Review only the following retry_scope_files"));
        assert!(prompt.contains("- src/crates/core/src/token.rs"));
        assert!(prompt.ends_with("Continue the security review."));
    }
}
