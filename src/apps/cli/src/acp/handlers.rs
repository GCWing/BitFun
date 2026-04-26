//! ACP Request Handlers
//!
//! Implements handlers for all ACP methods.

use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, Stdout};

use crate::acp::protocol::*;
use crate::acp::session::{AcpSession, AcpSessionManager};
use crate::agent::AgenticSystem;
use bitfun_core::agentic::coordination::{DialogSubmissionPolicy, DialogTriggerSource};
use bitfun_core::agentic::core::SessionConfig;
use bitfun_core::agentic::tools::framework::{ToolResult, ToolUseContext};
use bitfun_events::{AgenticEvent as CoreEvent, ToolEventData};

/// Handle an ACP method call
pub async fn handle_method(
    request: JsonRpcRequest,
    agentic_system: &AgenticSystem,
    session_manager: &Arc<AcpSessionManager>,
) -> Result<Option<JsonRpcResponse>> {
    let method = request.method.as_str();

    tracing::info!("Handling ACP method: {}", method);

    let result = match method {
        // Lifecycle methods
        "initialize" => handle_initialize(&request)?,
        "authenticate" => handle_authenticate(&request)?,

        // Session methods
        "session/new" => handle_session_new(&request, session_manager, agentic_system).await?,
        "session/load" => handle_session_load(&request)?,
        "session/prompt" => {
            handle_session_prompt(&request, agentic_system, session_manager).await?
        }
        "session/cancel" => {
            // Notification - no response
            handle_session_cancel(&request, session_manager).await?;
            return Ok(None);
        }
        "session/list" => handle_session_list(&request, session_manager)?,

        // Tools methods
        "tools/list" => handle_tools_list(&request, agentic_system).await?,
        "tools/call" => handle_tools_call(&request, agentic_system, session_manager).await?,

        // Config methods
        "session/set_config_option" => handle_set_config_option(&request)?,
        "session/set_mode" => handle_set_mode(&request)?,

        // Unknown method
        _ => {
            if let Some(id) = request.id {
                return Ok(Some(JsonRpcResponse::error(
                    id,
                    -32601,
                    format!("Method not found: {}", method),
                )));
            } else {
                // Notification for unknown method, just ignore
                return Ok(None);
            }
        }
    };

    if let Some(id) = request.id {
        Ok(Some(JsonRpcResponse::success(id, result)))
    } else {
        // Notification
        Ok(None)
    }
}

