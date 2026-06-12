//! MiniApp agent bridge API.
//!
//! Lets a MiniApp (gated by the `agent` permission group) run full host agent
//! turns — the complete agent loop with tools (WebSearch/WebFetch/Read/...)
//! and skills — instead of the raw single-call LLM access provided by the
//! `ai` permission group.
//!
//! Each run creates a hidden subagent session (invisible in the session list)
//! owned by `miniapp-agent:{app_id}:{run_id}` and submits exactly one dialog
//! turn through the standard `DialogScheduler`. Streaming output reaches the
//! MiniApp iframe through the normal `agentic://*` Tauri events, which the
//! web-ui MiniApp bridge filters by session id and forwards into the iframe.

use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::api::app_state::AppState;
use bitfun_core::agentic::coordination::{
    ConversationCoordinator, DialogScheduler, DialogSubmissionPolicy, DialogTriggerSource,
};
use bitfun_core::agentic::core::{MessageContent, MessageRole, SessionConfig};

// ============== Run registry ==============

#[derive(Debug, Clone)]
struct MiniAppAgentRunRecord {
    app_id: String,
    session_id: String,
    turn_id: String,
}

/// Active/recent agent runs: run_id → record. Used for ownership validation,
/// stale-run cancellation after a webview reload, and turn-text fallback.
static AGENT_RUN_REGISTRY: OnceLock<Mutex<HashMap<String, MiniAppAgentRunRecord>>> =
    OnceLock::new();

/// Per-app agent rate limiter state: app_id → (request_count, window_start_ms).
static AGENT_RATE_LIMITER: OnceLock<Mutex<HashMap<String, (u32, u64)>>> = OnceLock::new();

static AGENT_RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Cap the per-process registry so completed runs cannot grow it unboundedly.
const AGENT_RUN_REGISTRY_MAX: usize = 256;

fn agent_run_registry() -> &'static Mutex<HashMap<String, MiniAppAgentRunRecord>> {
    AGENT_RUN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn agent_rate_limiter() -> &'static Mutex<HashMap<String, (u32, u64)>> {
    AGENT_RATE_LIMITER.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn check_agent_rate_limit(app_id: &str, rate_limit_per_minute: u32) -> Result<(), String> {
    if rate_limit_per_minute == 0 {
        return Ok(());
    }
    let now = now_ms();
    let window_ms: u64 = 60_000;
    let mut map = agent_rate_limiter()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(app_id.to_string()).or_insert((0, now));
    if now - entry.1 >= window_ms {
        *entry = (1, now);
    } else {
        entry.0 += 1;
        if entry.0 > rate_limit_per_minute {
            return Err(format!(
                "Agent rate limit exceeded: max {} runs/minute",
                rate_limit_per_minute
            ));
        }
    }
    Ok(())
}

fn register_agent_run(record: MiniAppAgentRunRecord) {
    let mut registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if registry.len() >= AGENT_RUN_REGISTRY_MAX {
        // Drop an arbitrary old entry; the registry is a safety net, not a
        // source of truth, so losing the oldest record is acceptable.
        if let Some(key) = registry.keys().next().cloned() {
            registry.remove(&key);
        }
    }
    registry.insert(record.turn_id.clone(), record);
}

fn lookup_agent_run(
    app_id: &str,
    session_id: &str,
    turn_id: &str,
) -> Option<MiniAppAgentRunRecord> {
    let registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry
        .get(turn_id)
        .filter(|record| record.app_id == app_id && record.session_id == session_id)
        .cloned()
}

fn take_agent_runs_for_app(app_id: &str) -> Vec<MiniAppAgentRunRecord> {
    let mut registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let turn_ids: Vec<String> = registry
        .iter()
        .filter(|(_, record)| record.app_id == app_id)
        .map(|(turn_id, _)| turn_id.clone())
        .collect();
    turn_ids
        .into_iter()
        .filter_map(|turn_id| registry.remove(&turn_id))
        .collect()
}

fn remove_agent_run(turn_id: &str) {
    let mut registry = agent_run_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry.remove(turn_id);
}

async fn require_agent_permission(
    state: &AppState,
    app_id: &str,
) -> Result<bitfun_core::miniapp::AgentPermissions, String> {
    let app = state
        .miniapp_manager
        .get(app_id)
        .await
        .map_err(|e| e.to_string())?;
    let agent_perms = app
        .permissions
        .agent
        .clone()
        .ok_or("Agent access is not enabled for this MiniApp")?;
    if !agent_perms.enabled {
        return Err("Agent access is not enabled for this MiniApp".to_string());
    }
    Ok(agent_perms)
}

