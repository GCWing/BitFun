//! Edit constraint guard.
//!
//! Extracts explicit "don't modify X" constraints from user instructions and
//! exposes deterministic checks for file-mutation tools. Extraction evidence is
//! persisted with the session, while every guard decision and successful direct
//! file mutation is appended to a session-scoped JSONL telemetry stream.

use log::warn;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::framework::{ToolUseContext, ValidationResult};
use crate::infrastructure::ai::get_global_ai_client_factory;
use crate::util::json_extract::extract_json_from_ai_response;
use crate::util::types::Message;

mod model;
mod shell_targets;

pub use model::{
    AgentCreatedPathRecord, ConstraintExtractionRecord, ConstraintMatcher,
    ConstraintOperationScope, ConstraintRevocation, ConstraintSource, EditConstraintState,
    ExtractedConstraint, ExtractionFailure, ExtractionStatus, ModelExtractionStatus,
};
use shell_targets::explicit_bash_mutation_targets;

pub const EDIT_CONSTRAINT_METADATA_KEY: &str = "editConstraintGuard";
const EDIT_CONSTRAINT_SCHEMA_VERSION: u32 = 5;
const MAX_PROMPT_CHARS: usize = 8_000;
const MAX_RESPONSE_TELEMETRY_CHARS: usize = 4_000;
const MAX_MODEL_ATTEMPTS: usize = 2;
const MAX_RECURSIVE_INSPECTION_ENTRIES: usize = 100_000;
const TELEMETRY_RELATIVE_PATH: &str = "telemetry/edit-constraint-guard.jsonl";

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You update the active file-edit prohibitions for a software task.

You receive the currently active prohibitions and the latest user message.

- Add a prohibition only when the latest message explicitly forbids modifying
  certain files, file types, or categories of files.
- Revoke an active prohibition only when the latest message explicitly cancels,
  relaxes, or contradicts it (e.g. "you may modify tests now"). A revocation
  MUST copy the exact constraint_id from the active list. Never invent an id.
- An unrelated message does not revoke anything.
- Ignore constraints about anything other than *which files may be edited*.

For each added prohibition, classify it into exactly ONE matcher kind and one
operation scope:
- "test_files": the prohibition is about test files / testing logic in general
- "path_contains": the prohibition names specific files or keywords (give the literal substrings)
- "path_under_dir": the prohibition names a specific directory (give the directory names)
- "extension": the prohibition is about a specific file type (give the extensions, including the dot)
- "unmatched": you found a prohibition but it doesn't fit any of the above

Use operation_scope "delete_only" only when the user explicitly prohibits
deleting/removing files, without also prohibiting other edits. Otherwise use
"all".