/// Handle initialize request
fn handle_initialize(request: &JsonRpcRequest) -> Result<serde_json::Value> {
    let params: InitializeParams = serde_json::from_value(
        request
            .params
            .as_ref()
            .ok_or_else(|| anyhow!("Missing params for initialize"))?
            .clone(),
    )
    .context("Failed to parse initialize params")?;

    tracing::info!(
        "ACP initialization: protocol_version={}, client_name={:?}",
        params.protocol_version,
        params.client_info.as_ref().map(|i| &i.name)
    );

    let result = InitializeResult {
        protocol_version: "0.1.0".to_string(), // ACP protocol version
        agent_capabilities: AgentCapabilities {
            load_session: true,
            mcp_capabilities: McpCapabilities {
                http: true,
                sse: true,
            },
            prompt_capabilities: PromptCapabilities {
                audio: false,
                embedded_context: true,
                image: true,
            },
            session_capabilities: SessionCapabilities { list: true },
        },
        agent_info: Some(AgentInfo {
            name: "BitFun".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
        auth_methods: vec![], // No authentication required
    };

    Ok(serde_json::to_value(result)?)
}

/// Handle authenticate request
fn handle_authenticate(_request: &JsonRpcRequest) -> Result<serde_json::Value> {
    // BitFun doesn't require authentication
    Ok(serde_json::json!({ "success": true }))
}

/// Handle session/new request
async fn handle_session_new(
    request: &JsonRpcRequest,
    session_manager: &Arc<AcpSessionManager>,
    agentic_system: &AgenticSystem,
) -> Result<serde_json::Value> {
    let params: SessionNewParams = serde_json::from_value(
        request
            .params
            .as_ref()
            .ok_or_else(|| anyhow!("Missing params for session/new"))?
            .clone(),
    )
    .context("Failed to parse session/new params")?;

    tracing::info!("Creating new ACP session: cwd={}", params.cwd);

    // Create ACP session
    let client_caps = ClientCapabilities::default(); // TODO: Get from previous initialize
    let acp_session = session_manager.create_session(params.cwd.clone(), client_caps)?;

    // Create a BitFun session via ConversationCoordinator
    let workspace_path = Some(params.cwd.clone());
    let session = agentic_system
        .coordinator
        .create_session(
            format!(
                "ACP Session - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ),
            "agentic".to_string(),
            SessionConfig {
                workspace_path,
                ..Default::default()
            },
        )
        .await?;

    // Update the ACP session with the real BitFun session ID
    session_manager
        .update_bitfun_session_id(&acp_session.acp_session_id, session.session_id.clone());

    tracing::info!(
        "Created BitFun session for ACP: acp_id={}, bitfun_id={}",
        acp_session.acp_session_id,
        session.session_id
    );

    let result = SessionNewResult {
        session_id: acp_session.acp_session_id.clone(),
        config_options: Some(vec![]), // No special config options
        modes: Some(SessionModes {
            available_modes: vec![
                ModeInfo {
                    id: "ask".to_string(),
                    name: Some("Ask".to_string()),
                    description: Some("Ask questions and get information".to_string()),
                },
                ModeInfo {
                    id: "architect".to_string(),
                    name: Some("Architect".to_string()),
                    description: Some("Design and plan architecture".to_string()),
                },
                ModeInfo {
                    id: "code".to_string(),
                    name: Some("Code".to_string()),
                    description: Some("Write and modify code".to_string()),
                },
            ],
            current_mode: Some("code".to_string()),
        }),
    };

    Ok(serde_json::to_value(result)?)
}

/// Handle session/load request
fn handle_session_load(_request: &JsonRpcRequest) -> Result<serde_json::Value> {
    // TODO: Implement session loading
    Err(anyhow!("Session loading not yet implemented"))
}

/// Handle session/prompt request - executes user message with BitFun's agentic system
async fn handle_session_prompt(
    request: &JsonRpcRequest,
    agentic_system: &AgenticSystem,
    session_manager: &Arc<AcpSessionManager>,
) -> Result<serde_json::Value> {
    let params: SessionPromptParams = serde_json::from_value(
        request
            .params
            .as_ref()
            .ok_or_else(|| anyhow!("Missing params for session/prompt"))?
            .clone(),
    )
    .context("Failed to parse session/prompt params")?;

    tracing::info!(
        "Processing session/prompt: session_id={}, prompt_blocks={}",
        params.session_id,
        params.prompt.len()
    );

    // Get ACP session
    let acp_session = session_manager
        .get_session(&params.session_id)
        .ok_or_else(|| anyhow!("Session not found: {}", params.session_id))?;

    // Extract text from prompt content blocks
    let user_message = params
        .prompt
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if user_message.is_empty() {
        return Err(anyhow!("Empty user message"));
    }

    tracing::info!(
        "User message for session {}: {}",
        acp_session.bitfun_session_id,
        user_message.chars().take(100).collect::<String>()
    );

    // Get stdout for sending notifications
    let stdout = tokio::io::stdout();

    // Execute the message through ConversationCoordinator
    let result = execute_prompt_turn(agentic_system, &acp_session, user_message, stdout).await?;

    Ok(serde_json::to_value(result)?)
}

/// Execute a prompt turn using BitFun's ConversationCoordinator
async fn execute_prompt_turn(
    agentic_system: &AgenticSystem,
    acp_session: &AcpSession,
    user_message: String,
    mut stdout: Stdout,
) -> Result<SessionPromptResult> {
    let session_id = acp_session.bitfun_session_id.clone();
    let agent_type = "agentic".to_string();

    tracing::info!("Starting dialog turn for session: {}", session_id);

    // Start the dialog turn
    agentic_system
        .coordinator
        .start_dialog_turn(
            session_id.clone(),
            user_message.clone(),
            None,
            None,
            agent_type.clone(),
            Some(acp_session.cwd.clone()),
            DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
        )
        .await?;

    let event_queue = agentic_system.event_queue.clone();
    let mut stop_reason: Option<StopReason> = None;
    let mut accumulated_text = String::new();

    loop {
        let events = event_queue.dequeue_batch(10).await;

        if events.is_empty() {
            if stop_reason.is_some() {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            continue;
        }

        for envelope in events {
            let event = envelope.event;

            if event.session_id() != Some(&session_id) {
                continue;
            }

            match event {
                CoreEvent::TextChunk { text, .. } => {
                    accumulated_text.push_str(&text);
                    let notification = SessionUpdateNotification {
                        session_id: acp_session.acp_session_id.clone(),
                        update: SessionUpdate::AgentMessageChunk {
                            content: ContentBlock::Text { text },
                        },
                    };
                    send_notification(&mut stdout, "session/update", &notification).await?;
                }

                CoreEvent::ToolEvent { tool_event, .. } => {
                    handle_tool_event(&mut stdout, &acp_session.acp_session_id, tool_event).await?;
                }

                CoreEvent::DialogTurnCompleted { .. } => {
                    tracing::info!("Dialog turn completed in ACP handler");
                    stop_reason = Some(StopReason::EndTurn);
                    break;
                }

                CoreEvent::DialogTurnFailed { error, .. } => {
                    tracing::error!("Dialog turn failed: {}", error);
                    stop_reason = Some(StopReason::Error);
                    let notification = SessionUpdateNotification {
                        session_id: acp_session.acp_session_id.clone(),
                        update: SessionUpdate::AgentMessageChunk {
                            content: ContentBlock::Text {
                                text: format!("Error: {}", error),
                            },
                        },
                    };
                    send_notification(&mut stdout, "session/update", &notification).await?;
                    break;
                }

                CoreEvent::SystemError { error, .. } => {
                    tracing::error!("System error: {}", error);
                    stop_reason = Some(StopReason::Error);
                    break;
                }

                _ => {
                    tracing::debug!("Ignoring event: {:?}", event);
                }
            }
        }
    }

    tracing::info!(
        "Dialog turn finished: stop_reason={:?}, text_len={}",
        stop_reason,
        accumulated_text.len(),
    );

    Ok(SessionPromptResult {
        stop_reason: stop_reason.unwrap_or(StopReason::EndTurn),
    })
}

/// Handle tool event and send appropriate notification
async fn handle_tool_event(
    stdout: &mut Stdout,
    session_id: &str,
    tool_event: ToolEventData,
) -> Result<()> {
    match tool_event {
        ToolEventData::Started {
            tool_id,
            tool_name,
            params: _,
        } => {
            let notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolCall {
                    tool_call_id: tool_id,
                    name: tool_name,
                    title: None,
                    kind: None,
                    status: Some(ToolCallStatus::Pending),
                },
            };
            send_notification(stdout, "session/update", &notification).await?;
        }

        ToolEventData::Progress {
            tool_id,
            tool_name,
            message,
            percentage: _,
        } => {
            let notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolCall {
                    tool_call_id: tool_id.clone(),
                    name: tool_name.clone(),
                    title: None,
                    kind: None,
                    status: Some(ToolCallStatus::InProgress),
                },
            };
            send_notification(stdout, "session/update", &notification).await?;

            // Send tool result with progress message
            let result_notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolResult {
                    tool_call_id: tool_id,
                    content: vec![ToolResultContent::Text { text: message }],
                    status: Some(ToolCallStatus::InProgress),
                },
            };
            send_notification(stdout, "session/update", &result_notification).await?;
        }

        ToolEventData::Completed {
            tool_id,
            tool_name,
            result,
            duration_ms: _,
            ..
        } => {
            let result_text =
                serde_json::to_string(&result).unwrap_or_else(|_| "Success".to_string());

            let notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolCall {
                    tool_call_id: tool_id.clone(),
                    name: tool_name.clone(),
                    title: None,
                    kind: None,
                    status: Some(ToolCallStatus::Completed),
                },
            };
            send_notification(stdout, "session/update", &notification).await?;

            // Send tool result
            let result_notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolResult {
                    tool_call_id: tool_id,
                    content: vec![ToolResultContent::Text { text: result_text }],
                    status: Some(ToolCallStatus::Completed),
                },
            };
            send_notification(stdout, "session/update", &result_notification).await?;
        }

        ToolEventData::Failed {
            tool_id,
            tool_name,
            error,
        } => {
            let notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolCall {
                    tool_call_id: tool_id.clone(),
                    name: tool_name.clone(),
                    title: None,
                    kind: None,
                    status: None, // Failed - no specific status
                },
            };
            send_notification(stdout, "session/update", &notification).await?;

            // Send tool result with error
            let result_notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolResult {
                    tool_call_id: tool_id,
                    content: vec![ToolResultContent::Text { text: error }],
                    status: None, // Failed
                },
            };
            send_notification(stdout, "session/update", &result_notification).await?;
        }

        ToolEventData::ConfirmationNeeded {
            tool_id,
            tool_name,
            params: _,
        } => {
            let notification = SessionUpdateNotification {
                session_id: session_id.to_string(),
                update: SessionUpdate::ToolCall {
                    tool_call_id: tool_id,
                    name: tool_name,
                    title: None,
                    kind: None,
                    status: Some(ToolCallStatus::Pending),
                },
            };
            send_notification(stdout, "session/update", &notification).await?;
        }

        _ => {
            // Ignore other tool events for now
        }
    }

    Ok(())
}

