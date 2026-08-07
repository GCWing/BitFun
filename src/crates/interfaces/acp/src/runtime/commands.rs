use agent_client_protocol::schema::{
    AvailableCommand, AvailableCommandsUpdate, ContentChunk, PromptResponse, SessionUpdate,
    StopReason,
};
use agent_client_protocol::{Client, ConnectionTo, Result};

use super::events::send_update;
use super::AcpSessionState;

/// Returns the list of built-in slash commands advertised to ACP clients.
pub(super) fn builtin_commands() -> Vec<AvailableCommand> {
    vec![
        AvailableCommand::new("help", "Show available commands"),
        AvailableCommand::new("clear", "Clear the conversation context"),
        AvailableCommand::new("compact", "Compact and summarize the conversation context"),
        AvailableCommand::new("status", "Show current session status"),
    ]
}

/// Builds the [`SessionUpdate`] that advertises built-in commands to the client.
pub(super) fn builtin_commands_update() -> SessionUpdate {
    SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(builtin_commands()))
}

/// Sends the built-in commands list to the client.  Errors are logged and
/// discarded so that command advertisement never blocks session creation.
pub(super) fn advertise_builtin_commands(connection: &ConnectionTo<Client>, session_id: &str) {
    if let Err(error) = send_update(connection, session_id, builtin_commands_update()) {
        log::warn!(
            "Failed to advertise built-in commands to ACP client for session {}: {}",
            session_id,
            error
        );
    }
}

/// Attempts to handle a built-in slash command locally without forwarding the
/// prompt to the agent runtime.
///
/// Returns `Ok(Some(response))` when `user_message` is a recognized built-in
/// command that has been handled.  Returns `Ok(None)` when the message is not a
/// built-in command and should be submitted to the agent runtime normally.
pub(super) fn try_handle_builtin_command(
    connection: &ConnectionTo<Client>,
    session: &AcpSessionState,
    user_message: &str,
) -> Result<Option<PromptResponse>> {
    let trimmed = user_message.trim();
    if !trimmed.starts_with('/') {
        return Ok(None);
    }

    let command_name = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_start_matches('/')
        .to_lowercase();

    let response_text = match command_name.as_str() {
        "help" => format_help_text(),
        "status" => format_status_text(session),
        "clear" => "Context clearing is not yet available via slash command. \
                    Please use the client's clear button or the session/clear RPC method."
            .to_string(),
        "compact" => "Context compaction is not yet available via slash command.".to_string(),
        _ => return Ok(None),
    };

    send_update(
        connection,
        &session.acp_session_id,
        SessionUpdate::AgentMessageChunk(ContentChunk::new(response_text.into())),
    )?;

    Ok(Some(PromptResponse::new(StopReason::EndTurn)))
}

fn format_help_text() -> String {
    let commands = builtin_commands();
    let mut text = String::from("Available commands:\n\n");
    for cmd in &commands {
        text.push_str(&format!("/{} - {}\n", cmd.name, cmd.description));
    }
    text.push_str("\nType any command to execute it.");
    text
}

fn format_status_text(session: &AcpSessionState) -> String {
    format!(
        "Session Status:\n  Session ID: {}\n  Mode: {}\n  Model: {}\n  Working Directory: {}",
        session.acp_session_id, session.mode_id, session.model_id, session.cwd,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_commands_have_unique_names() {
        use std::collections::HashSet;
        let commands = builtin_commands();
        let names: HashSet<&str> = commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names.len(), commands.len(), "command names must be unique");
    }

    #[test]
    fn help_text_lists_all_commands() {
        let text = format_help_text();
        for cmd in &builtin_commands() {
            assert!(
                text.contains(&format!("/{}", cmd.name)),
                "help text must mention /{}",
                cmd.name
            );
        }
    }

    #[test]
    fn status_text_includes_session_fields() {
        let session = AcpSessionState {
            acp_session_id: "test-session".to_string(),
            bitfun_session_id: "test-bitfun".to_string(),
            cwd: "/tmp/work".to_string(),
            mode_id: "code".to_string(),
            model_id: "test-model".to_string(),
            mcp_server_ids: Vec::new(),
            lifecycle: std::sync::Arc::new(tokio::sync::Mutex::new(())),
        };
        let text = format_status_text(&session);
        assert!(text.contains("test-session"));
        assert!(text.contains("code"));
        assert!(text.contains("test-model"));
        assert!(text.contains("/tmp/work"));
    }
}
