//! Shell Integration Module
//!
//! This module provides OSC 633 sequence parsing for shell integration,
//! enabling command detection, exit code retrieval, and streaming output.

use std::collections::HashMap;
use std::sync::Arc;

use log::warn;
use tokio::sync::{mpsc, RwLock};

use crate::shell::ShellType;

/// OSC 633 sequence types for shell integration
#[derive(Debug, Clone, PartialEq)]
pub enum OscSequence {
    /// 633;A - Prompt started
    PromptStart,
    /// 633;B - Command input started (prompt ended)
    CommandInputStart,
    /// 633;C - Command execution started
    CommandExecutionStart,
    /// 633;D[;exitCode] - Command finished with optional exit code
    CommandFinished { exit_code: Option<i32> },
    /// 633;E;commandLine[;nonce] - Command line content
    CommandLine {
        command: String,
        nonce: Option<String>,
    },
    /// 633;F - Continuation prompt start
    ContinuationStart,
    /// 633;G - Continuation prompt end
    ContinuationEnd,
    /// 633;H - Right prompt start
    RightPromptStart,
    /// 633;I - Right prompt end
    RightPromptEnd,
    /// 633;P;property=value - Property
    Property { key: String, value: String },
}

/// Command execution state
#[derive(Debug, Clone, PartialEq)]
#[derive(Default)]
pub enum CommandState {
    /// Waiting for prompt
    #[default]
    Idle,
    /// Prompt is being displayed
    Prompt,
    /// User is inputting command
    Input,
    /// Command is executing
    Executing,
    /// Command has finished (but may still have pending output)
    Finished { exit_code: Option<i32> },
}

impl CommandState {
    /// Check if we should still collect output (executing or just finished)
    ///
    /// Note: This only checks the state itself. `ShellIntegration::should_collect_output()`
    /// also considers the `post_command_collecting` flag for ConPTY late output.
    pub fn should_collect_output(&self) -> bool {
        matches!(
            self,
            CommandState::Executing | CommandState::Finished { .. }
        )
    }
}


/// Event emitted by shell integration
#[derive(Debug, Clone)]
pub enum ShellIntegrationEvent {
    /// Command started executing
    CommandStarted { command: String, command_id: String },
    /// Command finished with exit code
    CommandFinished {
        command_id: String,
        exit_code: Option<i32>,
    },
    /// Current working directory changed
    CwdChanged { cwd: String },
    /// Shell property changed
    PropertyChanged { key: String, value: String },
    /// Output data received during command execution
    OutputData { command_id: String, data: String },
}

/// Shell integration parser and state tracker
pub struct ShellIntegration {
    /// Current command state
    state: CommandState,
    /// Current command ID
    current_command_id: Option<String>,
    /// Current command line
    current_command: Option<String>,
    /// Accumulated output for current command
    output_buffer: String,
    /// Current working directory
    cwd: Option<String>,
    /// Shell properties
    properties: HashMap<String, String>,
    /// Whether rich command detection is supported
    has_rich_detection: bool,
    /// Nonce for command verification
    nonce: Option<String>,
    /// Buffer for incomplete OSC sequences
    osc_buffer: String,
    /// Whether we're currently parsing an OSC sequence
    in_osc: bool,
    /// Last command's exit code (survives state transitions)
    last_exit_code: Option<i32>,
    /// Flag indicating a command just finished (for output collection)
    command_just_finished: bool,
    /// Flag for collecting late output after CommandFinished.
    /// On Windows, ConPTY may deliver rendered output AFTER shell integration
    /// sequences (CommandFinished/PromptStart/CommandInputStart). This flag
    /// keeps output collection active until the next CommandExecutionStart.
    post_command_collecting: bool,
    /// When true, we are between PromptStart and CommandInputStart,
    /// checking whether prompt text exists to detect ConPTY reordering.
    detecting_conpty_reorder: bool,
}

impl ShellIntegration {
    /// Create a new shell integration instance
    pub fn new() -> Self {
        Self {
            state: CommandState::default(),
            current_command_id: None,
            current_command: None,
            output_buffer: String::new(),
            cwd: None,
            properties: HashMap::new(),
            has_rich_detection: false,
            nonce: None,
            osc_buffer: String::new(),
            in_osc: false,
            last_exit_code: None,
            command_just_finished: false,
            post_command_collecting: false,
            detecting_conpty_reorder: false,
        }
    }

