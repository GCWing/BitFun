//! Persistent subagent instance tracking.
//!
//! A SubagentInstance is an orchestrator-level concept that maps a resumable
//! instance_id to a child Session in the Runtime. This enables the main agent
//! to resume a previous subagent without creating a new session, preserving
//! context and reducing repeated code reads.

use std::time::{SystemTime, UNIX_EPOCH};

use log::{debug, info, warn};

/// Orchestrator-assigned instance ID prefix.
const SUBAGENT_INSTANCE_ID_PREFIX: &str = "subagent-instance";

/// Lifecycle state of a persistent subagent instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubagentInstanceStatus {
    /// A Task is currently executing on this instance.
    Running,
    /// Between Tasks. Session is persisted, context retained.
    Idle,
    /// Explicitly destroyed or parent session ended.
    Destroyed,
}

/// A resumable subagent instance.
///
/// Maps 1:1 to a child Session in the Runtime. The instance_id is the
/// orchestrator-level identifier used by the main agent; the child_session_id
/// is the Runtime-level session identifier.
#[derive(Debug, Clone)]
pub(crate) struct SubagentInstance {
    /// Orchestrator-assigned instance ID (not the session ID).
    pub instance_id: String,

    /// Parent (main agent) session ID.
    pub parent_session_id: String,

    /// Child session ID in the Runtime.
    pub child_session_id: String,

    /// Agent type used for this instance.
    pub agent_type: String,

    /// Current lifecycle state.
    pub status: SubagentInstanceStatus,

    /// When the instance was first created (unix epoch millis).
    pub created_at: u64,

    /// Last time a Task completed on this instance (unix epoch millis).
    pub last_active_at: u64,
}

impl SubagentInstance {
    /// Create a new instance with Running status.
    pub(crate) fn new(
        instance_id: String,
        parent_session_id: String,
        child_session_id: String,
        agent_type: String,
    ) -> Self {
        let now = now_millis();
        Self {
            instance_id,
            parent_session_id,
            child_session_id,
            agent_type,
            status: SubagentInstanceStatus::Running,
            created_at: now,
            last_active_at: now,
        }
    }

    /// Generate a new unique instance ID.
    pub(crate) fn generate_instance_id() -> String {
        format!("{}-{}", SUBAGENT_INSTANCE_ID_PREFIX, uuid::Uuid::new_v4())
    }

    /// Whether this instance can be resumed (must be Idle).
    pub(crate) fn can_resume(&self) -> bool {
        self.status == SubagentInstanceStatus::Idle
    }
}

/// Registry of active and idle subagent instances.
///
/// Thread-safe via DashMap. Scoped to a single Coordinator lifetime
/// (single main session). Not persisted across main session restarts.
pub(crate) struct SubagentInstanceRegistry {
    instances: dashmap::DashMap<String, SubagentInstance>,
}

impl SubagentInstanceRegistry {
    pub(crate) fn new() -> Self {
        Self {
            instances: dashmap::DashMap::new(),
        }
    }

    /// Register a new instance. Logs at debug level.
    pub(crate) fn register(&self, instance: SubagentInstance) {
        debug!(
            "Subagent instance registered: instance_id={}, parent_session_id={}, child_session_id={}, agent_type={}",
            instance.instance_id,
            instance.parent_session_id,
            instance.child_session_id,
            instance.agent_type
        );
        self.instances
            .insert(instance.instance_id.clone(), instance);
    }

    /// Get an instance by ID.
    pub(crate) fn get(&self, instance_id: &str) -> Option<SubagentInstance> {
        self.instances.get(instance_id).map(|entry| entry.clone())
    }

    /// Transition an instance to Running. Returns error if not Idle.
    pub(crate) fn set_running(&self, instance_id: &str) -> Result<(), String> {
        let mut entry = self
            .instances
            .get_mut(instance_id)
            .ok_or_else(|| format!("subagent instance not found: {}", instance_id))?;
        if entry.status != SubagentInstanceStatus::Idle {
            warn!(
                "Invalid state transition for subagent instance: instance_id={}, current={:?}, attempted={:?}",
                instance_id,
                entry.status,
                SubagentInstanceStatus::Running
            );
            return Err(format!(
                "subagent instance {} cannot transition to Running from {:?}",
                instance_id, entry.status
            ));
        }
        entry.status = SubagentInstanceStatus::Running;
        debug!(
            "Subagent instance transitioning to Running: instance_id={}",
            instance_id
        );
        Ok(())
    }

    /// Transition an instance to Idle. Returns error if not Running.
    pub(crate) fn set_idle(&self, instance_id: &str) -> Result<(), String> {
        let mut entry = self
            .instances
            .get_mut(instance_id)
            .ok_or_else(|| format!("subagent instance not found: {}", instance_id))?;
        if entry.status != SubagentInstanceStatus::Running {
            warn!(
                "Invalid state transition for subagent instance: instance_id={}, current={:?}, attempted={:?}",
                instance_id,
                entry.status,
                SubagentInstanceStatus::Idle
            );
            return Err(format!(
                "subagent instance {} cannot transition to Idle from {:?}",
                instance_id, entry.status
            ));
        }
        entry.status = SubagentInstanceStatus::Idle;
        entry.last_active_at = now_millis();
        debug!(
            "Subagent instance transitioning to Idle: instance_id={}, last_active_at={}",
            instance_id, entry.last_active_at
        );
        Ok(())
    }

    /// Destroy a single instance. Logs at info level.
    pub(crate) fn destroy(&self, instance_id: &str, reason: &str) {
        if self.instances.remove(instance_id).is_some() {
            info!(
                "Subagent instance destroyed: instance_id={}, reason={}",
                instance_id, reason
            );
        }
    }

    /// Destroy all instances for a given parent session. Logs at info level.
    /// Returns the count of destroyed instances.
    pub(crate) fn destroy_all_for_parent(&self, parent_session_id: &str) -> usize {
        let to_remove: Vec<String> = self
            .instances
            .iter()
            .filter(|entry| entry.value().parent_session_id == parent_session_id)
            .map(|entry| entry.key().clone())
            .collect();
        let count = to_remove.len();
        info!(
            "Destroying all subagent instances for parent session: parent_session_id={}, count={}",
            parent_session_id, count
        );
        for instance_id in to_remove {
            self.instances.remove(&instance_id);
        }
        count
    }

    /// List all instance IDs for a given parent session.
    pub(crate) fn list_for_parent(&self, parent_session_id: &str) -> Vec<String> {
        self.instances
            .iter()
            .filter(|entry| entry.value().parent_session_id == parent_session_id)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Number of active (non-destroyed) instances.
    pub(crate) fn active_count(&self) -> usize {
        self.instances.len()
    }
}

impl Default for SubagentInstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Current unix epoch time in milliseconds.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
