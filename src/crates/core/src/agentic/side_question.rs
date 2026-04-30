//! Shared `/btw` helpers and runtime-only request tracking.

use crate::agentic::core::PromptEnvelope;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct SideQuestionRuntime {
    tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    btw_turns: Arc<Mutex<HashMap<String, ActiveBtwTurn>>>,
}

#[derive(Debug, Clone)]
pub struct ActiveBtwTurn {
    pub session_id: String,
    pub turn_id: String,
}

impl Default for SideQuestionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SideQuestionRuntime {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(HashMap::new())),
            btw_turns: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, request_id: String) -> CancellationToken {
        let token = CancellationToken::new();

        let old = {
            let mut guard = self.tokens.lock().await;
            guard.insert(request_id, token.clone())
        };
        if let Some(old) = old {
            old.cancel();
        }

        token
    }

    pub async fn cancel(&self, request_id: &str) {
        let token = {
            let guard = self.tokens.lock().await;
            guard.get(request_id).cloned()
        };
        if let Some(token) = token {
            token.cancel();
        }
    }

    pub async fn remove(&self, request_id: &str) {
        {
            let mut guard = self.tokens.lock().await;
            guard.remove(request_id);
        }
        let mut btw_turns = self.btw_turns.lock().await;
        btw_turns.remove(request_id);
    }

    pub async fn register_btw_turn(&self, request_id: String, session_id: String, turn_id: String) {
        let mut guard = self.btw_turns.lock().await;
        guard.insert(
            request_id,
            ActiveBtwTurn {
                session_id,
                turn_id,
            },
        );
    }

    pub async fn get_btw_turn(&self, request_id: &str) -> Option<ActiveBtwTurn> {
        let guard = self.btw_turns.lock().await;
        guard.get(request_id).cloned()
    }
}

pub fn btw_system_reminder() -> &'static str {
    r#"This is a side question from the user. You must answer this question directly.

IMPORTANT CONTEXT:
- You are a separate, lightweight agent spawned to answer this question
- The main agent is NOT interrupted - it continues working independently in the background
- You share the conversation context but are a completely separate instance
- Do NOT reference being interrupted or what you were "previously doing" - that framing is incorrect

CRITICAL CONSTRAINTS:
- Use tools only when necessary to answer the question correctly
- You should answer the question directly, using what you already know from the conversation context as your starting point
- Do NOT say things like "Let me try...", "I'll now...", "Let me check...", or promise to take any action unless you actually take that action in this side thread
- If you don't know the answer, say so clearly - do not pretend you already checked something
- Reply concisely and match the user's language

Simply answer the question with the information you have, and use tools only when needed."#
}

pub fn build_btw_user_input(question: &str) -> String {
    let mut envelope = PromptEnvelope::new();
    envelope.push_system_reminder(btw_system_reminder());
    envelope.push_user_query(question.trim());
    envelope.render()
}