    /// Get the last command's exit code
    pub fn last_exit_code(&self) -> Option<i32> {
        self.last_exit_code
    }

    /// Check if a command just finished (for polling)
    pub fn command_just_finished(&self) -> bool {
        self.command_just_finished
    }

    /// Clear the command just finished flag
    pub fn clear_command_finished(&mut self) {
        self.command_just_finished = false;
    }

    /// Set the nonce for command verification
    pub fn set_nonce(&mut self, nonce: String) {
        self.nonce = Some(nonce);
    }

    /// Get current command state
    pub fn state(&self) -> &CommandState {
        &self.state
    }

    /// Get current working directory
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// Check if rich command detection is supported
    pub fn has_rich_detection(&self) -> bool {
        self.has_rich_detection
    }

    /// Check if output should be collected, considering both state and post-command flag.
    /// On Windows ConPTY, rendered output may arrive after shell integration sequences
    /// have already transitioned the state to Prompt/Input.
    fn should_collect(&self) -> bool {
        self.state.should_collect_output() || self.post_command_collecting
    }

    /// Get accumulated output for current command
    pub fn get_output(&self) -> &str {
        &self.output_buffer
    }

    /// Clear the output buffer
    pub fn clear_output(&mut self) {
        self.output_buffer.clear();
    }

    /// Process incoming data and extract events
    pub fn process_data(&mut self, data: &str) -> Vec<ShellIntegrationEvent> {
        let mut events = Vec::new();
        let mut plain_output = String::new();
        let mut chars = data.chars().peekable();

        while let Some(ch) = chars.next() {
            if self.in_osc {
                // Continue collecting OSC sequence
                if ch == '\x07' || (ch == '\\' && self.osc_buffer.ends_with('\x1b')) {
                    // End of OSC sequence
                    if ch == '\\' {
                        // Remove the ESC from buffer
                        self.osc_buffer.pop();
                    }

                    // Parse the OSC sequence
                    if let Some(seq) = self.parse_osc_sequence(&self.osc_buffer) {
                        // IMPORTANT: Before processing CommandFinished or PromptStart,
                        // flush any accumulated plain_output to the buffer while we still
                        // have the correct state and command_id
                        let should_flush = matches!(
                            seq,
                            OscSequence::CommandFinished { .. } | OscSequence::PromptStart
                        );
                        if should_flush && !plain_output.is_empty() && self.should_collect() {
                            self.output_buffer.push_str(&plain_output);
                            if let Some(cmd_id) = &self.current_command_id {
                                events.push(ShellIntegrationEvent::OutputData {
                                    command_id: cmd_id.clone(),
                                    data: std::mem::take(&mut plain_output),
                                });
                            } else {
                                plain_output.clear();
                            }
                        }

                        // ConPTY reorder detection: at CommandInputStart, if no
                        // prompt text accumulated since PromptStart, ConPTY sent
                        // the sequences before the rendered output. Re-enable
                        // post-command collection so late output is captured.
                        if self.detecting_conpty_reorder
                            && matches!(seq, OscSequence::CommandInputStart)
                        {
                            if plain_output.is_empty() {
                                self.post_command_collecting = true;
                            }
                            self.detecting_conpty_reorder = false;
                        }

                        if let Some(event) = self.handle_sequence(seq) {
                            events.push(event);
                        }
                    }

                    self.osc_buffer.clear();
                    self.in_osc = false;
                } else {
                    self.osc_buffer.push(ch);
                }
            } else if ch == '\x1b' {
                // Check for OSC start
                if chars.peek() == Some(&']') {
                    chars.next(); // consume ']'
                    self.in_osc = true;
                    self.osc_buffer.clear();
                } else {
                    // Not an OSC sequence, include the ESC in output
                    plain_output.push(ch);
                }
            } else {
                plain_output.push(ch);
            }
        }

        // Accumulate plain output if we should collect output.
        // Continue collecting after Finished via post_command_collecting flag,
        // because ConPTY may deliver rendered output after shell integration sequences.
        if !plain_output.is_empty() && self.should_collect() {
            self.output_buffer.push_str(&plain_output);

            if let Some(cmd_id) = &self.current_command_id {
                events.push(ShellIntegrationEvent::OutputData {
                    command_id: cmd_id.clone(),
                    data: plain_output,
                });
            }
        }

        events
    }

