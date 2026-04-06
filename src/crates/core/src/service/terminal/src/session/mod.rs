//! Session module - Terminal session management
//!
//! This module handles terminal session lifecycle, persistence,
//! and serialization for recovery.

mod binding;
mod manager;
mod persistent;
mod serializer;
mod singleton;

pub use binding::{TerminalBindingOptions, TerminalSessionBinding};
pub use manager::{
    CommandCompletionReason, CommandExecuteResult, CommandStream, CommandStreamEvent,
    ExecuteOptions, SessionManager,
};
pub use persistent::PersistentSession;
pub use serializer::SessionSerializer;
pub use singleton::{
    get_session_manager, init_session_manager, is_session_manager_initialized, session_manager,
    set_session_manager,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::shell::ShellType;

/// Terminal session status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionStatus {
    /// Session is starting up
    #[default]
    Starting,
    /// Session is active and running
    Active,
    /// Session is orphaned (client disconnected)
    Orphaned,
    /// Session is being restored
    Restoring,
    /// Session has exited
    Exited { exit_code: Option<i32> },
    /// Session is being terminated
    Terminating,
}

/// Terminal session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSession {
    /// Unique session ID
    pub id: String,

    /// Display name
    pub name: String,

    /// Shell type
    pub shell_type: ShellType,

    /// Working directory
    pub cwd: String,

    /// Initial working directory
    pub initial_cwd: String,

    /// Session status
    pub status: SessionStatus,

    /// Process ID (if running)
    pub pid: Option<u32>,

    /// Internal PTY process ID
    pub pty_id: Option<u32>,

    /// Terminal dimensions
    pub cols: u16,
    pub rows: u16,

    /// Session creation time
    pub created_at: DateTime<Utc>,

    /// Last activity time
    pub last_activity: DateTime<Utc>,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Session metadata
    pub metadata: SessionMetadata,

    /// Session creation source
    #[serde(default)]
    pub source: SessionSource,

    /// Whether the session should persist
    pub should_persist: bool,

    /// Exit code if exited
    pub exit_code: Option<i32>,

    /// Output history buffer (for frontend recovery)
    /// Not serialized because it's only used during session lifetime
    #[serde(skip)]
    pub output_history: Vec<String>,

    /// Maximum size of output history (in bytes)
    #[serde(skip)]
    pub max_history_size: usize,
}

impl TerminalSession {
    /// Default maximum history size: 100KB
    const DEFAULT_MAX_HISTORY_SIZE: usize = 100 * 1024;

    /// Create a new terminal session
    pub fn new(
        id: String,
        name: String,
        shell_type: ShellType,
        cwd: String,
        cols: u16,
        rows: u16,
        source: SessionSource,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            name,
            shell_type,
            cwd: cwd.clone(),
            initial_cwd: cwd,
            status: SessionStatus::Starting,
            pid: None,
            pty_id: None,
            cols,
            rows,
            created_at: now,
            last_activity: now,
            env: HashMap::new(),
            metadata: SessionMetadata::default(),
            source,
            should_persist: true,
            exit_code: None,
            output_history: Vec::new(),
            max_history_size: Self::DEFAULT_MAX_HISTORY_SIZE,
        }
    }

    /// Add output to history (with automatic trimming)
    pub fn add_output(&mut self, data: &str) {
        if data.is_empty() {
            return;
        }
        self.output_history.push(data.to_string());
        self.trim_history();
    }

    /// Get all output history as a single string
    pub fn get_history(&self) -> String {
        self.output_history.concat()
    }

    /// Clear all output history
    pub fn clear_history(&mut self) {
        self.output_history.clear();
    }

    /// Trim history to stay within max size limit
    fn trim_history(&mut self) {
        let mut total_size: usize = self.output_history.iter().map(|s| s.len()).sum();

        // Remove oldest entries until we're under the limit
        while total_size > self.max_history_size && !self.output_history.is_empty() {
            if let Some(oldest) = self.output_history.first() {
                total_size -= oldest.len();
                self.output_history.remove(0);
            } else {
                break;
            }
        }
    }

    /// Get current history size in bytes
    pub fn history_size(&self) -> usize {
        self.output_history.iter().map(|s| s.len()).sum()
    }

    /// Check if the session is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Active)
    }

    /// Check if the session is orphaned
    pub fn is_orphaned(&self) -> bool {
        matches!(self.status, SessionStatus::Orphaned)
    }

    /// Check if the session has exited
    pub fn has_exited(&self) -> bool {
        matches!(self.status, SessionStatus::Exited { .. })
    }

    /// Update last activity time
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }

    /// Set the session as active with PID
    pub fn set_active(&mut self, pid: u32, pty_id: u32) {
        self.pid = Some(pid);
        self.pty_id = Some(pty_id);
        self.status = SessionStatus::Active;
        self.touch();
    }

    /// Set the session as exited
    pub fn set_exited(&mut self, exit_code: Option<i32>) {
        self.exit_code = exit_code;
        self.status = SessionStatus::Exited { exit_code };
        self.touch();
    }

    /// Set the session as orphaned
    pub fn set_orphaned(&mut self) {
        self.status = SessionStatus::Orphaned;
        self.touch();
    }

    /// Update working directory
    pub fn update_cwd(&mut self, cwd: String) {
        self.cwd = cwd;
        self.touch();
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.touch();
    }
}

/// Source that created the terminal session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionSource {
    #[default]
    Manual,
    Agent,
}

/// Session metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Session icon
    pub icon: Option<String>,

    /// Session color
    pub color: Option<String>,

    /// Custom title (overrides shell title)
    pub custom_title: Option<String>,

    /// Title source
    pub title_source: TitleSource,

    /// Whether the session was restored
    pub was_restored: bool,

    /// Shell integration status
    pub shell_integration: ShellIntegrationStatus,

    /// Owner information
    pub owner: Option<SessionOwner>,
}

/// Source of the terminal title
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TitleSource {
    /// Title from API (custom)
    Api,
    /// Title from process
    #[default]
    Process,
    /// Title from shell integration
    Sequence,
}

/// Shell integration status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShellIntegrationStatus {
    /// Whether shell integration is enabled
    pub enabled: bool,
    /// Whether shell integration was successfully activated
    pub activated: bool,
    /// Whether command detection is working
    pub command_detection: bool,
    /// Whether CWD detection is working
    pub cwd_detection: bool,
}

/// Session owner information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOwner {
    /// Owner ID (e.g., workspace ID)
    pub id: String,
    /// Owner type
    pub owner_type: OwnerType,
}

/// Type of session owner
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnerType {
    /// Owned by a workspace
    Workspace,
    /// Owned by an extension
    Extension,
    /// Owned by a user
    User,
    /// Standalone session
    Standalone,
}