// ============== Request/Response DTOs ==============

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentRunRequest {
    pub app_id: String,
    /// Full user prompt for the agent turn. The MiniApp owns its own task
    /// protocol; the host only wraps it into a hidden agent session.
    pub prompt: String,
    /// Optional idempotency key reused as the turn id.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Optional human-readable session name for diagnostics.
    #[serde(default)]
    pub session_name: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentRunResponse {
    pub session_id: String,
    pub turn_id: String,
    pub action_run_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentCancelRequest {
    pub app_id: String,
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentTurnTextRequest {
    pub app_id: String,
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentTurnTextResponse {
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentCancelStaleRunsRequest {
    pub app_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentCancelStaleRunsResponse {
    pub cancelled_runs: u32,
}

// ============== Commands ==============

/// Start a full agent turn for a MiniApp inside a hidden subagent session.
#[tauri::command]
pub async fn miniapp_agent_run(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    scheduler: State<'_, Arc<DialogScheduler>>,
    request: MiniAppAgentRunRequest,
) -> Result<MiniAppAgentRunResponse, String> {
    if request.prompt.trim().is_empty() {
        return Err("prompt is required".to_string());
    }
    let agent_perms = require_agent_permission(&state, &request.app_id).await?;
    check_agent_rate_limit(
        &request.app_id,
        agent_perms.rate_limit_per_minute.unwrap_or(0),
    )?;

    let workspace_path = request
        .workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("workspacePath is required for MiniApp agent runs")?
        .to_string();

    let run_id = request
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "miniapp-agent-{}-{}",
                request.app_id,
                AGENT_RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
            )
        });
    let owner = format!("miniapp-agent:{}:{}", request.app_id, run_id);
    let session_name = request
        .session_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("MiniApp Agent Run")
        .to_string();

    // One hidden single-turn session per run so parallel runs never queue
    // behind each other and never pollute the visible session list.
    let config = SessionConfig {
        enable_tools: true,
        safe_mode: true,
        auto_compact: false,
        enable_context_compression: false,
        max_turns: 1,
        ..Default::default()
    };
    // Cowork is the office/collaboration mode: it is the only mode where the
    // office skill group (incl. ppt-design) is enabled by default, and its
    // toolset covers the research + file tools PPT generation needs.
    let session = coordinator
        .create_hidden_subagent_session_with_workspace(
            None,
            session_name,
            "Cowork".to_string(),
            config,
            workspace_path.clone(),
            Some(owner),
        )
        .await
        .map_err(|e| format!("Failed to create MiniApp agent session: {}", e))?;
    let session_id = session.session_id.clone();

    let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopApi)
        .with_skip_tool_confirmation(true);
    let metadata = json!({
        "surface": "miniapp_agent",
        "appId": request.app_id,
        "runId": run_id,
    });

    let outcome = scheduler
        .submit(
            session_id.clone(),
            request.prompt.clone(),
            Some("MiniApp agent run".to_string()),
            Some(run_id.clone()),
            "Cowork".to_string(),
            Some(workspace_path),
            policy,
            None,
            Some(metadata),
            None,
        )
        .await
        .map_err(|e| format!("Failed to start MiniApp agent turn: {}", e))?;

    let status = match outcome {
        bitfun_core::agentic::coordination::DialogSubmitOutcome::Started { .. } => "started",
        bitfun_core::agentic::coordination::DialogSubmitOutcome::Queued { .. } => "queued",
    };

    register_agent_run(MiniAppAgentRunRecord {
        app_id: request.app_id.clone(),
        session_id: session_id.clone(),
        turn_id: run_id.clone(),
    });

    Ok(MiniAppAgentRunResponse {
        session_id,
        turn_id: run_id.clone(),
        action_run_id: run_id,
        status: status.to_string(),
    })
}

/// Cancel a running MiniApp agent turn.
#[tauri::command]
pub async fn miniapp_agent_cancel(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: MiniAppAgentCancelRequest,
) -> Result<(), String> {
    require_agent_permission(&state, &request.app_id).await?;
    if lookup_agent_run(&request.app_id, &request.session_id, &request.turn_id).is_none() {
        return Err("Unknown MiniApp agent run".to_string());
    }
    coordinator
        .cancel_dialog_turn(&request.session_id, &request.turn_id)
        .await
        .map_err(|e| e.to_string())?;
    remove_agent_run(&request.turn_id);
    Ok(())
}

/// Read the assistant text of a (completed) MiniApp agent turn from the live
/// in-memory session. Used by MiniApps as a fallback when streaming was
/// interrupted (for example a webview reload during generation).
#[tauri::command]
pub async fn miniapp_agent_turn_text(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: MiniAppAgentTurnTextRequest,
) -> Result<MiniAppAgentTurnTextResponse, String> {
    require_agent_permission(&state, &request.app_id).await?;
    if lookup_agent_run(&request.app_id, &request.session_id, &request.turn_id).is_none() {
        return Err("Unknown MiniApp agent run".to_string());
    }

    let messages = coordinator
        .get_session_manager()
        .get_context_messages(&request.session_id)
        .await
        .map_err(|e| e.to_string())?;
    let text = messages
        .iter()
        .rev()
        .filter(|message| message.role == MessageRole::Assistant)
        .find_map(|message| {
            let text = match &message.content {
                MessageContent::Text(text) => text.as_str(),
                MessageContent::Multimodal { text, .. } => text.as_str(),
                MessageContent::Mixed { text, .. } => text.as_str(),
                MessageContent::ToolResult { .. } => "",
            };
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        })
        .unwrap_or_default();

    Ok(MiniAppAgentTurnTextResponse { text })
}

/// Cancel every tracked agent run for the given MiniApp. Called by the app on
/// startup/recovery so webview reloads do not leave orphaned agent turns.
#[tauri::command]
pub async fn miniapp_agent_cancel_stale_runs(
    state: State<'_, AppState>,
    coordinator: State<'_, Arc<ConversationCoordinator>>,
    request: MiniAppAgentCancelStaleRunsRequest,
) -> Result<MiniAppAgentCancelStaleRunsResponse, String> {
    require_agent_permission(&state, &request.app_id).await?;

    let runs = take_agent_runs_for_app(&request.app_id);
    let mut cancelled = 0u32;
    for run in runs {
        match coordinator
            .cancel_dialog_turn(&run.session_id, &run.turn_id)
            .await
        {
            Ok(()) => cancelled += 1,
            Err(error) => {
                // Completed turns fail to cancel; that is the expected steady state.
                warn!(
                    "MiniApp agent stale-run cancel skipped: app_id={}, session_id={}, turn_id={}, error={}",
                    run.app_id, run.session_id, run.turn_id, error
                );
            }
        }
    }

    Ok(MiniAppAgentCancelStaleRunsResponse {
        cancelled_runs: cancelled,
    })
}
