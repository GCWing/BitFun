use std::collections::{HashMap, HashSet};

use serde::Serialize;
use uuid::Uuid;

use crate::server::response::WebDriverErrorResponse;

#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_field_names)]
pub struct Timeouts {
    pub implicit: u64,
    #[serde(rename = "pageLoad")]
    pub page_load: u64,
    pub script: u64,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            implicit: 0,
            page_load: 300_000,
            script: 30_000,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FrameId {
    Index(u32),
    Element(String),
}

#[derive(Debug, Clone, Default)]
pub struct ActionState {
    pub pressed_keys: HashSet<String>,
    pub pressed_buttons: HashMap<String, HashSet<u32>>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub current_window: String,
    pub timeouts: Timeouts,
    pub frame_context: Vec<FrameId>,
    pub action_state: ActionState,
}

impl Session {
    pub fn new(initial_window: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            current_window: initial_window,
            timeouts: Timeouts::default(),
            frame_context: Vec::new(),
            action_state: ActionState::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn create(&mut self, initial_window: String) -> Session {
        let session = Session::new(initial_window);
        self.sessions.insert(session.id.clone(), session.clone());
        session
    }

    pub fn get(&self, id: &str) -> Result<&Session, WebDriverErrorResponse> {
        self.sessions
            .get(id)
            .ok_or_else(|| WebDriverErrorResponse::invalid_session_id(id))
    }

    pub fn get_cloned(&self, id: &str) -> Result<Session, WebDriverErrorResponse> {
        self.get(id).cloned()
    }

    pub fn get_mut(&mut self, id: &str) -> Result<&mut Session, WebDriverErrorResponse> {
        self.sessions
            .get_mut(id)
            .ok_or_else(|| WebDriverErrorResponse::invalid_session_id(id))
    }

    pub fn delete(&mut self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }
}
