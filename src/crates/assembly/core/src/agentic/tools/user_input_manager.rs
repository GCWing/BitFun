//! Global user input manager
//!
//! Used to manage tool waiting channels
//! supporting tools to asynchronously wait for user submissions during execution

use dashmap::DashMap;
use log::{debug, info, warn};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tokio::sync::oneshot;

/// User input response
#[derive(Debug, Clone)]
pub struct UserInputResponse {
    pub answers: Value,
}

/// Global user input manager
pub struct UserInputManager {
    /// Store waiting channels: tool_id -> oneshot::Sender
    channels: Arc<DashMap<String, oneshot::Sender<UserInputResponse>>>,
}

impl Default for UserInputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UserInputManager {
    /// Create a new manager instance
    pub fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
        }
    }

    /// Register a waiting channel
    pub fn register_channel(&self, tool_id: String, sender: oneshot::Sender<UserInputResponse>) {
        debug!("Registered waiting channel: tool_id={}", tool_id);
        self.channels.insert(tool_id, sender);
    }

    /// Send user answer
    pub fn send_answer(&self, tool_id: &str, answers: Value) -> Result<(), String> {
        info!("Sending user answer: tool_id={}", tool_id);

        if let Some((_, sender)) = self.channels.remove(tool_id) {
            let response = UserInputResponse { answers };
            sender
                .send(response)
                .map_err(|_| format!("Channel closed, cannot send answer: {}", tool_id))?;
            debug!("Answer sent: tool_id={}", tool_id);
            Ok(())
        } else {
            let error_msg = format!("Waiting channel not found: {}", tool_id);
            warn!("{}", error_msg);
            Err(error_msg)
        }
    }

    /// Cancel waiting (clean up channels)
    pub fn cancel(&self, tool_id: &str) -> bool {
        if self.channels.remove(tool_id).is_some() {
            debug!("Cancelled waiting: tool_id={}", tool_id);
            true
        } else {
            false
        }
    }

    /// Check if there are any waiting channels
    pub fn has_pending(&self, tool_id: &str) -> bool {
        self.channels.contains_key(tool_id)
    }

    /// Get all waiting tool_id lists
    pub fn pending_tool_ids(&self) -> Vec<String> {
        self.channels
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }
}

/// Global singleton instance
pub static USER_INPUT_MANAGER: LazyLock<UserInputManager> = LazyLock::new(|| {
    debug!("Initializing global user input manager");
    UserInputManager::new()
});

/// Get global user input manager
pub fn get_user_input_manager() -> &'static UserInputManager {
    &USER_INPUT_MANAGER
}