/// Send a JSON-RPC notification via stdout
async fn send_notification(
    stdout: &mut Stdout,
    method: &str,
    params: &impl serde::Serialize,
) -> Result<()> {
    let notification = JsonRpcRequest::new(
        None,
        method.to_string(),
        Some(serde_json::to_value(params)?),
    );

    let notification_json = serde_json::to_string(&notification)?;
    tracing::debug!("Sending notification: {}", notification_json);

    stdout.write_all(notification_json.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;

    Ok(())
}

/// Handle session/cancel notification
async fn handle_session_cancel(
    _request: &JsonRpcRequest,
    _session_manager: &Arc<AcpSessionManager>,
) -> Result<()> {
    tracing::info!("Received session/cancel notification");

    // TODO: Implement cancellation
    // 1. Stop ongoing model requests
    // 2. Abort tool invocations
    // 3. Send pending session/update notifications
    // 4. Mark session as cancelled

    Ok(())
}

/// Handle session/list request
fn handle_session_list(
    _request: &JsonRpcRequest,
    session_manager: &Arc<AcpSessionManager>,
) -> Result<serde_json::Value> {
    let sessions = session_manager.list_sessions();

    let session_infos: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "sessionId": s.acp_session_id,
                "cwd": s.cwd,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "sessions": session_infos,
    }))
}

