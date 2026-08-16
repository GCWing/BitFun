//! Runtime-owned, materialized event projection for reconnecting Session clients.
//!
//! Live broadcasts are intentionally bounded and ephemeral. A GUI, TUI, or
//! remote controller can therefore miss events while it is detached without
//! affecting the Dialog Turn that continues to run. This journal keeps the
//! smallest reconstructable projection of the current Turn, independently of
//! any client subscription or client-written persistence checkpoint.

use bitfun_events::{AgenticEvent, ToolEventData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

const MAX_PROJECTED_EVENTS_PER_SESSION: usize = 2048;
const MAX_RETAINED_TERMINAL_SESSION_PROJECTIONS: usize = 64;

/// Reserved metadata carried only by the product delivery envelope. Existing
/// frontend event payload fields remain unchanged for older clients.
pub const RUNTIME_EVENT_STREAM_ID_KEY: &str = "__bitfunRuntimeStreamId";
pub const RUNTIME_EVENT_CURSOR_KEY: &str = "__bitfunRuntimeEventCursor";

/// Add the cursor to an existing frontend event payload without changing its
/// stable product fields. Non-object payloads cannot identify a Session and
/// are left untouched.
pub fn attach_session_event_cursor(
    payload: &mut serde_json::Value,
    cursor: SessionEventCursor,
) -> bool {
    let Some(payload) = payload.as_object_mut() else {
        return false;
    };
    payload.insert(
        RUNTIME_EVENT_STREAM_ID_KEY.to_string(),
        serde_json::Value::String(cursor.stream_id),
    );
    payload.insert(
        RUNTIME_EVENT_CURSOR_KEY.to_string(),
        serde_json::Value::Number(cursor.cursor.into()),
    );
    true
}

/// Cursor attached to one event after it enters the host's ordered delivery
/// stream. `stream_id` changes when the Runtime Host process restarts, so a
/// cursor from an older process can never be mistaken for current progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEventCursor {
    pub stream_id: String,
    pub cursor: u64,
}

/// Coherent materialized projection of the currently executing Turn.
///
/// `events` are deliberately the existing authoritative `AgenticEvent`
/// contract. Text/thinking chunks are accumulated by stream and noisy tool
/// progress facts are compacted, so attachment cost is bounded by semantic
/// state rather than by how long a client was disconnected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEventProjectionSnapshot {
    pub session_id: String,
    pub stream_id: String,
    pub cursor: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_turn_id: Option<String>,
    #[serde(default)]
    pub events: Vec<AgenticEvent>,
}

#[derive(Default)]
struct SessionProjection {
    cursor: u64,
    active_turn_id: Option<String>,
    events: Vec<AgenticEvent>,
    terminal: bool,
    last_touched: u64,
}

struct JournalState {
    stream_id: String,
    sequence: u64,
    sessions: HashMap<String, SessionProjection>,
}

/// Durable sink for the materialized projection.
///
/// This projection is the only complete record of a Turn while it runs: client
/// persistence lags it by a coalescing window, and the persisted Session view
/// deliberately stores an executing Turn as idle so a restart cannot revive
/// work. Keeping the projection in memory alone therefore means a Host restart
/// loses the live state of work that is still running, and a client returning
/// to that Session sees a Turn frozen at the last checkpoint.
///
/// Implementations own their write policy. `save` is called for every
/// materialized event, so a store is expected to coalesce writes rather than
/// touch the filesystem per token.
pub trait SessionEventProjectionStore: Send + Sync {
    fn save(&self, snapshot: &SessionEventProjectionSnapshot);
    fn load(&self, session_id: &str) -> Option<SessionEventProjectionSnapshot>;
    /// The Turn reached a terminal state; the persisted Session view owns it now.
    fn discard(&self, session_id: &str);
}

/// Cloneable owner shared by the Runtime facade and its product delivery
/// adapter. Product hosts record events only after their ordering/coalescing
/// boundary, then expose snapshots through the same `AgentRuntime` instance.
#[derive(Clone)]
pub struct SessionEventJournal {
    state: Arc<Mutex<JournalState>>,
    store: Option<Arc<dyn SessionEventProjectionStore>>,
}

impl std::fmt::Debug for SessionEventJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionEventJournal")
            .finish_non_exhaustive()
    }
}

