//! Shared Peer Host service handles and bounded Peer-owned turn state.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use bitfun_agent_runtime::sdk::AgentRuntime;
use bitfun_agent_runtime::sdk::PermissionRequest;
use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use bitfun_core::service::filesystem::FileSystemService;
use bitfun_core::service::workspace::WorkspaceService;
use bitfun_runtime_ports::{AgentSubmissionSource, AgentTurnCancellationRequest};

use crate::runtime::events::CliAgentEventSource;

const MAX_TRACKED_PEER_TURNS: usize = 256;
const MAX_BACKGROUND_PEER_AUTHORIZATIONS: usize = 256;
const MAX_PENDING_PEER_TASK_CANCELLATIONS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PeerTurnKey {
    pub(crate) session_id: String,
    pub(crate) turn_id: String,
}

impl PeerTurnKey {
    pub(crate) fn new(session_id: impl Into<String>, turn_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            turn_id: turn_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PeerBackgroundSubagentLink {
    pub(crate) parent_session_id: String,
    pub(crate) subagent_session_id: String,
}

#[derive(Default)]
pub(crate) struct PeerTurnDrain {
    pub(crate) turns: Vec<PeerTurnKey>,
    pub(crate) background_subagents: Vec<PeerBackgroundSubagentLink>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerEventStreamState {
    Recovering,
    Ready,
    Closed,
}

struct PeerTurnTrackerInner {
    stream: PeerEventStreamState,
    stream_generation: u64,
    parents: HashMap<PeerTurnKey, Option<PeerTurnKey>>,
    active: HashSet<PeerTurnKey>,
    started: HashSet<PeerTurnKey>,
    terminal_deliveries: HashSet<PeerTurnKey>,
    interrupted_turns: HashSet<PeerTurnKey>,
    background_task_calls: HashMap<(PeerTurnKey, String), PeerBackgroundTaskCall>,
    background_task_cancellations: HashMap<(PeerTurnKey, String), String>,
    background_source_children: HashSet<PeerTurnKey>,
    background_source_tasks: HashMap<String, (PeerTurnKey, PeerTurnKey)>,
    background_follow_ups: HashSet<PeerTurnKey>,
    early_background_follow_ups: HashSet<PeerTurnKey>,
    completed_background_sources: HashMap<PeerTurnKey, PeerTurnKey>,
}

#[derive(Default)]
struct PeerBackgroundTaskCall {
    background_task_id: Option<String>,
    source_child: Option<PeerTurnKey>,
}

#[derive(Clone)]
pub(crate) struct PeerTurnTracker {
    inner: Arc<Mutex<PeerTurnTrackerInner>>,
}

impl PeerTurnTracker {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PeerTurnTrackerInner {
                stream: PeerEventStreamState::Recovering,
                stream_generation: 0,
                parents: HashMap::new(),
                active: HashSet::new(),
                started: HashSet::new(),
                terminal_deliveries: HashSet::new(),
                interrupted_turns: HashSet::new(),
                background_task_calls: HashMap::new(),
                background_task_cancellations: HashMap::new(),
                background_source_children: HashSet::new(),
                background_source_tasks: HashMap::new(),
                background_follow_ups: HashSet::new(),
                early_background_follow_ups: HashSet::new(),
                completed_background_sources: HashMap::new(),
            })),
        }
    }

    pub(crate) fn mark_event_stream_ready(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.stream != PeerEventStreamState::Closed {
                inner.stream = PeerEventStreamState::Ready;
            }
        }
    }

    pub(crate) fn register_root(&self, key: PeerTurnKey) -> Result<u64, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        match inner.stream {
            PeerEventStreamState::Ready => {}
            PeerEventStreamState::Recovering => {
                return Err("Peer event stream is recovering; retry the request".to_string())
            }
            PeerEventStreamState::Closed => return Err("Peer event stream is closed".to_string()),
        }
        if inner.parents.contains_key(&key) {
            return Err("Peer turn is already tracked".to_string());
        }
        reject_interrupted_turn_id(&inner, &key)?;
        if inner.active.len() >= MAX_TRACKED_PEER_TURNS
            || inner.parents.len() >= MAX_TRACKED_PEER_TURNS
        {
            return Err("Peer turn tracking capacity is exhausted".to_string());
        }
        inner.parents.insert(key.clone(), None);
        inner.active.insert(key);
        Ok(inner.stream_generation)
    }

    pub(crate) fn claim_terminal_delivery(&self, key: &PeerTurnKey) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        if !inner.active.contains(key) {
            return Ok(false);
        }
        Ok(inner.terminal_deliveries.insert(key.clone()))
    }

    pub(crate) fn is_interrupted_terminal(&self, key: &PeerTurnKey) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.interrupted_turns.contains(key))
            .unwrap_or(false)
    }

    pub(crate) fn complete_terminal_delivery(&self, generation: u64, key: &PeerTurnKey) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.stream == PeerEventStreamState::Ready && inner.stream_generation == generation
            {
                finish_turn_locked(&mut inner, key);
            }
        }
    }

    pub(crate) fn is_event_stream_generation_current(&self, generation: u64) -> bool {
        self.inner
            .lock()
            .map(|inner| {
                inner.stream == PeerEventStreamState::Ready && inner.stream_generation == generation
            })
            .unwrap_or(false)
    }

    pub(crate) fn current_event_stream_generation(&self) -> Result<u64, String> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        if inner.stream != PeerEventStreamState::Ready {
            return Err("Peer event stream is not ready".to_string());
        }
        Ok(inner.stream_generation)
    }

    #[cfg(test)]
    pub(crate) fn register_child(
        &self,
        parent: &PeerTurnKey,
        child: PeerTurnKey,
    ) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        register_child_locked(&mut inner, parent, child, false)
    }

    pub(crate) fn record_background_task_call(
        &self,
        parent: &PeerTurnKey,
        tool_call_id: String,
    ) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        if !inner.active.contains(parent) {
            return Ok(false);
        }
        let call_key = (parent.clone(), tool_call_id);
        if inner.background_task_calls.contains_key(&call_key) {
            return Ok(true);
        }
        if background_authorization_len(&inner) >= MAX_BACKGROUND_PEER_AUTHORIZATIONS {
            return Err("Peer background authorization capacity is exhausted".to_string());
        }
        inner
            .background_task_calls
            .insert(call_key, PeerBackgroundTaskCall::default());
        Ok(true)
    }

    pub(crate) fn record_background_task_cancellation(
        &self,
        parent: &PeerTurnKey,
        tool_call_id: String,
        target_session_id: String,
    ) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        if !inner.active.contains(parent) {
            return Ok(false);
        }
        let cancellation_key = (parent.clone(), tool_call_id);
        if let Some(existing_target) = inner.background_task_cancellations.get(&cancellation_key) {
            if existing_target == &target_session_id {
                return Ok(true);
            }
            return Err("Peer Task cancellation is already bound to another session".to_string());
        }
        if inner.background_task_cancellations.len() >= MAX_PENDING_PEER_TASK_CANCELLATIONS {
            return Err("Peer Task cancellation tracking capacity is exhausted".to_string());
        }
        inner
            .background_task_cancellations
            .insert(cancellation_key, target_session_id);
        Ok(true)
    }

    pub(crate) fn finish_task_call(
        &self,
        parent: &PeerTurnKey,
        tool_call_id: &str,
        background_task_id: Option<&str>,
        cancelled_background_tasks: Option<u64>,
    ) {
        if let Ok(mut inner) = self.inner.lock() {
            let call_key = (parent.clone(), tool_call_id.to_string());
            if let Some(background_task_id) = background_task_id {
                if let Some(call) = inner.background_task_calls.get_mut(&call_key) {
                    call.background_task_id = Some(background_task_id.to_string());
                }
                bind_background_source_task(&mut inner, &call_key);
            } else {
                inner.background_task_calls.remove(&call_key);
            }
            let cancellation = inner
                .background_task_cancellations
                .remove(&(parent.clone(), tool_call_id.to_string()));
            if cancelled_background_tasks.is_some_and(|count| count > 0) {
                if let Some(target_session_id) = cancellation {
                    release_background_sources_for_subagent(
                        &mut inner,
                        &parent.session_id,
                        &target_session_id,
                    );
                }
            }
        }
    }

    pub(crate) fn register_linked_child(
        &self,
        parent: &PeerTurnKey,
        child: PeerTurnKey,
        parent_tool_call_id: &str,
    ) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        let background_call_key = (parent.clone(), parent_tool_call_id.to_string());
        let is_background_source = inner
            .background_task_calls
            .contains_key(&background_call_key);
        let registered = register_child_locked(&mut inner, parent, child.clone(), false)?;
        if registered && is_background_source {
            if let Some(call) = inner.background_task_calls.get_mut(&background_call_key) {
                call.source_child = Some(child.clone());
            }
            inner.background_source_children.insert(child);
            bind_background_source_task(&mut inner, &background_call_key);
        }
        Ok(registered)
    }

    pub(crate) fn finish_background_injection(
        &self,
        parent: &PeerTurnKey,
        background_task_id: &str,
    ) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        let Some((mapped_parent, source)) = inner
            .background_source_tasks
            .get(background_task_id)
            .cloned()
        else {
            return false;
        };
        if &mapped_parent != parent {
            return false;
        }
        inner.background_source_tasks.remove(background_task_id);
        inner.background_source_children.remove(&source);
        inner.early_background_follow_ups.remove(&source);
        take_completed_background_source(&mut inner, &source);
        prune_completed_branch(&mut inner, &source);
        true
    }

    pub(crate) fn register_background_follow_up(
        &self,
        parent: &PeerTurnKey,
        source_child: &PeerTurnKey,
        follow_up: PeerTurnKey,
    ) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Peer turn tracker is unavailable".to_string())?;
        reject_interrupted_turn_id(&inner, &follow_up)?;
        if inner.active.contains(&follow_up) {
            return Ok(true);
        }
        let parent_is_tracked = inner.parents.contains_key(parent);
        let completed_source_matches =
            inner.completed_background_sources.get(source_child) == Some(parent);
        if !parent_is_tracked && !completed_source_matches {
            return Ok(false);
        }
        let required_parent_slots = usize::from(!parent_is_tracked) + 1;
        if inner.active.len().saturating_add(1) > MAX_TRACKED_PEER_TURNS
            || inner.parents.len().saturating_add(required_parent_slots) > MAX_TRACKED_PEER_TURNS
        {
            return Err("Peer turn tracking capacity is exhausted".to_string());
        }
        let tracked_source_matches = inner.parents.get(source_child).and_then(Option::as_ref)
            == Some(parent)
            && inner.background_source_children.contains(source_child);
        if (!tracked_source_matches && !completed_source_matches)
            || inner.background_follow_ups.contains(source_child)
        {
            return Err(
                "Peer background follow-up source child is not owned by its parent".to_string(),
            );
        }
        if completed_source_matches {
            take_completed_background_source(&mut inner, source_child);
        }
        if !parent_is_tracked {
            inner.parents.insert(parent.clone(), None);
        }

        if !completed_source_matches {
            if !inner.active.contains(source_child) {
                return Err(
                    "Peer background follow-up source child is not actively owned by its parent"
                        .to_string(),
                );
            }
            inner
                .early_background_follow_ups
                .insert(source_child.clone());
            inner.background_source_children.remove(source_child);
        }
        inner
            .parents
            .insert(follow_up.clone(), Some(parent.clone()));
        inner.active.insert(follow_up.clone());
        inner.background_follow_ups.insert(follow_up);
        Ok(true)
    }

    pub(crate) fn owns(&self, session_id: &str, turn_id: Option<&str>) -> bool {
        self.inner
            .lock()
            .map(|inner| match turn_id {
                Some(turn_id) => inner
                    .active
                    .contains(&PeerTurnKey::new(session_id, turn_id)),
                None => inner.started.iter().any(|key| key.session_id == session_id),
            })
            .unwrap_or(false)
    }

    pub(crate) fn owns_permission_request(&self, request: &PermissionRequest) -> bool {
        self.owns(&request.session_id, None)
            || request
                .delegation
                .as_ref()
                .is_some_and(|delegation| self.owns(&delegation.parent_session_id, None))
    }

    pub(crate) fn mark_started(&self, key: &PeerTurnKey) -> bool {
        self.inner
            .lock()
            .map(|mut inner| {
                if !inner.active.contains(key) {
                    return false;
                }
                inner.started.insert(key.clone());
                true
            })
            .unwrap_or(false)
    }

    pub(crate) fn finish_turn(&self, key: &PeerTurnKey) {
        if let Ok(mut inner) = self.inner.lock() {
            finish_turn_locked(&mut inner, key);
        }
    }

    pub(crate) fn drain_session_turns(&self, session_id: &str) -> PeerTurnDrain {
        self.inner
            .lock()
            .map(|mut inner| {
                let removed = session_tree_keys(&inner, session_id);
                if !try_quarantine_active_turns(&mut inner, &removed) {
                    inner.interrupted_turns.clear();
                    inner.stream = PeerEventStreamState::Closed;
                }
                let mut drain = peer_turn_drain_for_keys(&inner, &removed);
                merge_completed_background_subagents(&inner, &mut drain, Some(session_id));
                remove_completed_background_sources_for_session(&mut inner, session_id);
                remove_tracked_turns(&mut inner, &removed);
                drain
            })
            .unwrap_or_default()
    }

    pub(crate) fn session_turns_for_cancellation(&self, session_id: &str) -> PeerTurnDrain {
        self.inner
            .lock()
            .map(|inner| {
                let keys = session_tree_keys(&inner, session_id);
                let mut drain = peer_turn_drain_for_keys(&inner, &keys);
                merge_completed_background_subagents(&inner, &mut drain, Some(session_id));
                drain
            })
            .unwrap_or_default()
    }

    pub(crate) fn interrupt_event_stream(&self, closed: bool) -> PeerTurnDrain {
        let Ok(mut inner) = self.inner.lock() else {
            return PeerTurnDrain::default();
        };
        let interrupted = inner.active.clone();
        let quarantine_fits = try_quarantine_active_turns(&mut inner, &interrupted);
        let stream_closed = closed || !quarantine_fits;
        if stream_closed {
            inner.interrupted_turns.clear();
        }
        let turns = drain_peer_turns(&mut inner);
        inner.stream_generation = inner.stream_generation.wrapping_add(1);
        inner.stream = if stream_closed {
            PeerEventStreamState::Closed
        } else {
            PeerEventStreamState::Recovering
        };
        turns
    }

    pub(crate) fn drain_peer_turns(&self) -> PeerTurnDrain {
        self.inner
            .lock()
            .map(|mut inner| {
                let active = inner.active.clone();
                if !try_quarantine_active_turns(&mut inner, &active) {
                    inner.interrupted_turns.clear();
                    inner.stream = PeerEventStreamState::Closed;
                }
                drain_peer_turns(&mut inner)
            })
            .unwrap_or_default()
    }

    pub(crate) fn peer_turns_for_cancellation(&self) -> PeerTurnDrain {
        self.inner
            .lock()
            .map(|inner| {
                let keys = inner.parents.keys().cloned().collect::<HashSet<_>>();
                let mut drain = peer_turn_drain_for_keys(&inner, &keys);
                merge_completed_background_subagents(&inner, &mut drain, None);
                drain
            })
            .unwrap_or_default()
    }
}

