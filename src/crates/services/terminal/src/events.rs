//! Events module - Terminal event definitions
//!
//! This module defines the events that can be emitted by the terminal
//! for frontend communication.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::shell::ShellType;

/// Terminal events for frontend communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum TerminalEvent {
    /// Terminal session created
    SessionCreated {
        session_id: String,
        pid: Option<u32>,
        cwd: String,
    },

    /// Terminal data output
    Data { session_id: String, data: String },

    /// Binary data output (base64 encoded for serialization)
    BinaryData {
        session_id: String,
        data: String, // base64 encoded
    },

    /// Terminal process ready
    Ready {
        session_id: String,
        pid: u32,
        cwd: String,
    },

    /// Terminal process exited
    Exit {
        session_id: String,
        exit_code: Option<i32>,
    },

    /// Terminal title changed
    TitleChanged { session_id: String, title: String },

    /// Working directory changed
    CwdChanged { session_id: String, cwd: String },

    /// Shell type detected/changed
    ShellTypeChanged {
        session_id: String,
        shell_type: ShellType,
    },

    /// Command started (shell integration)
    CommandStarted {
        session_id: String,
        command: String,
        command_id: String,
    },

    /// Command finished (shell integration)
    CommandFinished {
        session_id: String,
        command_id: String,
        exit_code: i32,
    },

    /// Error occurred
    Error {
        session_id: Option<String>,
        message: String,
        code: Option<String>,
    },

    /// Session restored from persistence
    SessionRestored {
        session_id: String,
        replay_data: Option<String>,
    },

    /// Terminal resized
    Resized {
        session_id: String,
        cols: u16,
        rows: u16,
    },

    /// Terminal session destroyed/closed
    SessionDestroyed { session_id: String },
}

impl TerminalEvent {
    /// Get the session ID associated with this event (if any)
    pub fn session_id(&self) -> Option<&str> {
        match self {
            TerminalEvent::SessionCreated { session_id, .. } => Some(session_id),
            TerminalEvent::Data { session_id, .. } => Some(session_id),
            TerminalEvent::BinaryData { session_id, .. } => Some(session_id),
            TerminalEvent::Ready { session_id, .. } => Some(session_id),
            TerminalEvent::Exit { session_id, .. } => Some(session_id),
            TerminalEvent::TitleChanged { session_id, .. } => Some(session_id),
            TerminalEvent::CwdChanged { session_id, .. } => Some(session_id),
            TerminalEvent::ShellTypeChanged { session_id, .. } => Some(session_id),
            TerminalEvent::CommandStarted { session_id, .. } => Some(session_id),
            TerminalEvent::CommandFinished { session_id, .. } => Some(session_id),
            TerminalEvent::Error { session_id, .. } => session_id.as_deref(),
            TerminalEvent::SessionRestored { session_id, .. } => Some(session_id),
            TerminalEvent::Resized { session_id, .. } => Some(session_id),
            TerminalEvent::SessionDestroyed { session_id, .. } => Some(session_id),
        }
    }

    /// Check if this is a data event
    pub fn is_data_event(&self) -> bool {
        matches!(
            self,
            TerminalEvent::Data { .. } | TerminalEvent::BinaryData { .. }
        )
    }
}

/// Event emitter for terminal events
pub struct TerminalEventEmitter {
    /// Channel sender for events
    tx: mpsc::Sender<TerminalEvent>,
    /// Channel receiver for events
    rx: Arc<tokio::sync::RwLock<mpsc::Receiver<TerminalEvent>>>,
}

impl TerminalEventEmitter {
    /// Create a new event emitter
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer_size);
        Self {
            tx,
            rx: Arc::new(tokio::sync::RwLock::new(rx)),
        }
    }

    /// Emit an event
    pub async fn emit(
        &self,
        event: TerminalEvent,
    ) -> Result<(), mpsc::error::SendError<TerminalEvent>> {
        self.tx.send(event).await
    }

    /// Try to emit an event without blocking
    pub fn try_emit(
        &self,
        event: TerminalEvent,
    ) -> Result<(), mpsc::error::TrySendError<TerminalEvent>> {
        self.tx.try_send(event)
    }

    /// Get a clone of the sender for use in other tasks
    pub fn sender(&self) -> mpsc::Sender<TerminalEvent> {
        self.tx.clone()
    }

    /// Receive the next event
    pub async fn recv(&self) -> Option<TerminalEvent> {
        let mut rx = self.rx.write().await;
        rx.recv().await
    }

    /// Try to receive an event without blocking
    pub async fn try_recv(&self) -> Option<TerminalEvent> {
        let mut rx = self.rx.write().await;
        rx.try_recv().ok()
    }
}

impl Default for TerminalEventEmitter {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Callback-based event handler
pub type EventCallback = Box<dyn Fn(TerminalEvent) + Send + Sync>;

/// Event dispatcher that can register multiple callbacks
pub struct EventDispatcher {
    callbacks: Vec<EventCallback>,
}

impl EventDispatcher {
    /// Create a new event dispatcher
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
        }
    }

    /// Register a callback
    pub fn on_event<F>(&mut self, callback: F)
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
    }

    /// Dispatch an event to all callbacks
    pub fn dispatch(&self, event: TerminalEvent) {
        for callback in &self.callbacks {
            callback(event.clone());
        }
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