Respond with ONLY a fenced ```json code block containing this exact shape:
```json
{
  "additions": [
    {"description": "<short paraphrase>", "operation_scope": "all", "matcher": {"kind": "test_files"}},
    {"description": "<short paraphrase>", "operation_scope": "all", "matcher": {"kind": "path_contains", "substrings": ["..."]}},
    {"description": "<short paraphrase>", "operation_scope": "all", "matcher": {"kind": "path_under_dir", "dirs": ["..."]}},
    {"description": "<short paraphrase>", "operation_scope": "all", "matcher": {"kind": "extension", "exts": [".ext"]}},
    {"description": "<short paraphrase>", "operation_scope": "all", "matcher": {"kind": "unmatched"}}
  ],
  "revocations": [
    {"constraint_id": "<exact active constraint id>", "description": "<what the user explicitly relaxed>"}
  ]
}
```
If the latest message changes nothing, return empty `additions` and
`revocations`."#;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractionResponse {
    additions: Vec<ExtractedConstraint>,
    revocations: Vec<ConstraintRevocation>,
}

pub fn message_sha256(message: &str) -> String {
    format!("{:x}", Sha256::digest(message.as_bytes()))
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn truncate_for_extraction(message: &str) -> (String, bool) {
    let chars = message.chars().collect::<Vec<_>>();
    if chars.len() <= MAX_PROMPT_CHARS {
        return (message.to_string(), false);
    }

    let side = MAX_PROMPT_CHARS / 2;
    let mut output = chars[..side].iter().collect::<String>();
    output.push_str("\n...[middle of task omitted for constraint extraction]...\n");
    output.extend(chars[chars.len() - side..].iter());
    (output, true)
}

fn relevant_sentences(message: &str) -> impl Iterator<Item = &str> {
    message
        .split_inclusive(|character: char| {
            matches!(character, '.' | '!' | '?' | '\n' | '。' | '！' | '？')
        })
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
}

fn has_prohibition_signal(message: &str) -> bool {
    let lower = message.to_lowercase();
    [
        "do not",
        "don't",
        "dont",
        "must not",
        "should not",
        "never modify",
        "never change",
        "do not delete",
        "don't delete",
        "must not delete",
        "no need to modify",
        "no need to change",
        "don't have to modify",
        "do not have to modify",
        "not allowed to modify",
        "avoid modifying",
        "already taken care of",
        "keep tests unchanged",
        "keep test files unchanged",
        "leave tests unchanged",
        "leave test files unchanged",
        "tests are off limits",
        "test files are off limits",
        "tests must remain unchanged",
        "test files must remain unchanged",
        "without modifying tests",
        "without modifying test files",
        "only modify non-test",
        "non-test files only",
        "不要",
        "不得",
        "不能修改",
        "不能删除",
        "禁止修改",
        "禁止删除",
        "无需修改",
        "不需要修改",
        "测试文件保持不变",
        "仅修改非测试",
    ]
    .iter()
    .any(|signal| lower.contains(signal))
}

fn deterministic_test_constraint(message: &str) -> Option<ExtractedConstraint> {
    relevant_sentences(message).find_map(|sentence| {
        let lower = sentence.to_lowercase();
        let explicitly_relaxes_tests = [
            "may modify tests",
            "may modify test files",
            "can modify tests",
            "can modify test files",
            "allowed to modify tests",
            "allowed to modify test files",
            "tests are no longer off limits",
            "test files are no longer off limits",
            "test restriction is lifted",
            "test-file restriction is lifted",
            "可以修改测试",
            "可以改测试",
            "允许修改测试",
            "测试文件可以修改",
            "测试可以修改",
            "不再禁止修改测试",
        ]
        .iter()
        .any(|signal| lower.contains(signal));
        if explicitly_relaxes_tests {
            return None;
        }
        let mentions_tests = lower.contains("test file")
            || lower.contains("tests")
            || lower.contains("testing logic")
            || lower.contains("测试");
        let mentions_non_delete_mutation = [
            "modify",
            "change",
            "edit",
            "touch",
            "update",
            "alter",
            "write",
            "修改",
            "改动",
            "更改",
            "编辑",
            "保持不变",
        ]
        .iter()
        .any(|word| lower.contains(word));
        let mentions_delete = ["delete", "remove", "删除", "移除"]
            .iter()
            .any(|word| lower.contains(word));
        let mentions_mutation = mentions_non_delete_mutation || mentions_delete;
        let prohibits_mutation = has_prohibition_signal(&lower);

        (mentions_tests && mentions_mutation && prohibits_mutation).then(|| {
            let source_text = sentence.chars().take(500).collect::<String>();
            ExtractedConstraint {
                id: if mentions_delete && !mentions_non_delete_mutation {
                    "deterministic:test_files:delete_only".to_string()
                } else {
                    "deterministic:test_files".to_string()
                },
                description: if mentions_delete && !mentions_non_delete_mutation {
                    "The task explicitly says not to delete test files or testing logic".to_string()
                } else {
                    "The task explicitly says not to modify test files or testing logic".to_string()
                },
                operation_scope: if mentions_delete && !mentions_non_delete_mutation {
                    ConstraintOperationScope::DeleteOnly
                } else {
                    ConstraintOperationScope::All
                },
                matcher: ConstraintMatcher::TestFiles,
                source: ConstraintSource::Deterministic,
                source_text: Some(source_text),
            }
        })
    })
}

fn candidate_paths(context: Option<&ToolUseContext>, file_path: &str) -> Vec<String> {
    let mut candidates = vec![file_path.replace('\\', "/")];
    let Some(context) = context else {
        return candidates;
    };
    let Ok(resolved) = context.resolve_tool_path(file_path) else {
        return candidates;
    };
    for path in [
        resolved.logical_path.clone(),
        resolved.resolved_path.clone(),
    ] {
        let normalized = path.replace('\\', "/");
        if !candidates.contains(&normalized) {
            candidates.push(normalized);
        }
    }
    if !resolved.uses_remote_workspace_backend() {
        let resolved_path = Path::new(&resolved.resolved_path);
        let canonical = fs::canonicalize(resolved_path).ok().or_else(|| {
            let parent = resolved_path.parent()?;
            let file_name = resolved_path.file_name()?;
            fs::canonicalize(parent)
                .ok()
                .map(|parent| parent.join(file_name))
        });
        if let Some(canonical) = canonical {
            let normalized = canonical.to_string_lossy().replace('\\', "/");
            if !candidates.contains(&normalized) {
                candidates.push(normalized);
            }
        }
    }
    candidates
}

fn normalize_model_constraints(constraints: &mut [ExtractedConstraint], message_sha256: &str) {
    let message_prefix = message_sha256.chars().take(12).collect::<String>();
    for (index, constraint) in constraints.iter_mut().enumerate() {
        constraint.id = format!("model:{message_prefix}:{index}");
        constraint.source = ConstraintSource::Model;
    }
}

fn validated_revocation_ids(
    revocations: &[ConstraintRevocation],
    active_constraints: &[ExtractedConstraint],
    revocation_authorized: bool,
) -> (Vec<String>, Vec<String>) {
    if !revocation_authorized {
        return (Vec::new(), Vec::new());
    }

    let mut revoked = Vec::new();
    let mut unmatched = Vec::new();
    for revocation in revocations {
        let constraint_id = revocation.constraint_id.trim();
        if active_constraints
            .iter()
            .any(|constraint| constraint.id == constraint_id)
        {
            if !revoked.iter().any(|existing| existing == constraint_id) {
                revoked.push(constraint_id.to_string());
            }
        } else if !unmatched.iter().any(|existing| existing == constraint_id) {
            unmatched.push(constraint_id.to_string());
        }
    }
    (revoked, unmatched)
}

fn response_excerpt(response: &str) -> String {
    response
        .chars()
        .take(MAX_RESPONSE_TELEMETRY_CHARS)
        .collect()
}

/// Extract constraints from one user instruction when there is no prior
/// session state. Kept as the simple entry point for callers and tests that do
/// not need revocation semantics.
pub async fn extract_constraints(user_message: &str) -> ConstraintExtractionRecord {
    extract_constraints_with_active(user_message, &[]).await
}

/// Extract additions and explicit revocations from one user instruction.
/// Explicit test-file prohibitions are recognized deterministically before the
/// model call, while revocations are accepted only from a successfully parsed
/// fast-model response that references an active constraint id.
pub async fn extract_constraints_with_active(
    user_message: &str,
    active_constraints: &[ExtractedConstraint],
) -> ConstraintExtractionRecord {
    extract_constraints_with_active_and_revocation_authorization(
        user_message,
        active_constraints,
        true,
    )
    .await
}

/// Extract additions and explicit revocations from one instruction, applying
/// revocations only when the caller has established that the text came from a
/// real user submission. Internal follow-ups may add protections but must
/// never relax a protection on the user's behalf.
pub async fn extract_constraints_with_active_and_revocation_authorization(
    user_message: &str,
    active_constraints: &[ExtractedConstraint],
    revocation_authorized: bool,
) -> ConstraintExtractionRecord {
    let started_at = Instant::now();
    let input_chars = user_message.chars().count();
    let message_sha256 = message_sha256(user_message);
    let active_constraint_ids = active_constraints
        .iter()
        .map(|constraint| constraint.id.clone())
        .collect::<Vec<_>>();
    if user_message.trim().is_empty() {
        return ConstraintExtractionRecord {
            message_sha256,
            dialog_turn_id: None,
            status: ExtractionStatus::NoConstraints,
            constraints: Vec::new(),
            deterministic_constraint_count: 0,
            model_attempts: 0,
            active_constraint_ids,
            revocation_authorized,
            model_status: ModelExtractionStatus::NotRun,
            model_constraints: Vec::new(),
            model_revocations: Vec::new(),
            revoked_constraint_ids: Vec::new(),
            unmatched_revocation_ids: Vec::new(),
            input_chars,
            prompt_chars: 0,
            input_truncated: false,
            latency_ms: 0,
            extracted_at_ms: timestamp_ms(),
            failure: None,
            response_excerpt: None,
        };
    }

    let mut constraints = deterministic_test_constraint(user_message)
        .into_iter()
        .collect::<Vec<_>>();
    let deterministic_constraint_count = constraints.len();
    let (truncated, input_truncated) = truncate_for_extraction(user_message);
    let prompt_chars = truncated.chars().count();

    // Once constraints are active, every new user instruction is sent to fast
    // so an explicit relaxation cannot be missed by a keyword prefilter.
    if !has_prohibition_signal(user_message) && active_constraints.is_empty() {
        return ConstraintExtractionRecord {
            message_sha256,
            dialog_turn_id: None,
            status: if constraints.is_empty() {
                ExtractionStatus::NoConstraints
            } else {
                ExtractionStatus::Extracted
            },
            constraints,
            deterministic_constraint_count,
            model_attempts: 0,
            active_constraint_ids,
            revocation_authorized,
            model_status: ModelExtractionStatus::NotRun,
            model_constraints: Vec::new(),
            model_revocations: Vec::new(),
            revoked_constraint_ids: Vec::new(),
            unmatched_revocation_ids: Vec::new(),
            input_chars,
            prompt_chars,
            input_truncated,
            latency_ms: started_at
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            extracted_at_ms: timestamp_ms(),
            failure: None,
            response_excerpt: None,
        };
    }

    let factory = match get_global_ai_client_factory().await {
        Ok(factory) => factory,
        Err(error) => {
            return extraction_with_failure(
                started_at,
                message_sha256,
                constraints,
                deterministic_constraint_count,
                0,
                active_constraint_ids,
                revocation_authorized,
                input_chars,
                prompt_chars,
                input_truncated,
                "client_factory",
                error.to_string(),
                None,
            );
        }
    };
    let client = match factory.get_client_resolved("fast").await {
        Ok(client) => client,
        Err(error) => {
            return extraction_with_failure(
                started_at,
                message_sha256,
                constraints,
                deterministic_constraint_count,
                0,
                active_constraint_ids,
                revocation_authorized,
                input_chars,
                prompt_chars,
                input_truncated,
                "client_resolution",
                error.to_string(),
                None,
            );
        }
    };

    let active_constraints_json =
        serde_json::to_string(active_constraints).unwrap_or_else(|_| "[]".to_string());
    let task_message = format!(
        "<active_constraints>\n{active_constraints_json}\n</active_constraints>\n\
         <latest_user_message>\n{truncated}\n</latest_user_message>"
    );
    let mut failure = None;
    let mut last_response_excerpt = None;
    let mut model_attempts = 0;
    let mut model_status = ModelExtractionStatus::Failed;
    let mut model_constraints = Vec::new();
    let mut model_revocations = Vec::new();
    let mut revoked_constraint_ids = Vec::new();
    let mut unmatched_revocation_ids = Vec::new();

    for attempt in 1..=MAX_MODEL_ATTEMPTS {
        model_attempts = attempt;
        let response = match client
            .send_message(
                vec![
                    Message::system(EXTRACTION_SYSTEM_PROMPT.to_string()),
                    Message::user(task_message.clone()),
                ],
                None,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                failure = Some(ExtractionFailure {
                    stage: "model_request".to_string(),
                    reason: error.to_string(),
                });
                continue;
            }
        };

        last_response_excerpt = Some(response_excerpt(&response.text));
        if response.text.trim().is_empty() {
            failure = Some(ExtractionFailure {
                stage: "empty_response".to_string(),
                reason: "The extraction model returned no text".to_string(),
            });
            continue;
        }
        let Some(json_string) = extract_json_from_ai_response(&response.text) else {
            failure = Some(ExtractionFailure {
                stage: "json_extraction".to_string(),
                reason: "No JSON object was found in the extraction response".to_string(),
            });
            continue;
        };
        match serde_json::from_str::<ExtractionResponse>(&json_string) {
            Ok(mut parsed) => {
                normalize_model_constraints(&mut parsed.additions, &message_sha256);
                model_constraints = parsed.additions.clone();
                model_revocations = parsed.revocations;

                (revoked_constraint_ids, unmatched_revocation_ids) = validated_revocation_ids(
                    &model_revocations,
                    active_constraints,
                    revocation_authorized,
                );

                for constraint in &model_constraints {
                    if !constraints
                        .iter()
                        .any(|existing| existing.matcher == constraint.matcher)
                    {
                        constraints.push(constraint.clone());
                    }
                }
                model_status = ModelExtractionStatus::Parsed;
                failure = None;
                break;
            }
            Err(error) => {
                failure = Some(ExtractionFailure {
                    stage: "schema_validation".to_string(),
                    reason: error.to_string(),
                });
            }
        }
    }

    let status = if !constraints.is_empty() || !revoked_constraint_ids.is_empty() {
        ExtractionStatus::Extracted
    } else if failure.is_some() {
        ExtractionStatus::Failed
    } else {
        ExtractionStatus::NoConstraints
    };

    ConstraintExtractionRecord {
        message_sha256,
        dialog_turn_id: None,
        status,
        constraints,
        deterministic_constraint_count,
        model_attempts,
        active_constraint_ids,
        revocation_authorized,
        model_status,
        model_constraints,
        model_revocations,
        revoked_constraint_ids,
        unmatched_revocation_ids,
        input_chars,
        prompt_chars,
        input_truncated,
        latency_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
        extracted_at_ms: timestamp_ms(),
        failure,
        response_excerpt: last_response_excerpt,
    }
}

#[allow(clippy::too_many_arguments)]
fn extraction_with_failure(
    started_at: Instant,
    message_sha256: String,
    constraints: Vec<ExtractedConstraint>,
    deterministic_constraint_count: usize,
    model_attempts: usize,
    active_constraint_ids: Vec<String>,
    revocation_authorized: bool,
    input_chars: usize,
    prompt_chars: usize,
    input_truncated: bool,
    stage: &str,
    reason: String,
    response_excerpt: Option<String>,
) -> ConstraintExtractionRecord {
    ConstraintExtractionRecord {
        message_sha256,
        dialog_turn_id: None,
        status: if constraints.is_empty() {
            ExtractionStatus::Failed
        } else {
            ExtractionStatus::Extracted
        },
        constraints,
        deterministic_constraint_count,
        model_attempts,
        active_constraint_ids,
        revocation_authorized,
        model_status: ModelExtractionStatus::Failed,
        model_constraints: Vec::new(),
        model_revocations: Vec::new(),
        revoked_constraint_ids: Vec::new(),
        unmatched_revocation_ids: Vec::new(),
        input_chars,
        prompt_chars,
        input_truncated,
        latency_ms: started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
        extracted_at_ms: timestamp_ms(),
        failure: Some(ExtractionFailure {
            stage: stage.to_string(),
            reason,
        }),
        response_excerpt,
    }
}

pub fn find_violation<'a>(
    constraints: &'a [ExtractedConstraint],
    file_path: &str,
) -> Option<&'a ExtractedConstraint> {
    find_violation_for_operation(constraints, file_path, "write")
}

fn find_violation_for_operation<'a>(
    constraints: &'a [ExtractedConstraint],
    file_path: &str,
    operation: &str,
) -> Option<&'a ExtractedConstraint> {
    constraints.iter().find(|constraint| {
        constraint.operation_scope.applies_to(operation) && constraint.matcher.matches(file_path)
    })
}

pub fn violation_message(file_path: &str, constraint: &ExtractedConstraint) -> String {
    format!(
        "This file (`{file_path}`) matches a constraint stated in the task: \"{}\". \
         This edit was not applied.\n\n\
         Editing a file you were told not to touch usually means your own implementation \
         doesn't match what's expected — not that the file is wrong. Reconsider your \
         source-code approach instead of adjusting this file.",
        constraint.description
    )
}

fn telemetry_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn try_append_tool_telemetry(
    context: &ToolUseContext,
    event: &Value,
) -> Result<std::path::PathBuf, String> {
    let Some(session_id) = context.session_id.as_deref() else {
        return Err("tool context has no session id".to_string());
    };
    let session_dir = context
        .current_workspace_session_dir(session_id)
        .map_err(|error| format!("failed to resolve session directory: {error}"))?;
    let path = session_dir.join(TELEMETRY_RELATIVE_PATH);
    append_jsonl(&path, event)?;
    Ok(path)
}

fn append_jsonl(path: &Path, event: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "telemetry path has no parent".to_string())?;

    let _guard = telemetry_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create telemetry directory: {error}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("failed to open telemetry stream: {error}"))?;
    serde_json::to_writer(&mut file, event)
        .and_then(|_| file.write_all(b"\n").map_err(serde_json::Error::io))
        .map_err(|error| format!("failed to append telemetry event: {error}"))?;
    Ok(())
}

fn append_tool_telemetry(context: &ToolUseContext, event: &Value) {
    if let Err(error) = try_append_tool_telemetry(context, event) {
        warn!("Failed to append edit constraint telemetry event: {error}");
    }
}

fn resolved_path(context: &ToolUseContext, file_path: &str) -> Option<String> {
    context
        .resolve_tool_path(file_path)
        .ok()
        .map(|resolved| resolved.resolved_path)
}

fn decision_result(
    context: Option<&ToolUseContext>,
    tool_name: &str,
    operation: &str,
    file_path: &str,
    decision: &str,
    force_requested: bool,
    state: Option<&EditConstraintState>,
    violation: Option<&ExtractedConstraint>,
    message: Option<String>,
    error_code: Option<i32>,
) -> Option<ValidationResult> {
    let decision_id = Uuid::new_v4().to_string();
    if let Some(context) = context {
        append_tool_telemetry(
            context,
            &json!({
                "event": "guard_decision",
                "schema_version": EDIT_CONSTRAINT_SCHEMA_VERSION,
                "decision_id": decision_id,
                "timestamp_ms": timestamp_ms(),
                "session_id": context.session_id,
                "dialog_turn_id": context.dialog_turn_id,
                "tool_call_id": context.tool_call_id,
                "agent_type": context.agent_type,
                "tool_name": tool_name,
                "operation": operation,
                "requested_path": file_path,
                "resolved_path": resolved_path(context, file_path),
                "workspace_kind": if context.is_remote() { "remote" } else { "local" },
                "decision": decision,
                "force_requested": force_requested,
                "extraction_status": state.and_then(EditConstraintState::latest_status),
                "constraint": violation,
            }),
        );
    }

    message.map(|message| ValidationResult {
        result: false,
        message: Some(message),
        error_code,
        meta: Some(json!({
            "failure_kind": "edit_constraint_guard",
            "guard_decision_id": decision_id,
            "guard_decision": decision,
            "constraint_id": violation.map(|constraint| constraint.id.as_str()),
            "protected_path": file_path,
            "force_requested": force_requested,
        })),
    })
}

/// Deterministic guard check shared by direct file mutation tools.
///
/// `force` is no longer a model-controlled escape hatch. A stale caller that
/// still sends it is rejected and recorded explicitly.
pub fn check(
    context: Option<&ToolUseContext>,
    tool_name: &str,
    operation: &str,
    file_path: &str,
    force_requested: bool,
) -> Option<ValidationResult> {
    let state = context
        .and_then(|value| value.session_id.as_deref())
        .and_then(|session_id| {
            get_global_coordinator()?
                .get_session_manager()
                .edit_constraint_state(session_id)
        });

    if force_requested {
        return decision_result(
            context,
            tool_name,
            operation,
            file_path,
            "force_denied",
            true,
            state.as_ref(),
            None,
            Some(
                "`force` cannot override constraints stated by the user. Reconsider the source-code approach without modifying the protected file."
                    .to_string(),
            ),
            Some(403),
        );
    }

    let paths = candidate_paths(context, file_path);
    let violation = state.as_ref().and_then(|state| {
        paths
            .iter()
            .find_map(|path| find_violation_for_operation(&state.constraints, path, operation))
    });
    if let Some(violation) = violation {
        return decision_result(
            context,
            tool_name,
            operation,
            file_path,
            "deny",
            false,
            state.as_ref(),
            Some(violation),
            Some(violation_message(file_path, violation)),
            Some(403),
        );
    }

    let decision = match state.as_ref().and_then(EditConstraintState::latest_status) {
        Some(ExtractionStatus::Failed) | None => "allow_extraction_unavailable",
        _ => "allow",
    };
    decision_result(
        context,
        tool_name,
        operation,
        file_path,
        decision,
        false,
        state.as_ref(),
        None,
        None,
        None,
    );
    None
}

async fn write_target_exists(context: &ToolUseContext, file_path: &str) -> Option<bool> {
    let resolved = context.resolve_tool_path(file_path).ok()?;
    if resolved.uses_remote_workspace_backend() {
        let fs = context.ws_fs()?;
        // Treat a failed remote inspection as existing so an unavailable
        // filesystem cannot become a way to overwrite a protected test file.
        return Some(fs.exists(&resolved.resolved_path).await.unwrap_or(true));
    }

    Some(Path::new(&resolved.resolved_path).exists())
}

fn has_only_relaxable_test_file_violations(
    state: &EditConstraintState,
    paths: &[String],
    operation: &str,
) -> bool {
    let violations = paths
        .iter()
        .flat_map(|path| {
            state.constraints.iter().filter(move |constraint| {
                constraint.operation_scope.applies_to(operation) && constraint.matcher.matches(path)
            })
        })
        .collect::<Vec<_>>();

    !violations.is_empty()
        && violations.iter().all(|constraint| {
            matches!(&constraint.matcher, ConstraintMatcher::TestFiles)
                && constraint.operation_scope == ConstraintOperationScope::All
        })
}

fn can_mutate_agent_created_test_file(
    state: Option<&EditConstraintState>,
    paths: &[String],
    operation: &str,
    newly_created: bool,
) -> bool {
    state.is_some_and(|state| {
        (newly_created || state.is_agent_created_path(paths))
            && has_only_relaxable_test_file_violations(state, paths, operation)
    })
}

/// Guard a Write operation while allowing a newly-created test helper.
///
/// A task instruction not to modify tests protects the repository's existing
/// tests from being adjusted to satisfy the task. It does not prohibit an
/// agent from creating an untracked repro or verification file. Other
/// constraint kinds retain their strict create-or-modify semantics.
pub async fn check_write(
    context: Option<&ToolUseContext>,
    tool_name: &str,
    operation: &str,
    file_path: &str,
    force_requested: bool,
) -> Option<ValidationResult> {
    let state = context
        .and_then(|value| value.session_id.as_deref())
        .and_then(|session_id| {
            get_global_coordinator()?
                .get_session_manager()
                .edit_constraint_state(session_id)
        });

    let is_new_file = if force_requested {
        false
    } else if let Some(context) = context {
        write_target_exists(context, file_path).await == Some(false)
    } else {
        false
    };
    let paths = candidate_paths(context, file_path);

    if can_mutate_agent_created_test_file(state.as_ref(), &paths, operation, is_new_file) {
        decision_result(
            context,
            tool_name,
            operation,
            file_path,
            if is_new_file {
                "allow_new_test_file"
            } else {
                "allow_agent_created_test_file"
            },
            false,
            state.as_ref(),
            None,
            None,
            None,
        );
        return None;
    }

    check(context, tool_name, operation, file_path, force_requested)
}

/// Guard an Edit operation while preserving the session provenance of helper
/// tests the agent created itself.
pub fn check_edit(
    context: Option<&ToolUseContext>,
    tool_name: &str,
    operation: &str,
    file_path: &str,
    force_requested: bool,
) -> Option<ValidationResult> {
    let state = context
        .and_then(|value| value.session_id.as_deref())
        .and_then(|session_id| {
            get_global_coordinator()?
                .get_session_manager()
                .edit_constraint_state(session_id)
        });
    let paths = candidate_paths(context, file_path);
    if !force_requested
        && can_mutate_agent_created_test_file(state.as_ref(), &paths, operation, false)
    {
        decision_result(
            context,
            tool_name,
            operation,
            file_path,
            "allow_agent_created_test_file",
            false,
            state.as_ref(),
            None,
            None,
            None,
        );
        return None;
    }
    check(context, tool_name, operation, file_path, force_requested)
}

/// Guard a Delete operation while allowing the agent to clean up a test file
/// it created in this session. A user-authored delete-only prohibition remains
/// strict and is never relaxed by file provenance.
pub fn check_delete(
    context: Option<&ToolUseContext>,
    tool_name: &str,
    operation: &str,
    file_path: &str,
    force_requested: bool,
) -> Option<ValidationResult> {
    check_edit(context, tool_name, operation, file_path, force_requested)
}

/// Preflight explicit file targets in terminal commands. This covers the
/// normal shell forms agents use for mutation while retaining Bash for build
/// and test execution. Commands whose write targets are computed dynamically
/// remain observable through the final patch/provenance telemetry instead of
/// being silently treated as guarded direct-file writes.
pub fn check_bash_command(context: &ToolUseContext, command: &str) -> Option<ValidationResult> {
    for path in explicit_bash_mutation_targets(command) {
        if let Some(rejection) = check(
            Some(context),
            "Bash",
            "explicit_shell_mutation",
            &path,
            false,
        ) {
            return Some(rejection);
        }
    }
    None
}

/// Checks the target and every non-symlink descendant before recursive delete.
/// Inspection failures are fail-closed only when an enforceable constraint is
/// active, because otherwise there is no protected path to discover.
pub async fn check_recursive_delete(
    context: Option<&ToolUseContext>,
    root_path: &str,
    force_requested: bool,
) -> Option<ValidationResult> {
    if let Some(rejection) = check_delete(
        context,
        "Delete",
        "recursive_delete",
        root_path,
        force_requested,
    ) {
        return Some(rejection);
    }
    let context = context?;
    let session_id = context.session_id.as_deref()?;
    let state = get_global_coordinator()?
        .get_session_manager()
        .edit_constraint_state(session_id)?;
    if !state.has_enforceable_constraints() {
        return None;
    }

    let resolved = match context.resolve_tool_path(root_path) {
        Ok(resolved) => resolved,
        Err(_) => return None,
    };
    let Some(workspace_fs) = context.ws_fs() else {
        if resolved.uses_remote_workspace_backend() {
            return decision_result(
                Some(context),
                "Delete",
                "recursive_delete",
                root_path,
                "deny_inspection_failed",
                false,
                Some(&state),
                None,
                Some(
                    "Recursive delete was not applied because the remote workspace filesystem is unavailable"
                        .to_string(),
                ),
                Some(503),
            );
        }
        return check_local_recursive_delete(context, root_path, &resolved.resolved_path, &state);
    };
    match workspace_fs.is_dir(&resolved.resolved_path).await {
        Ok(true) => {}
        Ok(false) => return None,
        Err(error) => {
            return decision_result(
                Some(context),
                "Delete",
                "recursive_delete",
                root_path,
                "deny_inspection_failed",
                false,
                Some(&state),
                None,
                Some(format!(
                    "Recursive delete was not applied because the target type could not be inspected: {error}"
                )),
                Some(503),
            );
        }
    }

    let mut pending = vec![resolved.resolved_path];
    let mut inspected = 0usize;
    while let Some(directory) = pending.pop() {
        let entries = match workspace_fs.read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) => {
                return decision_result(
                    Some(context),
                    "Delete",
                    "recursive_delete",
                    root_path,
                    "deny_inspection_failed",
                    false,
                    Some(&state),
                    None,
                    Some(format!(
                        "Recursive delete was not applied because protected descendants could not be inspected: {error}"
                    )),
                    Some(503),
                );
            }
        };

        for entry in entries {
            if entry.is_symlink {
                continue;
            }
            inspected += 1;
            if inspected > MAX_RECURSIVE_INSPECTION_ENTRIES {
                return decision_result(
                    Some(context),
                    "Delete",
                    "recursive_delete",
                    root_path,
                    "deny_inspection_limit",
                    false,
                    Some(&state),
                    None,
                    Some("Recursive delete was not applied because protected-path inspection exceeded its safety limit".to_string()),
                    Some(413),
                );
            }
            let entry_paths = vec![entry.path.clone()];
            if !can_mutate_agent_created_test_file(
                Some(&state),
                &entry_paths,
                "recursive_delete",
                false,
            ) {
                if let Some(violation) = find_violation_for_operation(
                    &state.constraints,
                    &entry.path,
                    "recursive_delete",
                ) {
                    return decision_result(
                        Some(context),
                        "Delete",
                        "recursive_delete",
                        &entry.path,
                        "deny",
                        false,
                        Some(&state),
                        Some(violation),
                        Some(violation_message(&entry.path, violation)),
                        Some(403),
                    );
                }
            }
            if entry.is_dir {
                pending.push(entry.path);
            }
        }
    }
    None
}

fn check_local_recursive_delete(
    context: &ToolUseContext,
    root_path: &str,
    resolved_root: &str,
    state: &EditConstraintState,
) -> Option<ValidationResult> {
    let root = Path::new(resolved_root);
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) => {
            return decision_result(
                Some(context),
                "Delete",
                "recursive_delete",
                root_path,
                "deny_inspection_failed",
                false,
                Some(state),
                None,
                Some(format!(
                    "Recursive delete was not applied because the target could not be inspected: {error}"
                )),
                Some(503),
            );
        }
    };
    if !metadata.is_dir() {
        return None;
    }

    let mut pending = vec![root.to_path_buf()];
    let mut inspected = 0usize;
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                return decision_result(
                    Some(context),
                    "Delete",
                    "recursive_delete",
                    root_path,
                    "deny_inspection_failed",
                    false,
                    Some(state),
                    None,
                    Some(format!(
                        "Recursive delete was not applied because protected descendants could not be inspected: {error}"
                    )),
                    Some(503),
                );
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    return decision_result(
                        Some(context),
                        "Delete",
                        "recursive_delete",
                        root_path,
                        "deny_inspection_failed",
                        false,
                        Some(state),
                        None,
                        Some(format!(
                            "Recursive delete was not applied because a descendant could not be inspected: {error}"
                        )),
                        Some(503),
                    );
                }
            };
            let path = entry.path();
            let entry_metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    return decision_result(
                        Some(context),
                        "Delete",
                        "recursive_delete",
                        root_path,
                        "deny_inspection_failed",
                        false,
                        Some(state),
                        None,
                        Some(format!(
                            "Recursive delete was not applied because a descendant type could not be inspected: {error}"
                        )),
                        Some(503),
                    );
                }
            };
            if entry_metadata.file_type().is_symlink() {
                continue;
            }
            inspected += 1;
            if inspected > MAX_RECURSIVE_INSPECTION_ENTRIES {
                return decision_result(
                    Some(context),
                    "Delete",
                    "recursive_delete",
                    root_path,
                    "deny_inspection_limit",
                    false,
                    Some(state),
                    None,
                    Some("Recursive delete was not applied because protected-path inspection exceeded its safety limit".to_string()),
                    Some(413),
                );
            }
            let path_string = path.to_string_lossy().to_string();
            let entry_paths = vec![path_string.clone()];
            if !can_mutate_agent_created_test_file(
                Some(state),
                &entry_paths,
                "recursive_delete",
                false,
            ) {
                if let Some(violation) = find_violation_for_operation(
                    &state.constraints,
                    &path_string,
                    "recursive_delete",
                ) {
                    return decision_result(
                        Some(context),
                        "Delete",
                        "recursive_delete",
                        &path_string,
                        "deny",
                        false,
                        Some(state),
                        Some(violation),
                        Some(violation_message(&path_string, violation)),
                        Some(403),
                    );
                }
            }
            if entry_metadata.is_dir() {
                pending.push(path);
            }
        }
    }
    None
}

/// Records that a path was first created through a direct agent file tool.
/// This provenance is persisted with the session so later Write/Edit/Delete
/// calls can clean up the helper without being confused for repository tests.
pub async fn remember_agent_created_file(context: &ToolUseContext, file_path: &str) {
    let Some(session_id) = context.session_id.as_deref() else {
        return;
    };
    let paths = candidate_paths(Some(context), file_path);
    if let Some(coordinator) = get_global_coordinator() {
        coordinator
            .get_session_manager()
            .remember_edit_constraint_agent_created_paths(
                session_id,
                paths.clone(),
                context.dialog_turn_id.as_deref().unwrap_or_default(),
            )
            .await;
    }
    append_tool_telemetry(
        context,
        &json!({
            "event": "session_file_origin",
            "schema_version": EDIT_CONSTRAINT_SCHEMA_VERSION,
            "timestamp_ms": timestamp_ms(),
            "session_id": context.session_id,
            "dialog_turn_id": context.dialog_turn_id,
            "tool_call_id": context.tool_call_id,
            "requested_path": file_path,
            "resolved_path": resolved_path(context, file_path),
            "origin": "agent_created",
        }),
    );
}

/// Clears agent-created provenance after a successful direct delete.
pub async fn forget_agent_created_file(context: &ToolUseContext, file_path: &str) {
    let Some(session_id) = context.session_id.as_deref() else {
        return;
    };
    let paths = candidate_paths(Some(context), file_path);
    if let Some(coordinator) = get_global_coordinator() {
        coordinator
            .get_session_manager()
            .forget_edit_constraint_agent_created_paths_under(session_id, paths.clone())
            .await;
    }
    append_tool_telemetry(
        context,
        &json!({
            "event": "session_file_origin",
            "schema_version": EDIT_CONSTRAINT_SCHEMA_VERSION,
            "timestamp_ms": timestamp_ms(),
            "session_id": context.session_id,
            "dialog_turn_id": context.dialog_turn_id,
            "tool_call_id": context.tool_call_id,
            "requested_path": file_path,
            "resolved_path": resolved_path(context, file_path),
            "origin": "removed",
        }),
    );
}

/// Records a successful direct mutation. Offline evaluation can join these
/// events with the final patch and mark paths with no matching event as
/// unattributed; this function never blocks or repairs the patch.
pub fn record_mutation_applied(
    context: &ToolUseContext,
    tool_name: &str,
    operation: &str,
    file_path: &str,
) {
    append_tool_telemetry(
        context,
        &json!({
            "event": "mutation_applied",
            "schema_version": EDIT_CONSTRAINT_SCHEMA_VERSION,
            "mutation_id": Uuid::new_v4().to_string(),
            "timestamp_ms": timestamp_ms(),
            "session_id": context.session_id,
            "dialog_turn_id": context.dialog_turn_id,
            "tool_call_id": context.tool_call_id,
            "agent_type": context.agent_type,
            "tool_name": tool_name,
            "operation": operation,
            "requested_path": file_path,
            "resolved_path": resolved_path(context, file_path),
            "workspace_kind": if context.is_remote() { "remote" } else { "local" },
        }),
    );
}

#[cfg(test)]
mod tests;