fn register_child_locked(
    inner: &mut PeerTurnTrackerInner,
    parent: &PeerTurnKey,
    child: PeerTurnKey,
    is_background_source: bool,
) -> Result<bool, String> {
    reject_interrupted_turn_id(inner, &child)?;
    if !inner.parents.contains_key(parent) {
        return Ok(false);
    }
    if let Some(existing_parent) = inner.parents.get(&child) {
        if existing_parent.as_ref() == Some(parent) {
            return Ok(true);
        }
        return Err("Peer child turn is already owned by another parent".to_string());
    }
    if inner.active.len() >= MAX_TRACKED_PEER_TURNS || inner.parents.len() >= MAX_TRACKED_PEER_TURNS
    {
        return Err("Peer turn tracking capacity is exhausted".to_string());
    }
    inner.parents.insert(child.clone(), Some(parent.clone()));
    inner.active.insert(child.clone());
    if is_background_source {
        inner.background_source_children.insert(child);
    }
    Ok(true)
}

fn reject_interrupted_turn_id(
    inner: &PeerTurnTrackerInner,
    key: &PeerTurnKey,
) -> Result<(), String> {
    if inner.interrupted_turns.contains(key) {
        return Err(
            "Peer turn ID was interrupted and cannot be reused; retry with a new turn ID"
                .to_string(),
        );
    }
    Ok(())
}