impl Default for SessionEventJournal {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionEventJournal {
    pub fn new() -> Self {
        Self::with_stream_id(uuid::Uuid::new_v4().to_string())
    }

    fn with_stream_id(stream_id: String) -> Self {
        Self {
            state: Arc::new(Mutex::new(JournalState {
                stream_id,
                sequence: 0,
                sessions: HashMap::new(),
            })),
            store: None,
        }
    }

    /// Attach durable storage so a projection outlives this Host process.
    pub fn with_store(mut self, store: Arc<dyn SessionEventProjectionStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Record one event in the host's final ordered delivery stream.
    ///
    /// A returned cursor must travel with that same live event. `None` means
    /// the event is not represented by this projection; attachment must always
    /// release that live event instead of letting a snapshot cursor cover it.
    pub fn record(&self, event: &AgenticEvent) -> Option<SessionEventCursor> {
        let session_id = event.session_id()?.to_string();
        let mut state = lock_state(&self.state);
        if matches!(event, AgenticEvent::SessionDeleted { .. }) {
            if let Some(projection) = state.sessions.get_mut(&session_id) {
                projection.events.clear();
                projection.active_turn_id = None;
                projection.terminal = true;
            }
            return None;
        }
        if event_turn_id(event).is_none()
            || matches!(
                event,
                AgenticEvent::ToolEvent {
                    tool_event: ToolEventData::StreamChunk { .. },
                    ..
                }
            )
        {
            return None;
        }
        let stream_id = state.stream_id.clone();
        let next_sequence = state.sequence.saturating_add(1);
        let (cursor, materialized) = {
            let projection = state.sessions.entry(session_id).or_default();
            let materialized = apply_event(projection, event);
            if materialized {
                projection.cursor = projection.cursor.saturating_add(1);
                projection.last_touched = next_sequence;
            }
            (projection.cursor, materialized)
        };
        if materialized {
            state.sequence = next_sequence;
            prune_terminal_projections(&mut state.sessions);
        }
        if materialized {
            if let Some(store) = self.store.as_ref() {
                let session_id = event.session_id().unwrap_or_default().to_string();
                let projection = state.sessions.get(&session_id);
                let terminal = projection.is_some_and(|projection| projection.terminal);
                let snapshot = projection_snapshot(&state, &session_id);
                // Release the lock before touching the store: a slow flush must
                // never stall the delivery stream it is recording.
                drop(state);
                if terminal {
                    store.discard(&session_id);
                } else {
                    store.save(&snapshot);
                }
                return Some(SessionEventCursor { stream_id, cursor });
            }
        }
        materialized.then_some(SessionEventCursor { stream_id, cursor })
    }

    pub fn snapshot(&self, session_id: &str) -> SessionEventProjectionSnapshot {
        let state = lock_state(&self.state);
        if state.sessions.contains_key(session_id) {
            return projection_snapshot(&state, session_id);
        }
        drop(state);

        // Nothing in memory. Either this Session never ran here, or this Host
        // restarted while its Turn was executing. A stored projection keeps its
        // original `stream_id`, so a client correctly treats it as a different
        // Runtime process and replays it whole instead of comparing cursors
        // across processes.
        self.store
            .as_ref()
            .and_then(|store| store.load(session_id))
            .unwrap_or_else(|| SessionEventProjectionSnapshot {
                session_id: session_id.to_string(),
                stream_id: lock_state(&self.state).stream_id.clone(),
                cursor: 0,
                active_turn_id: None,
                events: Vec::new(),
            })
    }
}

fn projection_snapshot(state: &JournalState, session_id: &str) -> SessionEventProjectionSnapshot {
    let projection = state.sessions.get(session_id);
    SessionEventProjectionSnapshot {
        session_id: session_id.to_string(),
        stream_id: state.stream_id.clone(),
        cursor: projection.map_or(0, |projection| projection.cursor),
        active_turn_id: projection.and_then(|projection| projection.active_turn_id.clone()),
        events: projection.map_or_else(Vec::new, |projection| projection.events.clone()),
    }
}

fn lock_state(state: &Mutex<JournalState>) -> MutexGuard<'_, JournalState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn event_turn_id(event: &AgenticEvent) -> Option<&str> {
    match event {
        AgenticEvent::DialogTurnStarted { turn_id, .. }
        | AgenticEvent::DialogTurnCompleted { turn_id, .. }
        | AgenticEvent::DialogTurnCancelled { turn_id, .. }
        | AgenticEvent::DialogTurnInterrupted { turn_id, .. }
        | AgenticEvent::DialogTurnRecovered { turn_id, .. }
        | AgenticEvent::DialogTurnFailed { turn_id, .. }
        | AgenticEvent::TokenUsageUpdated { turn_id, .. }
        | AgenticEvent::ContextCompressionStarted { turn_id, .. }
        | AgenticEvent::ContextCompressionCompleted { turn_id, .. }
        | AgenticEvent::ContextCompressionFailed { turn_id, .. }
        | AgenticEvent::ModelRoundStarted { turn_id, .. }
        | AgenticEvent::ModelRoundAttemptSuperseded { turn_id, .. }
        | AgenticEvent::ModelRoundCompleted { turn_id, .. }
        | AgenticEvent::TextChunk { turn_id, .. }
        | AgenticEvent::ThinkingChunk { turn_id, .. }
        | AgenticEvent::ToolEvent { turn_id, .. }
        | AgenticEvent::DeepReviewQueueStateChanged { turn_id, .. }
        | AgenticEvent::UserSteeringInjected { turn_id, .. } => Some(turn_id),
        AgenticEvent::SubagentSessionLinked {
            subagent_dialog_turn_id,
            ..
        } => Some(subagent_dialog_turn_id),
        _ => None,
    }
}

fn apply_event(projection: &mut SessionProjection, event: &AgenticEvent) -> bool {
    if let AgenticEvent::DialogTurnStarted { turn_id, .. } = event {
        if projection.active_turn_id.as_deref() != Some(turn_id) {
            projection.events.clear();
            projection.active_turn_id = Some(turn_id.clone());
        }
        projection.terminal = false;
        replace_unique(
            &mut projection.events,
            event.clone(),
            |candidate| matches!(candidate, AgenticEvent::DialogTurnStarted { turn_id: candidate_turn, .. } if candidate_turn == turn_id),
        );
        return true;
    }

    let Some(turn_id) = event_turn_id(event) else {
        return false;
    };
    if projection.active_turn_id.as_deref() != Some(turn_id) {
        // A Turn may begin before a particular product delivery adapter is
        // installed. Start a scoped projection on the first concrete Turn
        // event instead of mixing it with an older Turn.
        projection.events.clear();
        projection.active_turn_id = Some(turn_id.to_string());
        projection.terminal = false;
    }

    match event {
        AgenticEvent::TextChunk {
            turn_id,
            round_id,
            attempt_id,
            attempt_index,
            text,
            ..
        } => {
            if let Some(AgenticEvent::TextChunk {
                text: accumulated, ..
            }) = projection.events.last_mut().filter(|candidate| {
                matches!(candidate, AgenticEvent::TextChunk {
                    turn_id: candidate_turn,
                    round_id: candidate_round,
                    attempt_id: candidate_attempt,
                    attempt_index: candidate_attempt_index,
                    ..
                } if candidate_turn == turn_id
                    && candidate_round == round_id
                    && candidate_attempt == attempt_id
                    && candidate_attempt_index == attempt_index)
            }) {
                accumulated.push_str(text);
            } else {
                projection.events.push(event.clone());
            }
        }
        AgenticEvent::ThinkingChunk {
            turn_id,
            round_id,
            attempt_id,
            attempt_index,
            content,
            is_end,
            ..
        } => {
            if let Some(AgenticEvent::ThinkingChunk {
                content: accumulated,
                is_end: accumulated_end,
                ..
            }) = projection.events.last_mut().filter(|candidate| {
                matches!(candidate, AgenticEvent::ThinkingChunk {
                    turn_id: candidate_turn,
                    round_id: candidate_round,
                    attempt_id: candidate_attempt,
                    attempt_index: candidate_attempt_index,
                    ..
                } if candidate_turn == turn_id
                    && candidate_round == round_id
                    && candidate_attempt == attempt_id
                    && candidate_attempt_index == attempt_index)
            }) {
                accumulated.push_str(content);
                *accumulated_end |= *is_end;
            } else {
                projection.events.push(event.clone());
            }
        }
        AgenticEvent::ModelRoundStarted { round_id, .. } => {
            replace_unique(
                &mut projection.events,
                event.clone(),
                |candidate| matches!(candidate, AgenticEvent::ModelRoundStarted { round_id: candidate_round, .. } if candidate_round == round_id),
            );
        }
        AgenticEvent::ModelRoundCompleted { round_id, .. } => {
            replace_unique(
                &mut projection.events,
                event.clone(),
                |candidate| matches!(candidate, AgenticEvent::ModelRoundCompleted { round_id: candidate_round, .. } if candidate_round == round_id),
            );
        }
        AgenticEvent::TokenUsageUpdated { .. } => {
            replace_unique(&mut projection.events, event.clone(), |candidate| {
                matches!(candidate, AgenticEvent::TokenUsageUpdated { .. })
            });
        }
        AgenticEvent::ToolEvent {
            tool_event: ToolEventData::ParamsPartial { identity, params },
            ..
        } => {
            if let Some(AgenticEvent::ToolEvent {
                tool_event:
                    ToolEventData::ParamsPartial {
                        params: accumulated,
                        ..
                    },
                ..
            }) = projection.events.iter_mut().find(|candidate| {
                matches!(candidate, AgenticEvent::ToolEvent {
                    tool_event: ToolEventData::ParamsPartial { identity: candidate_identity, .. },
                    ..
                } if candidate_identity.tool_id == identity.tool_id)
            }) {
                accumulated.push_str(params);
            } else {
                projection.events.push(event.clone());
            }
        }
        AgenticEvent::ToolEvent { tool_event, .. } if compactable_tool_event(tool_event) => {
            let tool_id = tool_event_identity(tool_event);
            replace_unique(&mut projection.events, event.clone(), |candidate| {
                matches!(candidate, AgenticEvent::ToolEvent { tool_event: candidate_event, .. }
                    if compactable_tool_event(candidate_event)
                        && tool_event_identity(candidate_event) == tool_id
                        && std::mem::discriminant(candidate_event) == std::mem::discriminant(tool_event))
            });
        }
        AgenticEvent::ToolEvent { tool_event, .. } => {
            let tool_id = tool_event_identity(tool_event);
            replace_unique(&mut projection.events, event.clone(), |candidate| {
                matches!(candidate, AgenticEvent::ToolEvent { tool_event: candidate_event, .. }
                    if tool_event_identity(candidate_event) == tool_id
                        && std::mem::discriminant(candidate_event) == std::mem::discriminant(tool_event))
            });
        }
        _ => projection.events.push(event.clone()),
    }

    compact_projection(projection);
    if matches!(
        event,
        AgenticEvent::DialogTurnCompleted { .. }
            | AgenticEvent::DialogTurnCancelled { .. }
            | AgenticEvent::DialogTurnFailed { .. }
    ) {
        projection.terminal = true;
    }
    true
}

fn prune_terminal_projections(sessions: &mut HashMap<String, SessionProjection>) {
    let mut retained = sessions
        .iter()
        .filter_map(|(session_id, projection)| {
            (projection.terminal && !projection.events.is_empty())
                .then_some((session_id.clone(), projection.last_touched))
        })
        .collect::<Vec<_>>();
    if retained.len() <= MAX_RETAINED_TERMINAL_SESSION_PROJECTIONS {
        return;
    }
    retained.sort_unstable_by(|left, right| right.1.cmp(&left.1));
    for (session_id, _) in retained
        .into_iter()
        .skip(MAX_RETAINED_TERMINAL_SESSION_PROJECTIONS)
    {
        if let Some(projection) = sessions.get_mut(&session_id) {
            // Keep the per-Session cursor tombstone monotonic while releasing
            // replay content that persisted idle history can reconstruct.
            projection.events.clear();
            projection.active_turn_id = None;
        }
    }
}

fn replace_unique(
    events: &mut Vec<AgenticEvent>,
    replacement: AgenticEvent,
    matches: impl Fn(&AgenticEvent) -> bool,
) {
    if let Some(index) = events.iter().position(matches) {
        events[index] = replacement;
    } else {
        events.push(replacement);
    }
}

fn compactable_tool_event(event: &ToolEventData) -> bool {
    matches!(
        event,
        ToolEventData::EarlyDetected { .. }
            | ToolEventData::Queued { .. }
            | ToolEventData::Waiting { .. }
            | ToolEventData::Progress { .. }
            | ToolEventData::Streaming { .. }
    )
}

fn tool_event_identity(event: &ToolEventData) -> &str {
    match event {
        ToolEventData::EarlyDetected { identity }
        | ToolEventData::ParamsPartial { identity, .. }
        | ToolEventData::Queued { identity, .. }
        | ToolEventData::Waiting { identity, .. }
        | ToolEventData::Started { identity, .. }
        | ToolEventData::Progress { identity, .. }
        | ToolEventData::Streaming { identity, .. }
        | ToolEventData::StreamChunk { identity, .. }
        | ToolEventData::ConfirmationNeeded { identity, .. }
        | ToolEventData::Confirmed { identity }
        | ToolEventData::Rejected { identity }
        | ToolEventData::Completed { identity, .. }
        | ToolEventData::Failed { identity, .. }
        | ToolEventData::Cancelled { identity, .. } => &identity.tool_id,
    }
}

fn compact_projection(projection: &mut SessionProjection) {
    if projection.events.len() <= MAX_PROJECTED_EVENTS_PER_SESSION {
        return;
    }
    let excess = projection.events.len() - MAX_PROJECTED_EVENTS_PER_SESSION;
    let mut removable = projection
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (index > 0
                && matches!(
                    event,
                    AgenticEvent::ToolEvent {
                        tool_event: ToolEventData::Progress { .. }
                            | ToolEventData::Streaming { .. }
                            | ToolEventData::StreamChunk { .. },
                        ..
                    }
                ))
            .then_some(index)
        })
        .take(excess)
        .collect::<Vec<_>>();
    if removable.len() < excess {
        let fallback = (1..projection.events.len())
            .filter(|index| !removable.contains(index))
            .take(excess - removable.len())
            .collect::<Vec<_>>();
        removable.extend(fallback);
    }
    removable.sort_unstable_by(|left, right| right.cmp(left));
    for index in removable {
        projection.events.remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        attach_session_event_cursor, lock_state, SessionEventCursor, SessionEventJournal,
        SessionEventProjectionSnapshot, SessionEventProjectionStore,
        MAX_RETAINED_TERMINAL_SESSION_PROJECTIONS,
    };
    use bitfun_events::{AgenticEvent, ToolEventData, ToolEventIdentity};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// In-memory stand-in for a durable store; records the calls a real one
    /// would turn into filesystem writes.
    #[derive(Default)]
    struct RecordingStore {
        saved: Mutex<HashMap<String, SessionEventProjectionSnapshot>>,
        discarded: Mutex<Vec<String>>,
    }

    impl SessionEventProjectionStore for RecordingStore {
        fn save(&self, snapshot: &SessionEventProjectionSnapshot) {
            self.saved
                .lock()
                .unwrap()
                .insert(snapshot.session_id.clone(), snapshot.clone());
        }

        fn load(&self, session_id: &str) -> Option<SessionEventProjectionSnapshot> {
            self.saved.lock().unwrap().get(session_id).cloned()
        }

        fn discard(&self, session_id: &str) {
            self.discarded.lock().unwrap().push(session_id.to_string());
            self.saved.lock().unwrap().remove(session_id);
        }
    }

    fn turn_started(session_id: &str, turn_id: &str) -> AgenticEvent {
        AgenticEvent::DialogTurnStarted {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            turn_index: 0,
            user_input: "prompt".to_string(),
            original_user_input: None,
            user_message_metadata: None,
        }
    }

    fn text(session_id: &str, turn_id: &str, value: &str) -> AgenticEvent {
        AgenticEvent::TextChunk {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            round_id: "round".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: value.to_string(),
        }
    }

    fn turn_completed(session_id: &str, turn_id: &str) -> AgenticEvent {
        AgenticEvent::DialogTurnCompleted {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            total_rounds: 1,
            total_tools: 0,
            duration_ms: 1,
            partial_recovery_reason: None,
            success: Some(true),
            finish_reason: Some("stop".to_string()),
            has_final_response: Some(true),
        }
    }

    fn tool(session_id: &str, turn_id: &str, tool_event: ToolEventData) -> AgenticEvent {
        AgenticEvent::ToolEvent {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            round_id: "round".to_string(),
            attempt_id: None,
            attempt_index: None,
            tool_event,
        }
    }

    #[test]
    fn materializes_stream_content_without_a_subscriber() {
        let journal = SessionEventJournal::with_stream_id("runtime-a".to_string());
        let first = journal.record(&turn_started("session", "turn")).unwrap();
        let second = journal.record(&text("session", "turn", "hel")).unwrap();
        let third = journal.record(&text("session", "turn", "lo")).unwrap();

        assert_eq!(first.cursor, 1);
        assert_eq!(second.cursor, 2);
        assert_eq!(third.cursor, 3);
        assert_eq!(third.stream_id, "runtime-a");

        let snapshot = journal.snapshot("session");
        assert_eq!(snapshot.active_turn_id.as_deref(), Some("turn"));
        assert_eq!(snapshot.cursor, 3);
        assert_eq!(snapshot.events.len(), 2);
        assert!(matches!(
            &snapshot.events[1],
            AgenticEvent::TextChunk { text, .. } if text == "hello"
        ));
    }

    #[test]
    fn a_new_turn_replaces_the_previous_turn_projection() {
        let journal = SessionEventJournal::with_stream_id("runtime-a".to_string());
        journal.record(&turn_started("session", "turn-1"));
        journal.record(&text("session", "turn-1", "old"));
        journal.record(&turn_started("session", "turn-2"));

        let snapshot = journal.snapshot("session");
        assert_eq!(snapshot.cursor, 3);
        assert_eq!(snapshot.active_turn_id.as_deref(), Some("turn-2"));
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            &snapshot.events[0],
            AgenticEvent::DialogTurnStarted { turn_id, .. } if turn_id == "turn-2"
        ));
    }

    #[test]
    fn session_projections_and_cursors_are_isolated() {
        let journal = SessionEventJournal::with_stream_id("runtime-a".to_string());
        journal.record(&turn_started("session-a", "turn-a"));
        journal.record(&turn_started("session-b", "turn-b"));
        journal.record(&text("session-a", "turn-a", "a"));

        assert_eq!(journal.snapshot("session-a").cursor, 2);
        assert_eq!(journal.snapshot("session-b").cursor, 1);
        assert_eq!(journal.snapshot("missing").cursor, 0);
    }

    #[test]
    fn preserves_text_segments_separated_by_a_tool() {
        let journal = SessionEventJournal::with_stream_id("runtime-a".to_string());
        journal.record(&turn_started("session", "turn"));
        journal.record(&text("session", "turn", "before"));
        journal.record(&tool(
            "session",
            "turn",
            ToolEventData::EarlyDetected {
                identity: ToolEventIdentity::direct("tool", "Read"),
            },
        ));
        journal.record(&text("session", "turn", "after"));

        let snapshot = journal.snapshot("session");
        assert_eq!(snapshot.events.len(), 4);
        assert!(matches!(
            &snapshot.events[1],
            AgenticEvent::TextChunk { text, .. } if text == "before"
        ));
        assert!(matches!(
            &snapshot.events[3],
            AgenticEvent::TextChunk { text, .. } if text == "after"
        ));
    }

    #[test]
    fn materializes_partial_tool_params_and_discards_noop_stream_chunks() {
        let journal = SessionEventJournal::with_stream_id("runtime-a".to_string());
        journal.record(&turn_started("session", "turn"));
        for params in ["{\"path\":\"", "file.rs\"}"] {
            journal.record(&tool(
                "session",
                "turn",
                ToolEventData::ParamsPartial {
                    identity: ToolEventIdentity::direct("tool", "Read"),
                    params: params.to_string(),
                },
            ));
        }
        journal.record(&tool(
            "session",
            "turn",
            ToolEventData::StreamChunk {
                identity: ToolEventIdentity::direct("tool", "Read"),
                data: serde_json::json!({ "ignored": true }),
            },
        ));

        let snapshot = journal.snapshot("session");
        assert_eq!(snapshot.cursor, 3);
        assert_eq!(snapshot.events.len(), 2);
        assert!(matches!(
            &snapshot.events[1],
            AgenticEvent::ToolEvent {
                tool_event: ToolEventData::ParamsPartial { params, .. },
                ..
            } if params == "{\"path\":\"file.rs\"}"
        ));
    }

    #[test]
    fn cursor_metadata_is_additive_to_product_payloads() {
        let mut payload = serde_json::json!({ "sessionId": "session", "text": "hello" });
        assert!(attach_session_event_cursor(
            &mut payload,
            SessionEventCursor {
                stream_id: "runtime-a".to_string(),
                cursor: 42,
            },
        ));
        assert_eq!(payload["sessionId"], "session");
        assert_eq!(payload["text"], "hello");
        assert_eq!(payload["__bitfunRuntimeStreamId"], "runtime-a");
        assert_eq!(payload["__bitfunRuntimeEventCursor"], 42);
    }

    #[test]
    fn non_materialized_session_events_are_not_cursor_fenced() {
        let journal = SessionEventJournal::with_stream_id("runtime-a".to_string());
        let cursor = journal.record(&AgenticEvent::SessionTitleGenerated {
            session_id: "session".to_string(),
            title: "A title that must stay live".to_string(),
            method: "model".to_string(),
        });

        assert!(cursor.is_none());
        let snapshot = journal.snapshot("session");
        assert_eq!(snapshot.cursor, 0);
        assert!(snapshot.events.is_empty());
        assert!(lock_state(&journal.state).sessions.is_empty());
    }

    #[test]
    fn terminal_projection_retention_is_bounded_without_resetting_cursors() {
        let journal = SessionEventJournal::with_stream_id("runtime-a".to_string());
        for index in 0..=MAX_RETAINED_TERMINAL_SESSION_PROJECTIONS {
            let session_id = format!("session-{index}");
            let turn_id = format!("turn-{index}");
            journal.record(&turn_started(&session_id, &turn_id));
            journal.record(&turn_completed(&session_id, &turn_id));
        }

        let evicted = journal.snapshot("session-0");
        assert_eq!(evicted.cursor, 2);
        assert!(evicted.active_turn_id.is_none());
        assert!(evicted.events.is_empty());

        let retained = journal.snapshot(&format!(
            "session-{}",
            MAX_RETAINED_TERMINAL_SESSION_PROJECTIONS
        ));
        assert_eq!(retained.cursor, 2);
        assert!(!retained.events.is_empty());
    }

    #[test]
    fn persists_the_projection_of_a_running_turn() {
        let store = Arc::new(RecordingStore::default());
        let journal = SessionEventJournal::new().with_store(store.clone());

        journal.record(&turn_started("session-1", "turn-1"));
        journal.record(&text("session-1", "turn-1", "hello"));

        let saved = store.load("session-1").expect("running turn is persisted");
        assert_eq!(saved.active_turn_id.as_deref(), Some("turn-1"));
        assert!(saved.cursor > 0);
    }

    #[test]
    fn serves_a_stored_projection_after_a_host_restart() {
        // The reported freeze: the Turn keeps running but the Host process that
        // owned its in-memory projection is gone, so a returning client used to
        // see only the lagging persisted checkpoint.
        let store = Arc::new(RecordingStore::default());
        let first = SessionEventJournal::new().with_store(store.clone());
        first.record(&turn_started("session-1", "turn-1"));
        first.record(&text("session-1", "turn-1", "hello"));
        let before = first.snapshot("session-1");

        let restarted = SessionEventJournal::new().with_store(store.clone());
        let after = restarted.snapshot("session-1");

        assert_eq!(after.active_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(after.events.len(), before.events.len());
        // Its own stream id survives, so a client treats it as a different
        // Runtime process and replays it instead of comparing cursors.
        assert_eq!(after.stream_id, before.stream_id);
        assert_ne!(after.stream_id, restarted.snapshot("session-2").stream_id);
    }

    #[test]
    fn stops_persisting_once_the_turn_is_terminal() {
        let store = Arc::new(RecordingStore::default());
        let journal = SessionEventJournal::new().with_store(store.clone());

        journal.record(&turn_started("session-1", "turn-1"));
        journal.record(&turn_completed("session-1", "turn-1"));

        assert!(store
            .discarded
            .lock()
            .unwrap()
            .contains(&"session-1".to_string()));
        assert!(store.load("session-1").is_none());
    }

    #[test]
    fn a_journal_without_a_store_behaves_as_before() {
        let journal = SessionEventJournal::new();
        journal.record(&turn_started("session-1", "turn-1"));
        assert_eq!(
            journal.snapshot("session-1").active_turn_id.as_deref(),
            Some("turn-1")
        );
        assert!(journal.snapshot("missing").events.is_empty());
    }
}