/// Handle tools/list request
async fn handle_tools_list(
    _request: &JsonRpcRequest,
    _agentic_system: &AgenticSystem,
) -> Result<serde_json::Value> {
    tracing::info!("Listing available tools");

    // Get tools from BitFun's tool registry
    let registry = bitfun_core::agentic::tools::registry::get_global_tool_registry();
    let registry_lock = registry.read().await;
    let all_tools = registry_lock.get_all_tools();

    // Build tool definitions (need to await description)
    let mut tools: Vec<ToolDefinition> = Vec::new();
    for tool in all_tools.iter() {
        let desc = tool
            .description()
            .await
            .map_err(|e| anyhow!("Failed to get tool description: {}", e))?;
        tools.push(ToolDefinition {
            name: tool.name().to_string(),
            description: Some(desc),
            input_schema: Some(tool.input_schema().clone()),
        });
    }

    let result = ToolsListResult { tools };

    Ok(serde_json::to_value(result)?)
}

/// Handle tools/call request - executes a tool directly
async fn handle_tools_call(
    request: &JsonRpcRequest,
    agentic_system: &AgenticSystem,
    session_manager: &Arc<AcpSessionManager>,
) -> Result<serde_json::Value> {
    let params: ToolsCallParams = serde_json::from_value(
        request
            .params
            .as_ref()
            .ok_or_else(|| anyhow!("Missing params for tools/call"))?
            .clone(),
    )
    .context("Failed to parse tools/call params")?;

    tracing::info!(
        "Tool call request: session_id={}, tool_name={}",
        params.session_id,
        params.name
    );

    // Get ACP session
    let acp_session = session_manager
        .get_session(&params.session_id)
        .ok_or_else(|| anyhow!("Session not found: {}", params.session_id))?;

    // Get tool from registry
    let registry = bitfun_core::agentic::tools::registry::get_global_tool_registry();
    let registry_lock = registry.read().await;
    let tool = registry_lock
        .get_tool(&params.name)
        .ok_or_else(|| anyhow!("Tool not found: {}", params.name))?;

    // Create tool use context
    let context = ToolUseContext {
        tool_call_id: None,
        agent_type: Some("agentic".to_string()),
        session_id: Some(acp_session.bitfun_session_id.clone()),
        dialog_turn_id: None,
        workspace: None,
        custom_data: std::collections::HashMap::new(),
        computer_use_host: None,
        cancellation_token: None,
        runtime_tool_restrictions: Default::default(),
        workspace_services: None,
    };

    tracing::info!(
        "Executing tool {} with arguments: {:?}",
        params.name,
        params.arguments
    );

    // Execute the tool
    let tool_results = tool
        .call(&params.arguments, &context)
        .await
        .map_err(|e| anyhow!("Tool execution failed: {}", e))?;

    // Convert tool results to ACP content blocks
    let content: Vec<ToolResultContent> = tool_results
        .into_iter()
        .filter_map(|result| {
            match result {
                ToolResult::Result {
                    data,
                    result_for_assistant,
                    ..
                } => {
                    // Use result_for_assistant if available, otherwise serialize data
                    let text = result_for_assistant.unwrap_or_else(|| {
                        serde_json::to_string(&data).unwrap_or_else(|_| "Success".to_string())
                    });
                    Some(ToolResultContent::Text { text })
                }
                ToolResult::Progress { content: data, .. } => {
                    let text =
                        serde_json::to_string(&data).unwrap_or_else(|_| "Progress".to_string());
                    Some(ToolResultContent::Text { text })
                }
                ToolResult::StreamChunk { data, .. } => {
                    let text =
                        serde_json::to_string(&data).unwrap_or_else(|_| "Stream chunk".to_string());
                    Some(ToolResultContent::Text { text })
                }
            }
        })
        .collect();

    let result = ToolsCallResult { content };

    Ok(serde_json::to_value(result)?)
}

/// Handle session/set_config_option request
fn handle_set_config_option(_request: &JsonRpcRequest) -> Result<serde_json::Value> {
    // TODO: Implement config options
    Ok(serde_json::json!({ "success": true }))
}

/// Handle session/set_mode request
fn handle_set_mode(_request: &JsonRpcRequest) -> Result<serde_json::Value> {
    // TODO: Implement mode switching
    Ok(serde_json::json!({ "success": true }))
}
