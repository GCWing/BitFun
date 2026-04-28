//! Code review result submission tool
//!
//! Used to get structured code review results.

use crate::agentic::agents::get_agent_registry;
use crate::agentic::context_profile::ContextProfilePolicy;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::core::CompressionContract;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::get_app_language_code;
use crate::service::i18n::code_review_copy_for_language;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use log::warn;
use serde_json::{json, Value};

/// Code review tool definition
pub struct CodeReviewTool;

impl CodeReviewTool {
    pub fn new() -> Self {
        Self
    }

    pub fn name_str() -> &'static str {
        "submit_code_review"
    }

    /// Sync schema fallback (e.g. tests); prefers zh-CN wording. For model calls use [`input_schema_for_model`].
    pub fn input_schema_value() -> Value {
        Self::input_schema_value_for_language("zh-CN")
    }

    pub fn description_for_language(lang_code: &str) -> String {
        code_review_copy_for_language(lang_code)
            .description
            .to_string()
    }

    pub fn input_schema_value_for_language(lang_code: &str) -> Value {
        Self::input_schema_value_for_language_with_mode(lang_code, false)
    }

    fn input_schema_value_for_language_with_mode(
        lang_code: &str,
        require_deep_fields: bool,
    ) -> Value {
        let copy = code_review_copy_for_language(lang_code);
        let (
            scope_desc,
            reviewer_summary_desc,
            source_reviewer_desc,
            validation_note_desc,
            plan_desc,
        ) = match lang_code {
            "en-US" => (
                "Human-readable review scope (optional, in English)",
                "Reviewer summary (in English)",
                "Reviewer source / role (optional, in English)",
                "Validation or triage note (optional, in English)",
                "Concrete remediation / follow-up plan items (in English)",
            ),
            "zh-TW" => (
                "Human-readable review scope (optional, in Traditional Chinese)",
                "Reviewer summary (in Traditional Chinese)",
                "Reviewer source / role (optional, in Traditional Chinese)",
                "Validation or triage note (optional, in Traditional Chinese)",
                "Concrete remediation / follow-up plan items (in Traditional Chinese)",
            ),
            _ => (
                "Human-readable review scope (optional, in Simplified Chinese)",
                "Reviewer summary (in Simplified Chinese)",
                "Reviewer source / role (optional, in Simplified Chinese)",
                "Validation or triage note (optional, in Simplified Chinese)",
                "Concrete remediation / follow-up plan items (in Simplified Chinese)",
            ),
        };
        let mut required = vec!["summary", "issues", "positive_points"];
        if require_deep_fields {
            required.extend([
                "review_mode",
                "review_scope",
                "reviewers",
                "remediation_plan",
            ]);
        }

        json!({
            "type": "object",
            "properties": {
                "schema_version": {
                    "type": "integer",
                    "description": "Schema version for forward compatibility",
                    "default": 1
                },
                "summary": {
                    "type": "object",
                    "description": "Review summary",
                    "properties": {
                        "overall_assessment": {
                            "type": "string",
                            "description": copy.overall_assessment
                        },
                        "risk_level": {
                            "type": "string",
                            "enum": ["low", "medium", "high", "critical"],
                            "description": "Risk level"
                        },
                        "recommended_action": {
                            "type": "string",
                            "enum": ["approve", "approve_with_suggestions", "request_changes", "block"],
                            "description": "Recommended action"
                        },
                        "confidence_note": {
                            "type": "string",
                            "description": copy.confidence_note
                        }
                    },
                    "required": ["overall_assessment", "risk_level", "recommended_action"]
                },
                "issues": {
                    "type": "array",
                    "description": "List of issues found",
                    "items": {
                        "type": "object",
                        "properties": {
                            "severity": {
                                "type": "string",
                                "enum": ["critical", "high", "medium", "low", "info"],
                                "description": "Severity level"
                            },
                            "certainty": {
                                "type": "string",
                                "enum": ["confirmed", "likely", "possible"],
                                "description": "Certainty level"
                            },
                            "category": {
                                "type": "string",
                                "description": "Issue category (e.g., security, logic correctness, performance, etc.)"
                            },
                            "file": {
                                "type": "string",
                                "description": "File path"
                            },
                            "line": {
                                "type": ["integer", "null"],
                                "description": "Line number (null if uncertain)"
                            },
                            "title": {
                                "type": "string",
                                "description": copy.issue_title
                            },
                            "description": {
                                "type": "string",
                                "description": copy.issue_description
                            },
                            "suggestion": {
                                "type": ["string", "null"],
                                "description": copy.issue_suggestion
                            },
                            "source_reviewer": {
                                "type": "string",
                                "description": source_reviewer_desc
                            },
                            "validation_note": {
                                "type": "string",
                                "description": validation_note_desc
                            }
                        },
                        "required": ["severity", "certainty", "category", "file", "title", "description"]
                    }
                },
                "positive_points": {
                    "type": "array",
                    "description": copy.positive_points,
                    "items": {
                        "type": "string"
                    }
                },
                "review_mode": {
                    "type": "string",
                    "enum": ["standard", "deep"],
                    "description": "Review mode"
                },
                "review_scope": {
                    "type": "string",
                    "description": scope_desc
                },
                "reviewers": {
                    "type": "array",
                    "description": "Reviewer summaries",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Reviewer display name"
                            },
                            "specialty": {
                                "type": "string",
                                "description": "Reviewer specialty / role"
                            },
                            "status": {
                                "type": "string",
                                "description": "Reviewer result status"
                            },
                            "summary": {
                                "type": "string",
                                "description": reviewer_summary_desc
                            },
                            "partial_output": {
                                "type": "string",
                                "description": "Partial reviewer output captured before timeout or cancellation"
                            },
                            "packet_id": {
                                "type": "string",
                                "description": "Deep Review work packet id associated with this reviewer output"
                            },
                            "packet_status_source": {
                                "type": "string",
                                "enum": ["reported", "inferred", "missing"],
                                "description": "Whether packet_id/status was reported by the reviewer, inferred from scheduling metadata, or missing"
                            },
                            "issue_count": {
                                "type": "integer",
                                "description": "Validated issue count for this reviewer"
                            }
                        },
                        "required": ["name", "specialty", "status", "summary"],
                        "additionalProperties": false
                    }
                },
                "remediation_plan": {
                    "type": "array",
                    "description": plan_desc,
                    "items": {
                        "type": "string"
                    }
                },
                "report_sections": {
                    "type": "object",
                    "description": "Optional structured sections for richer review report presentation",
                    "properties": {
                        "executive_summary": {
                            "type": "array",
                            "description": "Short user-facing conclusion bullets",
                            "items": {
                                "type": "string"
                            }
                        },
                        "remediation_groups": {
                            "type": "object",
                            "description": "Grouped remediation and follow-up plan items",
                            "properties": {
                                "must_fix": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "should_improve": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "needs_decision": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "verification": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            },
                            "additionalProperties": false
                        },
                        "strength_groups": {
                            "type": "object",
                            "description": "Grouped positive observations",
                            "properties": {
                                "architecture": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "maintainability": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "tests": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "security": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "performance": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "user_experience": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "other": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            },
                            "additionalProperties": false
                        },
                        "coverage_notes": {
                            "type": "array",
                            "description": "Review coverage, confidence, timeout, cancellation, or manual follow-up notes",
                            "items": {
                                "type": "string"
                            }
                        }
                    },
                    "additionalProperties": false
                },
                "reliability_signals": {
                    "type": "array",
                    "description": "Structured reliability/status signals for Deep Review report UI and export",
                    "items": {
                        "type": "object",
                        "properties": {
                            "kind": {
                                "type": "string",
                                "enum": [
                                    "context_pressure",
                                    "compression_preserved",
                                    "partial_reviewer",
                                    "user_decision"
                                ],
                                "description": "Reliability signal category"
                            },
                            "severity": {
                                "type": "string",
                                "enum": ["info", "warning", "action"],
                                "description": "User-facing severity of this signal"
                            },
                            "count": {
                                "type": "integer",
                                "minimum": 0,
                                "description": "Optional affected item count"
                            },
                            "source": {
                                "type": "string",
                                "enum": ["runtime", "manifest", "report", "inferred"],
                                "description": "Where this reliability signal came from"
                            },
                            "detail": {
                                "type": "string",
                                "description": "Short user-facing detail for this signal"
                            }
                        },
                        "required": ["kind", "severity"],
                        "additionalProperties": false
                    }
                },
                "schema_version": {
                    "type": "integer",
                    "description": "Schema version for forward compatibility",
                    "minimum": 1
                }
            },
            "required": required,
            "additionalProperties": false
        })
    }

    fn is_deep_review_context(context: Option<&ToolUseContext>) -> bool {
        context
            .and_then(|context| context.agent_type.as_deref())
            .map(str::trim)
            .is_some_and(|agent_type| agent_type == "DeepReview")
    }

    fn normalized_non_empty_string(value: Option<&Value>) -> Option<String> {
        value
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    fn packet_string_field<'a>(packet: &'a Value, keys: &[&str]) -> Option<&'a str> {
        keys.iter()
            .find_map(|key| packet.get(*key).and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    fn reviewer_match_tokens(reviewer: &Value) -> Vec<String> {
        ["name", "specialty"]
            .iter()
            .filter_map(|key| Self::normalized_non_empty_string(reviewer.get(*key)))
            .map(|value| value.to_ascii_lowercase())
            .collect()
    }

    fn packet_match_tokens(packet: &Value) -> Vec<String> {
        [
            &["subagentId", "subagent_id", "subagent_type"][..],
            &["displayName", "display_name"][..],
            &["roleName", "role"][..],
        ]
        .iter()
        .filter_map(|keys| Self::packet_string_field(packet, keys))
        .map(|value| value.to_ascii_lowercase())
        .collect()
    }

    fn infer_unique_packet_id_for_reviewer(
        reviewer: &Value,
        run_manifest: Option<&Value>,
    ) -> Option<String> {
        let reviewer_tokens = Self::reviewer_match_tokens(reviewer);
        if reviewer_tokens.is_empty() {
            return None;
        }

        let manifest = run_manifest?;
        let packets = manifest
            .get("workPackets")
            .or_else(|| manifest.get("work_packets"))?
            .as_array()?;
        let mut matches = packets.iter().filter_map(|packet| {
            let packet_id = Self::packet_string_field(packet, &["packetId", "packet_id"])?;
            let packet_tokens = Self::packet_match_tokens(packet);
            let matched = packet_tokens
                .iter()
                .any(|packet_token| reviewer_tokens.iter().any(|token| token == packet_token));
            matched.then(|| packet_id.to_string())
        });
        let first = matches.next()?;
        if matches.next().is_some() {
            None
        } else {
            Some(first)
        }
    }

    fn fill_deep_review_packet_metadata(input: &mut Value, run_manifest: Option<&Value>) {
        let Some(reviewers) = input.get_mut("reviewers").and_then(Value::as_array_mut) else {
            return;
        };

        for reviewer in reviewers {
            let packet_id = Self::normalized_non_empty_string(reviewer.get("packet_id"));
            let packet_status_source =
                Self::normalized_non_empty_string(reviewer.get("packet_status_source"));
            let inferred_packet_id = if packet_id.is_none() {
                Self::infer_unique_packet_id_for_reviewer(reviewer, run_manifest)
            } else {
                None
            };

            let Some(object) = reviewer.as_object_mut() else {
                continue;
            };

            if packet_id.is_some() {
                if packet_status_source.is_none() {
                    object.insert("packet_status_source".to_string(), json!("reported"));
                }
            } else if let Some(inferred_packet_id) = inferred_packet_id {
                object.insert("packet_id".to_string(), json!(inferred_packet_id));
                object.insert("packet_status_source".to_string(), json!("inferred"));
            } else if packet_status_source.is_none() {
                object.insert("packet_status_source".to_string(), json!("missing"));
            }
        }
    }

    fn value_for_any_key<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
        keys.iter().find_map(|key| value.get(*key))
    }

    fn bool_for_any_key(value: &Value, keys: &[&str]) -> bool {
        Self::value_for_any_key(value, keys)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn u64_for_any_key(value: &Value, keys: &[&str]) -> Option<u64> {
        Self::value_for_any_key(value, keys).and_then(Value::as_u64)
    }

    fn has_non_empty_array_for_any_key(value: &Value, keys: &[&str]) -> bool {
        Self::value_for_any_key(value, keys)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    }

    fn count_partial_reviewers(input: &Value) -> usize {
        input
            .get("reviewers")
            .and_then(Value::as_array)
            .map(|reviewers| {
                reviewers
                    .iter()
                    .filter(|reviewer| {
                        let status = reviewer
                            .get("status")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .unwrap_or_default();
                        let has_partial_output = reviewer
                            .get("partial_output")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .is_some_and(|output| !output.is_empty());
                        status == "partial_timeout"
                            || (matches!(status, "timed_out" | "cancelled_by_user")
                                && has_partial_output)
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    fn count_decision_items(input: &Value) -> usize {
        let needs_decision_count = input
            .pointer("/report_sections/remediation_groups/needs_decision")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .count()
            })
            .unwrap_or(0);
        if needs_decision_count > 0 {
            return needs_decision_count;
        }

        let recommended_action = input
            .pointer("/summary/recommended_action")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        usize::from(recommended_action == "block")
    }

    fn has_reliability_signal(input: &Value, kind: &str) -> bool {
        input
            .get("reliability_signals")
            .and_then(Value::as_array)
            .is_some_and(|signals| {
                signals.iter().any(|signal| {
                    signal
                        .get("kind")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value == kind)
                })
            })
    }

    fn push_reliability_signal_if_missing(input: &mut Value, signal: Value) {
        let Some(kind) = signal.get("kind").and_then(Value::as_str) else {
            return;
        };
        if Self::has_reliability_signal(input, kind) {
            return;
        }
        if !input
            .get("reliability_signals")
            .is_some_and(Value::is_array)
        {
            input["reliability_signals"] = json!([]);
        }
        if let Some(signals) = input
            .get_mut("reliability_signals")
            .and_then(Value::as_array_mut)
        {
            signals.push(signal);
        }
    }

    fn compression_contract_for_context(context: &ToolUseContext) -> Option<CompressionContract> {
        let session_id = context.session_id.as_deref()?;
        let coordinator = get_global_coordinator()?;
        let session = coordinator.get_session_manager().get_session(session_id)?;
        let agent_type = Some(session.agent_type.as_str());
        let model_id = session.config.model_id.as_deref();
        let limit = Self::reliability_contract_limit(agent_type, model_id);
        let contract = coordinator
            .get_session_manager()
            .compression_contract_for_session(session_id, limit)?;
        Self::should_report_compression_preserved(
            session.compression_state.compression_count,
            Some(&contract),
        )
        .then_some(contract)
    }

    fn reliability_contract_limit(agent_type: Option<&str>, model_id: Option<&str>) -> usize {
        let agent_type = agent_type
            .map(str::trim)
            .filter(|agent_type| !agent_type.is_empty())
            .unwrap_or("DeepReview");
        let model_id = model_id
            .map(str::trim)
            .filter(|model_id| !model_id.is_empty())
            .unwrap_or_default();
        let is_review_subagent = get_agent_registry()
            .get_subagent_is_review(agent_type)
            .unwrap_or(false);

        ContextProfilePolicy::for_agent_context_and_model(
            agent_type,
            is_review_subagent,
            model_id,
            model_id,
        )
        .compression_contract_limit
    }

    fn should_report_compression_preserved(
        compression_count: usize,
        compression_contract: Option<&CompressionContract>,
    ) -> bool {
        compression_count > 0 && compression_contract.is_some_and(|contract| !contract.is_empty())
    }

    fn compression_contract_signal_count(contract: &CompressionContract) -> usize {
        contract.touched_files.len()
            + contract.verification_commands.len()
            + contract.blocking_failures.len()
            + contract.subagent_statuses.len()
    }

    fn fill_deep_review_reliability_signals(
        input: &mut Value,
        run_manifest: Option<&Value>,
        compression_contract: Option<&CompressionContract>,
    ) {
        if let Some(token_budget) = run_manifest.and_then(|manifest| {
            Self::value_for_any_key(manifest, &["tokenBudget", "token_budget"])
        }) {
            let has_context_pressure =
                Self::bool_for_any_key(
                    token_budget,
                    &["largeDiffSummaryFirst", "large_diff_summary_first"],
                ) || Self::has_non_empty_array_for_any_key(token_budget, &["warnings"]);
            if has_context_pressure {
                let count = Self::u64_for_any_key(
                    token_budget,
                    &["estimatedReviewerCalls", "estimated_reviewer_calls"],
                )
                .unwrap_or(0);
                Self::push_reliability_signal_if_missing(
                    input,
                    json!({
                        "kind": "context_pressure",
                        "severity": "info",
                        "count": count,
                        "source": "runtime"
                    }),
                );
            }
        }

        if let Some(contract) = compression_contract.filter(|contract| !contract.is_empty()) {
            let count = Self::compression_contract_signal_count(contract);
            if count > 0 {
                Self::push_reliability_signal_if_missing(
                    input,
                    json!({
                        "kind": "compression_preserved",
                        "severity": "info",
                        "count": count,
                        "source": "runtime"
                    }),
                );
            }
        }

        let partial_reviewer_count = Self::count_partial_reviewers(input);
        if partial_reviewer_count > 0 {
            Self::push_reliability_signal_if_missing(
                input,
                json!({
                    "kind": "partial_reviewer",
                    "severity": "warning",
                    "count": partial_reviewer_count,
                    "source": "runtime"
                }),
            );
        }

        let decision_item_count = Self::count_decision_items(input);
        if decision_item_count > 0 {
            Self::push_reliability_signal_if_missing(
                input,
                json!({
                    "kind": "user_decision",
                    "severity": "action",
                    "count": decision_item_count,
                    "source": "report"
                }),
            );
        }
    }

    /// Validate and fill missing fields with default values
    ///
    /// When AI-returned data is missing certain fields, fill with default values to avoid entire review failure
    fn validate_and_fill_defaults(
        input: &mut Value,
        deep_review: bool,
        run_manifest: Option<&Value>,
        compression_contract: Option<&CompressionContract>,
    ) {
        // Fill summary default values
        if input.get("summary").is_none() {
            warn!("CodeReview tool missing summary field, using default values");
            input["summary"] = json!({
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI did not return complete review results"
            });
        } else if let Some(summary) = input.get_mut("summary") {
            if summary.get("overall_assessment").is_none() {
                summary["overall_assessment"] = json!("None");
            }
            if summary.get("risk_level").is_none() {
                summary["risk_level"] = json!("low");
            }
            if summary.get("recommended_action").is_none() {
                summary["recommended_action"] = json!("approve");
            }
        } else {
            warn!(
                "CodeReview tool summary field exists but is not mutable object, using default values"
            );
            input["summary"] = json!({
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI returned invalid summary format"
            });
        }

        // Fill issues default values
        if input.get("issues").is_none() {
            warn!("CodeReview tool missing issues field, using default values");
            input["issues"] = json!([]);
        }

        // Fill positive_points default values
        if input.get("positive_points").is_none() {
            warn!("CodeReview tool missing positive_points field, using default values");
            input["positive_points"] = json!(["None"]);
        }

        if deep_review {
            input["review_mode"] = json!("deep");
            if input.get("review_scope").is_none() {
                input["review_scope"] = json!("Deep review scope was not provided");
            }
        } else if input.get("review_mode").is_none() {
            input["review_mode"] = json!("standard");
        }

        if input.get("reviewers").is_none() {
            input["reviewers"] = json!([]);
        }
        if deep_review {
            Self::fill_deep_review_packet_metadata(input, run_manifest);
            Self::fill_deep_review_reliability_signals(input, run_manifest, compression_contract);
        }

        if input.get("remediation_plan").is_none() {
            input["remediation_plan"] = json!([]);
        }

        if input.get("schema_version").is_none() {
            input["schema_version"] = json!(1);
        }
    }

    /// Generate review result using all default values
    ///
    /// Used when retries fail multiple times
    pub fn create_default_result() -> Value {
        json!({
            "schema_version": 1,
            "summary": {
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI review failed, using default result"
            },
            "issues": [],
            "positive_points": ["None"],
            "review_mode": "standard",
            "reviewers": [],
            "remediation_plan": [],
            "schema_version": 1
        })
    }
}

impl Default for CodeReviewTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CodeReviewTool {
    fn name(&self) -> &str {
        Self::name_str()
    }

    async fn description(&self) -> BitFunResult<String> {
        let lang = get_app_language_code().await;
        Ok(Self::description_for_language(lang.as_str()))
    }

    fn input_schema(&self) -> Value {
        Self::input_schema_value()
    }

    async fn input_schema_for_model(&self) -> Value {
        let lang = get_app_language_code().await;
        Self::input_schema_value_for_language(lang.as_str())
    }

    async fn input_schema_for_model_with_context(
        &self,
        context: Option<&crate::agentic::tools::framework::ToolUseContext>,
    ) -> Value {
        let lang = get_app_language_code().await;
        Self::input_schema_value_for_language_with_mode(
            lang.as_str(),
            Self::is_deep_review_context(context),
        )
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let mut filled_input = input.clone();
        let deep_review = Self::is_deep_review_context(Some(context));
        let compression_contract = deep_review
            .then(|| Self::compression_contract_for_context(context))
            .flatten();
        Self::validate_and_fill_defaults(
            &mut filled_input,
            deep_review,
            context.custom_data.get("deep_review_run_manifest"),
            compression_contract.as_ref(),
        );

        Ok(vec![ToolResult::Result {
            data: filled_input,
            result_for_assistant: Some("Code review results submitted successfully".to_string()),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::CodeReviewTool;
    use crate::agentic::core::{CompressionContract, CompressionContractItem};
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use serde_json::json;
    use std::collections::HashMap;

    fn tool_context(agent_type: Option<&str>) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: agent_type.map(str::to_string),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: Default::default(),
            workspace_services: None,
        }
    }

    #[tokio::test]
    async fn deep_review_schema_requires_deep_review_fields() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;
        let required = schema["required"].as_array().expect("required fields");

        for field in [
            "review_mode",
            "review_scope",
            "reviewers",
            "remediation_plan",
        ] {
            assert!(
                required.iter().any(|value| value.as_str() == Some(field)),
                "DeepReview schema should require {field}"
            );
        }
    }

    #[tokio::test]
    async fn deep_review_schema_accepts_reviewer_partial_output() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;
        let reviewer_properties = &schema["properties"]["reviewers"]["items"]["properties"];

        assert_eq!(reviewer_properties["partial_output"]["type"], "string");
    }

    #[tokio::test]
    async fn deep_review_schema_accepts_reviewer_packet_fallback_metadata() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;
        let reviewer_properties = &schema["properties"]["reviewers"]["items"]["properties"];

        assert_eq!(reviewer_properties["packet_id"]["type"], "string");
        assert_eq!(
            reviewer_properties["packet_status_source"]["enum"],
            json!(["reported", "inferred", "missing"])
        );
    }

    #[tokio::test]
    async fn deep_review_schema_accepts_structured_reliability_signals() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;
        let reliability_properties =
            &schema["properties"]["reliability_signals"]["items"]["properties"];

        assert_eq!(
            reliability_properties["kind"]["enum"],
            json!([
                "context_pressure",
                "compression_preserved",
                "partial_reviewer",
                "user_decision"
            ])
        );
        assert_eq!(
            reliability_properties["source"]["enum"],
            json!(["runtime", "manifest", "report", "inferred"])
        );
    }

    #[tokio::test]
    async fn deep_review_submission_defaults_missing_mode_to_deep() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let result = tool
            .call_impl(
                &json!({
                    "summary": {
                        "overall_assessment": "No blocking issues",
                        "risk_level": "low",
                        "recommended_action": "approve"
                    },
                    "issues": [],
                    "positive_points": []
                }),
                &context,
            )
            .await
            .expect("submit review result");

        let ToolResult::Result { data, .. } = &result[0] else {
            panic!("expected tool result");
        };
        assert_eq!(data["review_mode"], "deep");
        assert!(data["reviewers"].as_array().is_some());
        assert!(data["remediation_plan"].as_array().is_some());
    }

    #[tokio::test]
    async fn deep_review_submission_infers_unique_reviewer_packet_from_manifest() {
        let tool = CodeReviewTool::new();
        let mut context = tool_context(Some("DeepReview"));
        context.custom_data.insert(
            "deep_review_run_manifest".to_string(),
            json!({
                "workPackets": [
                    {
                        "packetId": "reviewer:ReviewSecurity",
                        "phase": "reviewer",
                        "subagentId": "ReviewSecurity",
                        "displayName": "Security Reviewer",
                        "roleName": "Security Reviewer"
                    }
                ]
            }),
        );

        let result = tool
            .call_impl(
                &json!({
                    "summary": {
                        "overall_assessment": "No blocking issues",
                        "risk_level": "low",
                        "recommended_action": "approve"
                    },
                    "issues": [],
                    "positive_points": [],
                    "reviewers": [
                        {
                            "name": "Security Reviewer",
                            "specialty": "security",
                            "status": "completed",
                            "summary": "Checked the security packet."
                        }
                    ]
                }),
                &context,
            )
            .await
            .expect("submit review result");

        let ToolResult::Result { data, .. } = &result[0] else {
            panic!("expected tool result");
        };
        assert_eq!(data["reviewers"][0]["packet_id"], "reviewer:ReviewSecurity");
        assert_eq!(data["reviewers"][0]["packet_status_source"], "inferred");
    }

    #[tokio::test]
    async fn deep_review_submission_marks_uninferable_packet_metadata_as_missing() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let result = tool
            .call_impl(
                &json!({
                    "summary": {
                        "overall_assessment": "No blocking issues",
                        "risk_level": "low",
                        "recommended_action": "approve"
                    },
                    "issues": [],
                    "positive_points": [],
                    "reviewers": [
                        {
                            "name": "Unknown Reviewer",
                            "specialty": "unknown",
                            "status": "completed",
                            "summary": "Packet was omitted."
                        }
                    ]
                }),
                &context,
            )
            .await
            .expect("submit review result");

        let ToolResult::Result { data, .. } = &result[0] else {
            panic!("expected tool result");
        };
        assert!(data["reviewers"][0].get("packet_id").is_none());
        assert_eq!(data["reviewers"][0]["packet_status_source"], "missing");
    }

    #[tokio::test]
    async fn deep_review_submission_marks_existing_packet_metadata_as_reported() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let result = tool
            .call_impl(
                &json!({
                    "summary": {
                        "overall_assessment": "No blocking issues",
                        "risk_level": "low",
                        "recommended_action": "approve"
                    },
                    "issues": [],
                    "positive_points": [],
                    "reviewers": [
                        {
                            "name": "Security Reviewer",
                            "specialty": "security",
                            "status": "completed",
                            "summary": "Packet was reported.",
                            "packet_id": "reviewer:ReviewSecurity"
                        }
                    ]
                }),
                &context,
            )
            .await
            .expect("submit review result");

        let ToolResult::Result { data, .. } = &result[0] else {
            panic!("expected tool result");
        };
        assert_eq!(data["reviewers"][0]["packet_id"], "reviewer:ReviewSecurity");
        assert_eq!(data["reviewers"][0]["packet_status_source"], "reported");
    }

    #[tokio::test]
    async fn deep_review_submission_fills_runtime_reliability_signals() {
        let tool = CodeReviewTool::new();
        let mut context = tool_context(Some("DeepReview"));
        context.custom_data.insert(
            "deep_review_run_manifest".to_string(),
            json!({
                "tokenBudget": {
                    "largeDiffSummaryFirst": true,
                    "warnings": [],
                    "estimatedReviewerCalls": 7
                }
            }),
        );

        let result = tool
            .call_impl(
                &json!({
                    "summary": {
                        "overall_assessment": "Review completed with reduced confidence",
                        "risk_level": "medium",
                        "recommended_action": "request_changes"
                    },
                    "issues": [],
                    "positive_points": [],
                    "reviewers": [
                        {
                            "name": "Security Reviewer",
                            "specialty": "security",
                            "status": "partial_timeout",
                            "summary": "Timed out after partial evidence.",
                            "partial_output": "Found one likely issue before timeout."
                        }
                    ],
                    "report_sections": {
                        "remediation_groups": {
                            "needs_decision": [
                                "Decide whether to block the release."
                            ]
                        }
                    }
                }),
                &context,
            )
            .await
            .expect("submit review result");

        let ToolResult::Result { data, .. } = &result[0] else {
            panic!("expected tool result");
        };
        assert_eq!(
            data["reliability_signals"],
            json!([
                {
                    "kind": "context_pressure",
                    "severity": "info",
                    "count": 7,
                    "source": "runtime"
                },
                {
                    "kind": "partial_reviewer",
                    "severity": "warning",
                    "count": 1,
                    "source": "runtime"
                },
                {
                    "kind": "user_decision",
                    "severity": "action",
                    "count": 1,
                    "source": "report"
                }
            ])
        );
    }

    #[test]
    fn deep_review_defaults_include_compression_contract_reliability_signal() {
        let contract = CompressionContract {
            touched_files: vec!["src/web-ui/src/flow_chat/utils/codeReviewReport.ts".to_string()],
            verification_commands: vec![CompressionContractItem {
                target: "pnpm --dir src/web-ui run test:run".to_string(),
                status: "succeeded".to_string(),
                summary: "Frontend report tests passed.".to_string(),
                error_kind: None,
            }],
            blocking_failures: vec![],
            subagent_statuses: vec![],
        };
        let mut input = json!({
            "summary": {
                "overall_assessment": "No blocking issues",
                "risk_level": "low",
                "recommended_action": "approve"
            },
            "issues": [],
            "positive_points": []
        });

        CodeReviewTool::validate_and_fill_defaults(&mut input, true, None, Some(&contract));

        assert_eq!(
            input["reliability_signals"],
            json!([
                {
                    "kind": "compression_preserved",
                    "severity": "info",
                    "count": 2,
                    "source": "runtime"
                }
            ])
        );
    }

    #[test]
    fn deep_review_reliability_contract_limit_uses_context_profile_policy() {
        assert_eq!(
            CodeReviewTool::reliability_contract_limit(Some("DeepReview"), Some("gpt-5")),
            8
        );
        assert_eq!(
            CodeReviewTool::reliability_contract_limit(Some("DeepReview"), Some("gpt-5-mini")),
            4
        );
    }

    #[test]
    fn deep_review_compression_signal_requires_completed_compression() {
        let contract = CompressionContract {
            touched_files: vec!["src/main.rs".to_string()],
            verification_commands: vec![],
            blocking_failures: vec![],
            subagent_statuses: vec![],
        };

        assert!(!CodeReviewTool::should_report_compression_preserved(
            0,
            Some(&contract)
        ));
        assert!(CodeReviewTool::should_report_compression_preserved(
            1,
            Some(&contract)
        ));
        assert!(!CodeReviewTool::should_report_compression_preserved(
            1,
            Some(&CompressionContract::default())
        ));
    }
}