fn drain_peer_turns(inner: &mut PeerTurnTrackerInner) -> PeerTurnDrain {
    let all_keys = inner.parents.keys().cloned().collect::<HashSet<_>>();
    let mut drain = peer_turn_drain_for_keys(inner, &all_keys);
    merge_completed_background_subagents(inner, &mut drain, None);
    inner.parents.clear();
    inner.active.clear();
    inner.started.clear();
    inner.terminal_deliveries.clear();
    inner.background_task_calls.clear();
    inner.background_task_cancellations.clear();
    inner.background_source_children.clear();
    inner.background_source_tasks.clear();
    inner.background_follow_ups.clear();
    inner.early_background_follow_ups.clear();
    inner.completed_background_sources.clear();
    drain
}

fn try_quarantine_active_turns(
    inner: &mut PeerTurnTrackerInner,
    keys: &HashSet<PeerTurnKey>,
) -> bool {
    let new_keys = keys
        .iter()
        .filter(|key| inner.active.contains(*key) && !inner.interrupted_turns.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    if inner.interrupted_turns.len().saturating_add(new_keys.len()) > MAX_TRACKED_PEER_TURNS {
        return false;
    }
    inner.interrupted_turns.extend(new_keys);
    true
}

fn finish_turn_locked(inner: &mut PeerTurnTrackerInner, key: &PeerTurnKey) {
    if !inner.active.remove(key) {
        return;
    }
    inner.started.remove(key);
    inner.terminal_deliveries.remove(key);
    inner
        .background_task_calls
        .retain(|(parent, _), _| parent != key);
    inner
        .background_task_cancellations
        .retain(|(parent, _), _| parent != key);
    let parent = inner.parents.get(key).cloned().flatten();
    let is_background_follow_up = inner.background_follow_ups.remove(key);
    let is_background_source = inner.background_source_children.remove(key);
    let follow_up_already_registered = inner.early_background_follow_ups.remove(key);

    if !is_background_follow_up && is_background_source && !follow_up_already_registered {
        if let Some(parent) = parent.as_ref() {
            remember_completed_background_source(inner, key, parent);
        }
    }

    prune_completed_branch(inner, key);
    if let Some(root) = root_for(inner, key).or_else(|| parent.clone()) {
        prune_idle_tree(inner, &root);
    }
}

fn bind_background_source_task(inner: &mut PeerTurnTrackerInner, call_key: &(PeerTurnKey, String)) {
    let Some(call) = inner.background_task_calls.get(call_key) else {
        return;
    };
    let (Some(background_task_id), Some(source_child)) =
        (call.background_task_id.as_ref(), call.source_child.as_ref())
    else {
        return;
    };
    inner.background_source_tasks.insert(
        background_task_id.clone(),
        (call_key.0.clone(), source_child.clone()),
    );
    inner.background_task_calls.remove(call_key);
}

fn merge_completed_background_subagents(
    inner: &PeerTurnTrackerInner,
    drain: &mut PeerTurnDrain,
    session_id: Option<&str>,
) {
    let mut links = drain.background_subagents.drain(..).collect::<HashSet<_>>();
    links.extend(
        inner
            .completed_background_sources
            .iter()
            .filter(|(source, parent)| {
                source.session_id != parent.session_id
                    && session_id.is_none_or(|session_id| {
                        source.session_id == session_id || parent.session_id == session_id
                    })
            })
            .map(|(source, parent)| PeerBackgroundSubagentLink {
                parent_session_id: parent.session_id.clone(),
                subagent_session_id: source.session_id.clone(),
            }),
    );
    drain.background_subagents = links.into_iter().collect();
}

fn peer_turn_drain_for_keys(
    inner: &PeerTurnTrackerInner,
    keys: &HashSet<PeerTurnKey>,
) -> PeerTurnDrain {
    let turns = keys
        .iter()
        .filter(|key| inner.active.contains(*key))
        .cloned()
        .collect();
    let background_subagents = keys
        .iter()
        .filter_map(|key| {
            let parent = inner.parents.get(key)?.as_ref()?;
            (parent.session_id != key.session_id).then(|| PeerBackgroundSubagentLink {
                parent_session_id: parent.session_id.clone(),
                subagent_session_id: key.session_id.clone(),
            })
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    PeerTurnDrain {
        turns,
        background_subagents,
    }
}

fn session_tree_keys(inner: &PeerTurnTrackerInner, session_id: &str) -> HashSet<PeerTurnKey> {
    let mut keys = inner
        .parents
        .iter()
        .filter(|(key, parent)| {
            key.session_id == session_id
                || parent
                    .as_ref()
                    .is_some_and(|parent| parent.session_id == session_id)
        })
        .map(|(key, _)| key.clone())
        .collect::<HashSet<_>>();
    loop {
        let descendants = inner
            .parents
            .iter()
            .filter_map(|(key, parent)| {
                parent
                    .as_ref()
                    .filter(|parent| keys.contains(*parent))
                    .map(|_| key.clone())
            })
            .filter(|key| !keys.contains(key))
            .collect::<Vec<_>>();
        if descendants.is_empty() {
            break;
        }
        keys.extend(descendants);
    }
    keys
}

fn root_for(inner: &PeerTurnTrackerInner, key: &PeerTurnKey) -> Option<PeerTurnKey> {
    let mut current = key.clone();
    let mut remaining = inner.parents.len().saturating_add(1);
    while remaining > 0 {
        remaining -= 1;
        match inner.parents.get(&current) {
            Some(Some(parent)) => current = parent.clone(),
            Some(None) => return Some(current),
            None => return None,
        }
    }
    None
}

fn tree_keys(inner: &PeerTurnTrackerInner, root: &PeerTurnKey) -> HashSet<PeerTurnKey> {
    inner
        .parents
        .keys()
        .filter(|key| root_for(inner, key).as_ref() == Some(root))
        .cloned()
        .collect()
}

fn remember_completed_background_source(
    inner: &mut PeerTurnTrackerInner,
    source: &PeerTurnKey,
    parent: &PeerTurnKey,
) {
    if inner.completed_background_sources.contains_key(source) {
        return;
    }
    inner
        .completed_background_sources
        .insert(source.clone(), parent.clone());
}

fn take_completed_background_source(inner: &mut PeerTurnTrackerInner, source: &PeerTurnKey) {
    inner.completed_background_sources.remove(source);
    inner
        .background_source_tasks
        .retain(|_, (_, mapped_source)| mapped_source != source);
}

fn remove_completed_background_sources_for_session(
    inner: &mut PeerTurnTrackerInner,
    session_id: &str,
) {
    inner.completed_background_sources.retain(|source, parent| {
        source.session_id != session_id && parent.session_id != session_id
    });
    inner.background_source_tasks.retain(|_, (parent, source)| {
        source.session_id != session_id && parent.session_id != session_id
    });
}

fn release_background_sources_for_subagent(
    inner: &mut PeerTurnTrackerInner,
    parent_session_id: &str,
    subagent_session_id: &str,
) {
    let tracked_sources = inner
        .parents
        .iter()
        .filter_map(|(source, parent)| {
            parent.as_ref().filter(|parent| {
                parent.session_id == parent_session_id && source.session_id == subagent_session_id
            })?;
            (inner.background_source_children.contains(source)
                || inner.early_background_follow_ups.contains(source))
            .then(|| source.clone())
        })
        .collect::<HashSet<_>>();
    let completed_sources = inner
        .completed_background_sources
        .iter()
        .filter(|(source, parent)| {
            parent.session_id == parent_session_id && source.session_id == subagent_session_id
        })
        .map(|(source, _)| source.clone())
        .collect::<HashSet<_>>();

    for source in &tracked_sources {
        inner.background_source_children.remove(source);
        inner.early_background_follow_ups.remove(source);
    }
    for source in &completed_sources {
        take_completed_background_source(inner, source);
    }
    for source in tracked_sources {
        prune_completed_branch(inner, &source);
    }
    inner.background_source_tasks.retain(|_, (parent, source)| {
        parent.session_id != parent_session_id || source.session_id != subagent_session_id
    });
}

fn background_authorization_len(inner: &PeerTurnTrackerInner) -> usize {
    inner
        .background_task_calls
        .values()
        .filter(|call| call.source_child.is_none())
        .count()
        .saturating_add(inner.background_source_children.len())
        .saturating_add(inner.completed_background_sources.len())
}

fn remove_tracked_turns(inner: &mut PeerTurnTrackerInner, removed: &HashSet<PeerTurnKey>) {
    inner.parents.retain(|key, _| !removed.contains(key));
    inner.active.retain(|key| !removed.contains(key));
    inner.started.retain(|key| !removed.contains(key));
    inner
        .terminal_deliveries
        .retain(|key| !removed.contains(key));
    inner.background_task_calls.retain(|(parent, _), call| {
        !removed.contains(parent)
            && call
                .source_child
                .as_ref()
                .is_none_or(|source| !removed.contains(source))
    });
    inner
        .background_task_cancellations
        .retain(|(parent, _), _| !removed.contains(parent));
    inner
        .background_source_children
        .retain(|key| !removed.contains(key));
    let completed_background_sources = &inner.completed_background_sources;
    inner.background_source_tasks.retain(|_, (parent, source)| {
        completed_background_sources.get(source) == Some(parent)
            || (!removed.contains(parent) && !removed.contains(source))
    });
    inner
        .background_follow_ups
        .retain(|key| !removed.contains(key));
    inner
        .early_background_follow_ups
        .retain(|key| !removed.contains(key));
}

fn prune_completed_branch(inner: &mut PeerTurnTrackerInner, key: &PeerTurnKey) {
    let branch = inner
        .parents
        .keys()
        .filter(|candidate| {
            let mut current = (*candidate).clone();
            let mut remaining = inner.parents.len().saturating_add(1);
            while remaining > 0 {
                remaining -= 1;
                if &current == key {
                    return true;
                }
                match inner.parents.get(&current).and_then(Clone::clone) {
                    Some(parent) => current = parent,
                    None => return false,
                }
            }
            false
        })
        .cloned()
        .collect::<HashSet<_>>();
    if branch.iter().any(|key| inner.active.contains(key))
        || branch
            .iter()
            .any(|key| inner.early_background_follow_ups.contains(key))
    {
        return;
    }
    remove_tracked_turns(inner, &branch);
}

fn prune_idle_tree(inner: &mut PeerTurnTrackerInner, key: &PeerTurnKey) {
    let Some(root) = root_for(inner, key) else {
        return;
    };
    let tree = tree_keys(inner, &root);
    if tree.iter().any(|key| inner.active.contains(key))
        || tree
            .iter()
            .any(|key| inner.early_background_follow_ups.contains(key))
    {
        return;
    }
    remove_tracked_turns(inner, &tree);
}

#[derive(Clone)]
pub(crate) struct PeerHostState {
    pub(crate) agent_runtime: AgentRuntime,
    pub(crate) local_workspace_snapshot: Arc<dyn bitfun_runtime_ports::LocalWorkspaceSnapshotPort>,
    pub(crate) compatibility: CoreAgentRuntimeCompatibility,
    pub(crate) agent_events: CliAgentEventSource,
    pub(crate) turns: PeerTurnTracker,
    pub(crate) workspace_service: Arc<WorkspaceService>,
    pub(crate) filesystem_service: Arc<FileSystemService>,
}

impl PeerHostState {
    pub(crate) async fn cancel_and_drain_peer_turns(
        &self,
        reason: &'static str,
    ) -> Result<(), String> {
        let initial = self.turns.peer_turns_for_cancellation();
        let initial_result = self.cancel_peer_turns(initial, reason).await;
        tokio::task::yield_now().await;
        let raced = self.turns.drain_peer_turns();
        let raced_result = self.cancel_peer_turns(raced, reason).await;
        aggregate_cancellation_results(initial_result, raced_result)
    }

    pub(crate) async fn cancel_peer_turns(
        &self,
        drain: PeerTurnDrain,
        reason: &'static str,
    ) -> Result<(), String> {
        const MAX_CONCURRENT_CANCELLATIONS: usize = 32;

        let mut failure_count = 0usize;
        let mut pending_background = drain.background_subagents.into_iter();
        let mut background_tasks = tokio::task::JoinSet::new();
        for _ in 0..MAX_CONCURRENT_CANCELLATIONS {
            let Some(link) = pending_background.next() else {
                break;
            };
            spawn_background_subagent_cancellation(
                &mut background_tasks,
                self.compatibility.clone(),
                link,
            );
        }
        while let Some(joined) = background_tasks.join_next().await {
            match joined {
                Ok((link, Err(error))) => {
                    failure_count += 1;
                    tracing::warn!(
                        "Failed to cancel Peer-owned background subagent: parent_session_id={}, subagent_session_id={}, reason={}, error={}",
                        link.parent_session_id,
                        link.subagent_session_id,
                        reason,
                        error
                    );
                }
                Err(error) => {
                    failure_count += 1;
                    tracing::warn!(
                        "Peer-owned background subagent cancellation task failed: reason={}, error={}",
                        reason,
                        error
                    );
                }
                Ok((_, Ok(_))) => {}
            }
            if let Some(link) = pending_background.next() {
                spawn_background_subagent_cancellation(
                    &mut background_tasks,
                    self.compatibility.clone(),
                    link,
                );
            }
        }

        let mut pending = drain.turns.into_iter();
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..MAX_CONCURRENT_CANCELLATIONS {
            let Some(turn) = pending.next() else {
                break;
            };
            spawn_turn_cancellation(&mut tasks, self.agent_runtime.clone(), turn, reason);
        }

        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok((turn, Err(error))) => {
                    failure_count += 1;
                    tracing::warn!(
                        "Failed to cancel Peer-owned turn: session_id={}, turn_id={}, reason={}, error={}",
                        turn.session_id,
                        turn.turn_id,
                        reason,
                        error
                    );
                }
                Err(error) => {
                    failure_count += 1;
                    tracing::warn!(
                        "Peer-owned turn cancellation task failed: reason={}, error={}",
                        reason,
                        error
                    );
                }
                Ok((_, Ok(_))) => {}
            }
            if let Some(turn) = pending.next() {
                spawn_turn_cancellation(&mut tasks, self.agent_runtime.clone(), turn, reason);
            }
        }
        if failure_count == 0 {
            Ok(())
        } else {
            Err(format!(
                "{failure_count} Peer-owned cancellation operation(s) failed"
            ))
        }
    }
}

fn aggregate_cancellation_results(
    initial: Result<(), String>,
    raced: Result<(), String>,
) -> Result<(), String> {
    let failures = [initial, raced]
        .into_iter()
        .filter_map(Result::err)
        .collect::<Vec<_>>();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn spawn_background_subagent_cancellation(
    tasks: &mut tokio::task::JoinSet<(PeerBackgroundSubagentLink, Result<(), String>)>,
    compatibility: CoreAgentRuntimeCompatibility,
    link: PeerBackgroundSubagentLink,
) {
    tasks.spawn(async move {
        let result = compatibility
            .cancel_background_subagents_for_parent(
                &link.parent_session_id,
                &link.subagent_session_id,
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        (link, result)
    });
}

fn spawn_turn_cancellation(
    tasks: &mut tokio::task::JoinSet<(PeerTurnKey, Result<(), String>)>,
    runtime: AgentRuntime,
    turn: PeerTurnKey,
    reason: &'static str,
) {
    tasks.spawn(async move {
        let result = runtime
            .cancel_turn(AgentTurnCancellationRequest {
                session_id: turn.session_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                source: Some(AgentSubmissionSource::Cli),
                requester_session_id: None,
                reason: Some(reason.to_string()),
                wait_timeout_ms: Some(1_500),
            })
            .await;
        (turn, result.map(|_| ()).map_err(|error| error.to_string()))
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bitfun_agent_runtime::sdk::{
        PermissionDelegationContext, PermissionRequest, PermissionRequestSource,
        PermissionRequestSourceKind,
    };

    use super::{aggregate_cancellation_results, PeerTurnKey, PeerTurnTracker};

    fn register_background_child(
        tracker: &PeerTurnTracker,
        parent: &PeerTurnKey,
        child: PeerTurnKey,
    ) {
        let tool_call_id = format!("task-{}", child.turn_id);
        assert!(tracker
            .record_background_task_call(parent, tool_call_id.clone())
            .expect("record background Task call"));
        assert!(tracker
            .register_linked_child(parent, child, &tool_call_id)
            .expect("register background child"));
    }

    fn permission_request(session_id: &str, parent_session_id: Option<&str>) -> PermissionRequest {
        PermissionRequest {
            request_id: format!("request-{session_id}"),
            round_id: format!("synthetic:request-{session_id}"),
            order: 0,
            tool_call_id: Some("tool-call".to_string()),
            project_path: None,
            project_id: "project".to_string(),
            session_id: session_id.to_string(),
            agent_id: "Explore".to_string(),
            action: "read".to_string(),
            resources: vec!["README.md".to_string()],
            save_resources: Vec::new(),
            source: PermissionRequestSource {
                kind: PermissionRequestSourceKind::ToolCall,
                identity: "Read".to_string(),
            },
            delegation: parent_session_id.map(|parent_session_id| PermissionDelegationContext {
                parent_session_id: parent_session_id.to_string(),
                parent_dialog_turn_id: Some("parent-turn".to_string()),
                parent_tool_call_id: "parent-task".to_string(),
                subagent_type: "Explore".to_string(),
            }),
            display_metadata: serde_json::Map::new(),
        }
    }

    #[test]
    fn detach_reports_any_unconfirmed_cancellation_round() {
        assert!(aggregate_cancellation_results(Ok(()), Ok(())).is_ok());

        let error =
            aggregate_cancellation_results(Err("initial cancellation failed".to_string()), Ok(()))
                .expect_err("detach must not hide an unconfirmed cancellation");
        assert!(error.contains("initial cancellation failed"), "{error}");

        let error =
            aggregate_cancellation_results(Ok(()), Err("raced cancellation failed".to_string()))
                .expect_err("detach must report a raced cancellation failure");
        assert!(error.contains("raced cancellation failed"), "{error}");
    }

    #[test]
    fn tracker_rejects_turns_until_event_stream_is_ready_and_after_close() {
        let tracker = PeerTurnTracker::new();
        let turn = PeerTurnKey::new("session-1", "turn-1");
        assert!(tracker.register_root(turn.clone()).is_err());

        tracker.mark_event_stream_ready();
        tracker.register_root(turn.clone()).expect("register turn");
        assert_eq!(
            tracker.interrupt_event_stream(true).turns,
            vec![turn.clone()]
        );
        assert!(tracker.register_root(turn).is_err());
    }

    #[test]
    fn permission_ownership_includes_delegated_child_requests_without_leaking_unrelated_sessions() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("parent-session", "parent-turn");
        tracker.register_root(root.clone()).expect("register root");
        assert!(tracker.mark_started(&root));

        assert!(tracker.owns_permission_request(&permission_request("parent-session", None)));
        assert!(tracker
            .owns_permission_request(
                &permission_request("child-session", Some("parent-session"),)
            ));
        assert!(!tracker
            .owns_permission_request(&permission_request("other-child", Some("other-parent"),)));
    }

    #[test]
    fn finishing_a_root_preserves_an_active_child() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        tracker.register_root(root.clone()).expect("register root");
        assert!(tracker
            .register_child(&root, child.clone())
            .expect("register child"));
        tracker.finish_turn(&root);

        assert!(!tracker.owns("session-1", Some("turn-1")));
        assert!(tracker.owns("session-2", Some("turn-2")));
    }

    #[test]
    fn draining_a_parent_session_after_root_completion_returns_the_active_child_only() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        let other = PeerTurnKey::new("session-3", "turn-3");
        tracker.register_root(root.clone()).expect("register root");
        tracker
            .register_child(&root, child.clone())
            .expect("register child");
        tracker
            .register_root(other.clone())
            .expect("register other root");

        tracker.finish_turn(&root);
        let drained = tracker
            .drain_session_turns("session-1")
            .turns
            .into_iter()
            .collect::<HashSet<_>>();

        assert_eq!(drained, HashSet::from([child]));
        assert!(!tracker.owns("session-1", Some("turn-1")));
        assert!(!tracker.owns("session-2", Some("turn-2")));
        assert!(tracker.owns("session-3", Some("turn-3")));
    }

    #[test]
    fn background_result_follow_up_inherits_peer_ownership() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        let follow_up = PeerTurnKey::new("session-1", "turn-3");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());

        tracker.finish_turn(&root);
        tracker.finish_turn(&child);
        let pending = tracker.peer_turns_for_cancellation();
        assert!(pending.turns.is_empty());
        assert_eq!(
            pending
                .background_subagents
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([super::PeerBackgroundSubagentLink {
                parent_session_id: "session-1".to_string(),
                subagent_session_id: "session-2".to_string(),
            }])
        );
        assert!(!tracker
            .register_background_follow_up(
                &PeerTurnKey::new("session-1", "local-turn"),
                &child,
                PeerTurnKey::new("session-1", "local-follow-up")
            )
            .expect("reject another owner's follow-up"));
        assert!(tracker
            .register_background_follow_up(&root, &child, follow_up.clone())
            .expect("register follow-up"));
        assert!(tracker.owns("session-1", Some("turn-3")));

        tracker.finish_turn(&follow_up);
        assert!(!tracker.owns("session-1", None));
        assert!(!tracker
            .register_background_follow_up(&root, &child, PeerTurnKey::new("session-1", "turn-4"))
            .expect("reject unrelated follow-up"));
    }

    #[test]
    fn background_follow_up_can_start_before_terminal_fanout_finishes() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        let follow_up = PeerTurnKey::new("session-1", "turn-3");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());

        assert!(tracker
            .register_background_follow_up(&root, &child, follow_up.clone())
            .expect("register exact follow-up while terminal events are queued"));
        assert!(tracker.owns("session-1", Some("turn-3")));

        tracker.finish_turn(&root);
        tracker.finish_turn(&PeerTurnKey::new("session-2", "turn-2"));
        tracker.finish_turn(&follow_up);
        assert!(!tracker.owns("session-1", None));
        assert!(!tracker
            .register_background_follow_up(
                &root,
                &child,
                PeerTurnKey::new("session-1", "unrelated-follow-up")
            )
            .expect("reject a follow-up after the exact tree is complete"));
    }

    #[test]
    fn completed_source_child_can_register_its_delayed_exact_follow_up() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        let follow_up = PeerTurnKey::new("session-1", "turn-3");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());

        tracker.finish_turn(&child);
        tracker.finish_turn(&root);
        assert!(tracker
            .register_background_follow_up(&root, &child, follow_up.clone())
            .expect("register delayed exact follow-up"));
        assert!(tracker.owns("session-1", Some("turn-3")));

        tracker.finish_turn(&follow_up);
        assert!(!tracker.owns("session-1", None));
        assert!(!tracker
            .register_background_follow_up(
                &root,
                &child,
                PeerTurnKey::new("session-1", "unrelated-follow-up")
            )
            .expect("consumed tombstone must not authorize another follow-up"));
    }

    #[test]
    fn tombstone_only_lineage_is_included_in_cancellation_drain() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());

        tracker.finish_turn(&child);
        tracker.finish_turn(&root);

        let pending = tracker.peer_turns_for_cancellation();
        assert!(pending.turns.is_empty());
        assert_eq!(
            pending
                .background_subagents
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([super::PeerBackgroundSubagentLink {
                parent_session_id: "session-1".to_string(),
                subagent_session_id: "session-2".to_string(),
            }])
        );
        assert_eq!(
            tracker
                .drain_peer_turns()
                .background_subagents
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([super::PeerBackgroundSubagentLink {
                parent_session_id: "session-1".to_string(),
                subagent_session_id: "session-2".to_string(),
            }])
        );
        assert!(!tracker
            .register_background_follow_up(
                &root,
                &child,
                PeerTurnKey::new("session-1", "late-follow-up")
            )
            .expect("drained tombstone must not authorize a late follow-up"));
    }

    #[test]
    fn completed_background_authorizations_do_not_reduce_live_turn_capacity() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        for index in 0..super::MAX_BACKGROUND_PEER_AUTHORIZATIONS {
            let root = PeerTurnKey::new(format!("parent-{index}"), format!("root-{index}"));
            let child = PeerTurnKey::new(format!("child-{index}"), format!("child-turn-{index}"));
            tracker.register_root(root.clone()).expect("register root");
            register_background_child(&tracker, &root, child.clone());
            tracker.finish_turn(&child);
            tracker.finish_turn(&root);
        }

        let mut live_roots = Vec::new();
        for index in 0..super::MAX_TRACKED_PEER_TURNS {
            let root = PeerTurnKey::new(
                format!("live-session-{index}"),
                format!("live-turn-{index}"),
            );
            tracker
                .register_root(root.clone())
                .expect("background history must not reduce live capacity");
            live_roots.push(root);
        }
        assert!(tracker
            .record_background_task_call(&live_roots[0], "overflow-task".to_string())
            .expect_err("the next background authorization must fail closed")
            .contains("background authorization capacity"));
    }

    #[test]
    fn active_capacity_rejects_a_new_root_without_requesting_a_reset() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        for index in 0..super::MAX_TRACKED_PEER_TURNS {
            tracker
                .register_root(PeerTurnKey::new(
                    format!("session-{index}"),
                    format!("turn-{index}"),
                ))
                .expect("register active root");
        }

        assert!(tracker
            .register_root(PeerTurnKey::new("overflow-session", "overflow-turn"))
            .expect_err("live capacity must reject one more root")
            .contains("capacity is exhausted"));
    }

    #[test]
    fn unlinked_background_and_cancellation_task_markers_are_bounded() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session", "turn");
        tracker.register_root(root.clone()).expect("register root");
        for index in 0..super::MAX_BACKGROUND_PEER_AUTHORIZATIONS {
            tracker
                .record_background_task_call(&root, format!("background-{index}"))
                .expect("reserve background authorization");
        }
        assert!(tracker
            .record_background_task_call(&root, "background-overflow".to_string())
            .expect_err("unlinked background calls must be bounded")
            .contains("background authorization capacity"));

        for index in 0..super::MAX_PENDING_PEER_TASK_CANCELLATIONS {
            tracker
                .record_background_task_cancellation(
                    &root,
                    format!("cancel-{index}"),
                    format!("subagent-{index}"),
                )
                .expect("track Task cancellation");
        }
        assert!(tracker
            .record_background_task_cancellation(
                &root,
                "cancel-overflow".to_string(),
                "subagent-overflow".to_string(),
            )
            .expect_err("Task cancellation markers must be bounded")
            .contains("cancellation tracking capacity"));
    }

    #[test]
    fn foreground_children_do_not_consume_background_lineage_capacity() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        for index in 0..(super::MAX_TRACKED_PEER_TURNS * 2) {
            let root = PeerTurnKey::new(format!("parent-{index}"), format!("root-{index}"));
            let child = PeerTurnKey::new(format!("child-{index}"), format!("child-turn-{index}"));
            tracker.register_root(root.clone()).expect("register root");
            tracker
                .register_child(&root, child.clone())
                .expect("register foreground child");
            tracker.finish_turn(&child);
            tracker.finish_turn(&root);
        }

        tracker
            .register_root(PeerTurnKey::new("final-parent", "final-root"))
            .expect("foreground Task history must not exhaust lineage capacity");
    }

    #[test]
    fn successful_task_cancel_releases_the_exact_background_source() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("parent-session", "root-turn");
        let child = PeerTurnKey::new("subagent-session", "child-turn");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());
        tracker.finish_turn(&child);
        tracker.finish_turn(&root);

        let cancel_turn = PeerTurnKey::new("parent-session", "cancel-turn");
        tracker
            .register_root(cancel_turn.clone())
            .expect("register cancel turn");
        assert!(tracker
            .record_background_task_cancellation(
                &cancel_turn,
                "cancel-tool".to_string(),
                "subagent-session".to_string(),
            )
            .expect("record Task cancellation"));
        tracker.finish_task_call(&cancel_turn, "cancel-tool", None, Some(1));

        assert!(!tracker
            .register_background_follow_up(
                &root,
                &child,
                PeerTurnKey::new("parent-session", "late-follow-up"),
            )
            .expect("cancelled background source must not authorize a follow-up"));
    }

    #[test]
    fn zero_count_task_cancel_keeps_a_completed_background_source() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("parent-session", "root-turn");
        let child = PeerTurnKey::new("subagent-session", "child-turn");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());
        tracker.finish_turn(&child);
        tracker.finish_turn(&root);

        let cancel_turn = PeerTurnKey::new("parent-session", "cancel-turn");
        tracker
            .register_root(cancel_turn.clone())
            .expect("register cancel turn");
        tracker
            .record_background_task_cancellation(
                &cancel_turn,
                "cancel-tool".to_string(),
                "subagent-session".to_string(),
            )
            .expect("record Task cancellation");
        tracker.finish_task_call(&cancel_turn, "cancel-tool", None, Some(0));

        assert!(tracker
            .register_background_follow_up(
                &root,
                &child,
                PeerTurnKey::new("parent-session", "follow-up"),
            )
            .expect("zero-count cancellation must not consume the source"));
    }

    #[test]
    fn one_tombstone_follow_up_does_not_consume_a_sibling_authorization() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child_a = PeerTurnKey::new("session-2", "turn-2");
        let child_b = PeerTurnKey::new("session-3", "turn-3");
        let follow_up_a = PeerTurnKey::new("session-1", "turn-4");
        let follow_up_b = PeerTurnKey::new("session-1", "turn-5");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child_a.clone());
        register_background_child(&tracker, &root, child_b.clone());

        tracker.finish_turn(&child_a);
        tracker.finish_turn(&root);
        tracker.finish_turn(&child_b);
        assert!(tracker
            .register_background_follow_up(&root, &child_a, follow_up_a.clone())
            .expect("register child A tombstone follow-up"));
        assert!(tracker
            .register_background_follow_up(&root, &child_b, follow_up_b.clone())
            .expect("register child B follow-up"));

        tracker.finish_turn(&follow_up_a);
        tracker.finish_turn(&follow_up_b);
        assert!(!tracker.owns("session-1", None));
    }

    #[test]
    fn draining_a_child_session_releases_its_early_follow_up_reservation() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        let follow_up = PeerTurnKey::new("session-1", "turn-3");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());
        assert!(tracker
            .register_background_follow_up(&root, &child, follow_up.clone())
            .expect("register exact early follow-up"));

        assert_eq!(
            tracker.drain_session_turns("session-2").turns,
            vec![child.clone()]
        );
        tracker.finish_turn(&root);
        tracker.finish_turn(&follow_up);

        assert!(!tracker
            .register_background_follow_up(
                &root,
                &child,
                PeerTurnKey::new("session-1", "unrelated-follow-up")
            )
            .expect("completed lineage must be pruned"));
    }

    #[test]
    fn draining_a_sibling_child_does_not_release_another_childs_reservation() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child_a = PeerTurnKey::new("session-2", "turn-2");
        let child_b = PeerTurnKey::new("session-3", "turn-3");
        let follow_up = PeerTurnKey::new("session-1", "turn-4");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child_a.clone());
        tracker
            .register_child(&root, child_b.clone())
            .expect("register child B");
        assert!(tracker
            .register_background_follow_up(&root, &child_a, follow_up.clone())
            .expect("register child A follow-up"));

        assert_eq!(
            tracker.drain_session_turns("session-3").turns,
            vec![child_b]
        );
        tracker.finish_turn(&root);
        tracker.finish_turn(&child_a);
        tracker.finish_turn(&follow_up);

        assert!(!tracker
            .register_background_follow_up(
                &root,
                &child_a,
                PeerTurnKey::new("session-1", "unrelated-follow-up")
            )
            .expect("completed lineage must be pruned"));
    }

    #[test]
    fn unrelated_running_turn_does_not_consume_background_follow_up_authorization() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        tracker.register_root(root.clone()).expect("register root");
        register_background_child(&tracker, &root, child.clone());

        tracker.finish_turn(&root);
        tracker.finish_turn(&child);

        let follow_up = PeerTurnKey::new("session-1", "turn-3");
        assert!(tracker
            .register_background_follow_up(&root, &child, follow_up.clone())
            .expect("register queued follow-up"));
        assert!(tracker.owns("session-1", Some("turn-3")));
    }

    #[test]
    fn event_stream_interruption_returns_every_active_turn_once() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("session-1", "turn-1");
        let child = PeerTurnKey::new("session-2", "turn-2");
        tracker.register_root(root.clone()).expect("register root");
        tracker
            .register_child(&root, child.clone())
            .expect("register child");

        assert_eq!(
            tracker
                .interrupt_event_stream(false)
                .turns
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([root, child])
        );
        assert!(tracker.interrupt_event_stream(false).turns.is_empty());
    }

    #[test]
    fn completion_keeps_the_stream_generation_but_interruption_invalidates_it() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let turn = PeerTurnKey::new("session-1", "turn-1");
        let generation = tracker.register_root(turn.clone()).expect("register root");

        tracker.finish_turn(&turn);
        assert!(tracker.is_event_stream_generation_current(generation));

        tracker.interrupt_event_stream(false);
        tracker.mark_event_stream_ready();
        assert!(!tracker.is_event_stream_generation_current(generation));
    }

    #[test]
    fn duplicate_root_ids_are_rejected() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let turn = PeerTurnKey::new("session-1", "turn-1");
        tracker.register_root(turn.clone()).expect("register root");

        assert!(tracker.register_root(turn).is_err());
    }

    #[test]
    fn terminal_delivery_is_claimed_once_and_cleared_with_the_turn() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let turn = PeerTurnKey::new("session-1", "turn-1");
        tracker.register_root(turn.clone()).expect("register root");

        assert!(tracker
            .claim_terminal_delivery(&turn)
            .expect("claim terminal"));
        assert!(!tracker
            .claim_terminal_delivery(&turn)
            .expect("reject duplicate terminal"));
        tracker.finish_turn(&turn);
        assert!(!tracker
            .claim_terminal_delivery(&turn)
            .expect("finished turn is not claimable"));
    }

    #[test]
    fn interrupted_turn_id_remains_quarantined_after_late_terminals() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let interrupted = PeerTurnKey::new("session-1", "turn-1");
        tracker
            .register_root(interrupted.clone())
            .expect("register root");

        tracker.interrupt_event_stream(false);
        tracker.mark_event_stream_ready();
        assert!(tracker.register_root(interrupted.clone()).is_err());
        assert!(tracker.is_interrupted_terminal(&interrupted));
        assert!(tracker.register_root(interrupted).is_err());
    }

    #[test]
    fn completed_background_source_is_released_by_its_exact_injection() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let parent = PeerTurnKey::new("parent-session", "parent-turn");
        let source = PeerTurnKey::new("subagent-session", "subagent-turn");
        tracker
            .register_root(parent.clone())
            .expect("register parent");
        tracker
            .record_background_task_call(&parent, "task-tool".to_string())
            .expect("record background Task");
        tracker
            .register_linked_child(&parent, source.clone(), "task-tool")
            .expect("link source");
        tracker.finish_task_call(&parent, "task-tool", Some("background-task"), None);
        tracker.finish_turn(&source);

        assert!(tracker.finish_background_injection(&parent, "background-task"));
        assert!(!tracker.finish_background_injection(&parent, "background-task"));
        assert!(!tracker
            .register_background_follow_up(
                &parent,
                &source,
                PeerTurnKey::new("parent-session", "late-follow-up"),
            )
            .unwrap_or(false));
    }

    #[test]
    fn injected_background_results_do_not_accumulate_authorization() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let parent = PeerTurnKey::new("parent-session", "parent-turn");
        tracker
            .register_root(parent.clone())
            .expect("register parent");

        for index in 0..(super::MAX_BACKGROUND_PEER_AUTHORIZATIONS * 2) {
            let tool_call_id = format!("task-tool-{index}");
            let background_task_id = format!("background-task-{index}");
            let source = PeerTurnKey::new("subagent-session", format!("subagent-turn-{index}"));
            tracker
                .record_background_task_call(&parent, tool_call_id.clone())
                .expect("record background Task");
            tracker
                .register_linked_child(&parent, source.clone(), &tool_call_id)
                .expect("link source");
            tracker.finish_task_call(&parent, &tool_call_id, Some(&background_task_id), None);
            tracker.finish_turn(&source);
            assert!(tracker.finish_background_injection(&parent, &background_task_id));
        }
    }

    #[test]
    fn interrupted_turn_quarantine_does_not_reduce_live_turn_capacity() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        tracker
            .register_root(PeerTurnKey::new("old-session", "old-turn"))
            .expect("register interrupted root");
        tracker.interrupt_event_stream(false);
        tracker.mark_event_stream_ready();

        for index in 0..super::MAX_TRACKED_PEER_TURNS {
            tracker
                .register_root(PeerTurnKey::new(
                    format!("session-{index}"),
                    format!("turn-{index}"),
                ))
                .expect("quarantine must not reduce live capacity");
        }
    }

    #[test]
    fn interrupted_child_turn_id_cannot_be_reused() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let old_root = PeerTurnKey::new("old-parent", "old-root");
        let interrupted_child = PeerTurnKey::new("subagent", "shared-child-turn");
        tracker
            .register_root(old_root.clone())
            .expect("register old root");
        tracker
            .register_child(&old_root, interrupted_child.clone())
            .expect("register old child");
        tracker.interrupt_event_stream(false);
        tracker.mark_event_stream_ready();

        let new_root = PeerTurnKey::new("new-parent", "new-root");
        tracker
            .register_root(new_root.clone())
            .expect("register new root");
        assert!(tracker
            .register_child(&new_root, interrupted_child)
            .is_err());
    }

    #[test]
    fn interrupted_follow_up_turn_id_cannot_be_reused() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let interrupted_follow_up = PeerTurnKey::new("parent-session", "shared-follow-up");
        tracker
            .register_root(interrupted_follow_up.clone())
            .expect("register interrupted turn");
        tracker.interrupt_event_stream(false);
        tracker.mark_event_stream_ready();

        let parent = PeerTurnKey::new("parent-session", "new-parent");
        let source = PeerTurnKey::new("subagent-session", "source-turn");
        tracker
            .register_root(parent.clone())
            .expect("register new parent");
        register_background_child(&tracker, &parent, source.clone());
        assert!(tracker
            .register_background_follow_up(&parent, &source, interrupted_follow_up)
            .is_err());
    }

    #[test]
    fn explicit_drains_quarantine_removed_turn_ids() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let root = PeerTurnKey::new("parent-session", "root-turn");
        let child = PeerTurnKey::new("child-session", "child-turn");
        tracker.register_root(root.clone()).expect("register root");
        tracker
            .register_child(&root, child.clone())
            .expect("register child");

        tracker.drain_session_turns(&child.session_id);
        assert!(tracker.register_child(&root, child).is_err());

        tracker.drain_peer_turns();
        assert!(tracker.register_root(root).is_err());
    }
}

static PEER_HOST_STATE: OnceLock<PeerHostState> = OnceLock::new();

pub(crate) fn set_peer_host_state(state: PeerHostState) -> Result<(), PeerHostState> {
    PEER_HOST_STATE.set(state)
}

pub(crate) fn try_peer_host_state() -> Option<&'static PeerHostState> {
    PEER_HOST_STATE.get()
}

pub(crate) fn peer_host_state() -> Result<&'static PeerHostState, String> {
    try_peer_host_state().ok_or_else(|| "CLI peer host is not initialized".to_string())
}