    /// Parse an OSC sequence string (without the ESC] prefix and terminator)
    fn parse_osc_sequence(&self, seq: &str) -> Option<OscSequence> {
        // OSC 633 sequences start with "633;"
        if !seq.starts_with("633;") {
            return None;
        }

        let content = &seq[4..]; // Skip "633;"
        let parts: Vec<&str> = content.splitn(3, ';').collect();

        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "A" => Some(OscSequence::PromptStart),
            "B" => Some(OscSequence::CommandInputStart),
            "C" => Some(OscSequence::CommandExecutionStart),
            "D" => {
                let exit_code = parts.get(1).and_then(|s| s.parse::<i32>().ok());
                Some(OscSequence::CommandFinished { exit_code })
            }
            "E" => {
                let command = parts
                    .get(1)
                    .map(|s| self.unescape_value(s))
                    .unwrap_or_default();
                let nonce = parts.get(2).map(|s| s.to_string());
                Some(OscSequence::CommandLine { command, nonce })
            }
            "F" => Some(OscSequence::ContinuationStart),
            "G" => Some(OscSequence::ContinuationEnd),
            "H" => Some(OscSequence::RightPromptStart),
            "I" => Some(OscSequence::RightPromptEnd),
            "P" => {
                // Property format: P;key=value
                if let Some(prop) = parts.get(1) {
                    if let Some((key, value)) = prop.split_once('=') {
                        return Some(OscSequence::Property {
                            key: key.to_string(),
                            value: self.unescape_value(value),
                        });
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Unescape a value from OSC sequence
    fn unescape_value(&self, value: &str) -> String {
        let mut result = String::new();
        let mut chars = value.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.peek() {
                    Some('\\') => {
                        chars.next();
                        result.push('\\');
                    }
                    Some('x') => {
                        chars.next();
                        // Read two hex digits
                        let hex: String = chars.by_ref().take(2).collect();
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                        }
                    }
                    _ => result.push(ch),
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Handle a parsed OSC sequence and optionally emit an event
    fn handle_sequence(&mut self, seq: OscSequence) -> Option<ShellIntegrationEvent> {
        match seq {
            OscSequence::PromptStart => {
                // When we see the next prompt, the previous command is truly done
                // Clear all state from previous command
                if self.post_command_collecting {
                    // Temporarily disable post-command collection.
                    // If no prompt text appears between PromptStart and
                    // CommandInputStart, ConPTY reordering is detected and
                    // collection will be re-enabled at CommandInputStart.
                    self.post_command_collecting = false;
                    self.detecting_conpty_reorder = true;
                }
                self.current_command_id = None;
                self.current_command = None;
                self.state = CommandState::Prompt;
                None
            }
            OscSequence::CommandInputStart => {
                self.state = CommandState::Input;
                None
            }
            OscSequence::CommandExecutionStart => {
                self.state = CommandState::Executing;
                self.output_buffer.clear();
                // Clear previous command's exit code when new command starts
                self.last_exit_code = None;
                self.command_just_finished = false;
                self.post_command_collecting = false;
                self.detecting_conpty_reorder = false;

                // Generate command ID if we have a command
                if self.current_command.is_some() {
                    let cmd_id = uuid::Uuid::new_v4().to_string();
                    self.current_command_id = Some(cmd_id.clone());

                    return Some(ShellIntegrationEvent::CommandStarted {
                        command: self.current_command.clone().unwrap_or_default(),
                        command_id: cmd_id,
                    });
                }
                None
            }
            OscSequence::CommandFinished { exit_code } => {
                // Set state to Finished but DON'T clear command_id yet
                // We may still receive output data until the next PromptStart
                self.state = CommandState::Finished { exit_code };

                // Save exit code - this survives state transitions
                self.last_exit_code = exit_code;
                self.command_just_finished = true;
                // Keep collecting output after finish — ConPTY may deliver
                // rendered output after the shell integration sequences.
                self.post_command_collecting = true;

                // Emit event but keep command_id for output collection
                let event = self.current_command_id.as_ref().map(|cmd_id| ShellIntegrationEvent::CommandFinished {
                        command_id: cmd_id.clone(),
                        exit_code,
                    });

                self.current_command = None;
                event
            }
            OscSequence::CommandLine { command, nonce } => {
                // Verify nonce if we have one set
                if let Some(expected_nonce) = &self.nonce {
                    if let Some(received_nonce) = &nonce {
                        if expected_nonce != received_nonce {
                            warn!(
                                "Nonce mismatch: expected {}, got {}",
                                expected_nonce, received_nonce
                            );
                        }
                    }
                }

                self.current_command = Some(command);
                None
            }
            OscSequence::Property { key, value } => {
                // debug!("Shell property: {} = {}", key, value);

                let event = match key.as_str() {
                    "Cwd" => {
                        self.cwd = Some(value.clone());
                        Some(ShellIntegrationEvent::CwdChanged { cwd: value.clone() })
                    }
                    "HasRichCommandDetection" => {
                        self.has_rich_detection = value == "True";
                        None
                    }
                    _ => Some(ShellIntegrationEvent::PropertyChanged {
                        key: key.clone(),
                        value: value.clone(),
                    }),
                };

                self.properties.insert(key, value);
                event
            }
            OscSequence::ContinuationStart
            | OscSequence::ContinuationEnd
            | OscSequence::RightPromptStart
            | OscSequence::RightPromptEnd => {
                // These are formatting hints, we don't need to emit events for them
                None
            }
        }
    }
}

impl Default for ShellIntegration {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for tracking shell integration across multiple sessions
pub struct ShellIntegrationManager {
    /// Integration instances per session
    integrations: Arc<RwLock<HashMap<String, ShellIntegration>>>,
    /// Event sender
    event_tx: mpsc::Sender<(String, ShellIntegrationEvent)>,
    /// Event receiver
    event_rx: Arc<RwLock<mpsc::Receiver<(String, ShellIntegrationEvent)>>>,
}

impl ShellIntegrationManager {
    /// Create a new shell integration manager
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(1024);
        Self {
            integrations: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
        }
    }

    /// Register a new session
    pub async fn register_session(&self, session_id: &str, nonce: Option<String>) {
        let mut integrations = self.integrations.write().await;
        let mut integration = ShellIntegration::new();
        if let Some(n) = nonce {
            integration.set_nonce(n);
        }
        integrations.insert(session_id.to_string(), integration);
    }

    /// Unregister a session
    pub async fn unregister_session(&self, session_id: &str) {
        let mut integrations = self.integrations.write().await;
        integrations.remove(session_id);
    }

    /// Process data for a session
    pub async fn process_data(&self, session_id: &str, data: &str) -> Vec<ShellIntegrationEvent> {
        let mut integrations = self.integrations.write().await;

        if let Some(integration) = integrations.get_mut(session_id) {
            let events = integration.process_data(data);

            // Send events through channel
            for event in &events {
                let _ = self
                    .event_tx
                    .send((session_id.to_string(), event.clone()))
                    .await;
            }

            events
        } else {
            Vec::new()
        }
    }

    /// Get the current state for a session
    pub async fn get_state(&self, session_id: &str) -> Option<CommandState> {
        let integrations = self.integrations.read().await;
        integrations.get(session_id).map(|i| i.state().clone())
    }

    /// Get the current working directory for a session
    pub async fn get_cwd(&self, session_id: &str) -> Option<String> {
        let integrations = self.integrations.read().await;
        integrations
            .get(session_id)
            .and_then(|i| i.cwd().map(|s| s.to_string()))
    }

    /// Get accumulated output for a session
    pub async fn get_output(&self, session_id: &str) -> Option<String> {
        let integrations = self.integrations.read().await;
        integrations
            .get(session_id)
            .map(|i| i.get_output().to_string())
    }

    /// Clear output buffer for a session
    pub async fn clear_output(&self, session_id: &str) {
        let mut integrations = self.integrations.write().await;
        if let Some(integration) = integrations.get_mut(session_id) {
            integration.clear_output();
        }
    }

    /// Receive the next event
    pub async fn recv_event(&self) -> Option<(String, ShellIntegrationEvent)> {
        let mut rx = self.event_rx.write().await;
        rx.recv().await
    }

    /// Get a clone of the event sender
    pub fn event_sender(&self) -> mpsc::Sender<(String, ShellIntegrationEvent)> {
        self.event_tx.clone()
    }
}

impl Default for ShellIntegrationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the path to shell integration script for a given shell type
pub fn get_integration_script_path(shell_type: &ShellType) -> Option<&'static str> {
    match shell_type {
        ShellType::Bash => Some("shellIntegration-bash.sh"),
        ShellType::Zsh => Some("shellIntegration-rc.zsh"),
        ShellType::Fish => Some("shellIntegration.fish"),
        ShellType::PowerShell | ShellType::PowerShellCore => Some("shellIntegration.ps1"),
        _ => None,
    }
}

/// Get the shell integration script content embedded in the binary
pub fn get_integration_script_content(shell_type: &ShellType) -> Option<&'static str> {
    match shell_type {
        ShellType::Bash => Some(include_str!("scripts/shellIntegration-bash.sh")),
        ShellType::Zsh => Some(include_str!("scripts/shellIntegration-rc.zsh")),
        ShellType::Fish => Some(include_str!("scripts/shellIntegration.fish")),
        ShellType::PowerShell | ShellType::PowerShellCore => {
            Some(include_str!("scripts/shellIntegration.ps1"))
        }
        _ => None,
    }
}

/// Generate shell command to inject shell integration
pub fn get_injection_command(shell_type: &ShellType, script_path: &str) -> Option<String> {
    match shell_type {
        ShellType::Bash => Some(format!(r#"source "{}""#, script_path.replace('\\', "/"))),
        ShellType::Zsh => Some(format!(r#"source "{}""#, script_path.replace('\\', "/"))),
        ShellType::Fish => Some(format!(r#"source "{}""#, script_path.replace('\\', "/"))),
        ShellType::PowerShell | ShellType::PowerShellCore => {
            Some(format!(r#". "{}""#, script_path.replace('/', "\\")))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prompt_start() {
        let mut integration = ShellIntegration::new();
        let events = integration.process_data("\x1b]633;A\x07");
        assert!(events.is_empty());
        assert_eq!(integration.state(), &CommandState::Prompt);
    }

    #[test]
    fn test_parse_command_finished_with_exit_code() {
        let mut integration = ShellIntegration::new();
        integration.state = CommandState::Executing;
        integration.current_command_id = Some("test-id".to_string());

        let events = integration.process_data("\x1b]633;D;0\x07");
        assert_eq!(events.len(), 1);

        if let ShellIntegrationEvent::CommandFinished {
            command_id,
            exit_code,
        } = &events[0]
        {
            assert_eq!(command_id, "test-id");
            assert_eq!(*exit_code, Some(0));
        } else {
            panic!("Expected CommandFinished event");
        }
    }

    #[test]
    fn test_parse_cwd_property() {
        let mut integration = ShellIntegration::new();
        let events = integration.process_data("\x1b]633;P;Cwd=/home/user\x07");

        assert_eq!(events.len(), 1);
        assert_eq!(integration.cwd(), Some("/home/user"));
    }

    #[test]
    fn test_parse_command_line() {
        let mut integration = ShellIntegration::new();
        let events = integration.process_data("\x1b]633;E;ls -la;nonce123\x07");

        assert!(events.is_empty()); // CommandLine doesn't emit event directly
        assert_eq!(integration.current_command, Some("ls -la".to_string()));
    }

    #[test]
    fn test_unescape_value() {
        let integration = ShellIntegration::new();

        assert_eq!(integration.unescape_value("hello"), "hello");
        assert_eq!(integration.unescape_value("hello\\\\world"), "hello\\world");
        assert_eq!(integration.unescape_value("hello\\x3bworld"), "hello;world");
    }
}
