//! DeviceEvent fan-out to attached Peer Mode controllers.

use std::collections::HashSet;
use std::sync::OnceLock;

use bitfun_agent_runtime::sdk::PermissionRequestEvent;
use bitfun_agent_tools::effective_tool_invocation;
use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
use bitfun_events::{project_agentic_frontend_event, AgenticEvent, ToolEventData};
use tokio::sync::{broadcast, mpsc};

use crate::account::PeerFanoutOwner;

use super::control::{attached_controllers, controller_delivery_lease};
use super::state::{PeerHostState, PeerTurnKey};

const PEER_EVENT_DELIVERY_CAPACITY: usize = 512;

struct QueuedPeerDeviceEvent {
    owner: PeerFanoutOwner,
    targets: Vec<String>,
    event: String,
    payload: serde_json::Value,
    continuity: Option<(super::state::PeerTurnTracker, u64)>,
    terminal: Option<(super::state::PeerTurnTracker, u64, PeerTurnKey)>,
}

impl QueuedPeerDeviceEvent {
    fn new(
        owner: PeerFanoutOwner,
        targets: Vec<String>,
        event: String,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            owner,
            targets,
            event,
            payload,
            continuity: None,
            terminal: None,
        }
    }

    fn for_agent_event(
        owner: PeerFanoutOwner,
        targets: Vec<String>,
        event: String,
        payload: serde_json::Value,
        turns: super::state::PeerTurnTracker,
        generation: u64,
        terminal_turn: Option<PeerTurnKey>,
    ) -> Self {
        let terminal = terminal_turn.map(|turn| (turns.clone(), generation, turn));
        Self {
            owner,
            targets,
            event,
            payload,
            continuity: Some((turns, generation)),
            terminal,
        }
    }
}

fn continuity_is_current(continuity: &Option<(super::state::PeerTurnTracker, u64)>) -> bool {
    continuity
        .as_ref()
        .is_none_or(|(turns, generation)| turns.is_event_stream_generation_current(*generation))
}

static PEER_EVENT_FANOUT_TX: OnceLock<mpsc::Sender<QueuedPeerDeviceEvent>> = OnceLock::new();

fn peer_event_sender() -> &'static mpsc::Sender<QueuedPeerDeviceEvent> {
    PEER_EVENT_FANOUT_TX.get_or_init(|| {
        let (tx, mut rx) = mpsc::channel::<QueuedPeerDeviceEvent>(PEER_EVENT_DELIVERY_CAPACITY);
        tokio::spawn(async move {
            while let Some(queued) = rx.recv().await {
                fanout_peer_device_event_once(queued).await;
            }
        });
        tx
    })
}

/// Subscribe to the invocation-scoped event source and forward only Peer-owned turns.
pub(crate) fn start_peer_event_fanout(state: PeerHostState) {
    start_peer_permission_event_fanout(state.clone());
    let mut rx = state.agent_events.subscribe();
    state.turns.mark_event_stream_ready();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(envelope) => {
                    if let Err(error) = handle_agentic_event(&state, envelope.event).await {
                        tracing::warn!("CLI Peer event fanout lost continuity: {error}");
                        interrupt_and_fail_peer_turns(
                            &state,
                            false,
                            "Peer event fanout lost continuity",
                        )
                        .await;
                        if drain_broadcast_receiver(&mut rx) {
                            state.turns.interrupt_event_stream(true);
                            break;
                        }
                        state.turns.mark_event_stream_ready();
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("CLI Peer event fanout lagged by {skipped} events");
                    interrupt_and_fail_peer_turns(&state, false, "Peer event stream lagged").await;
                    if drain_broadcast_receiver(&mut rx) {
                        state.turns.interrupt_event_stream(true);
                        break;
                    }
                    state.turns.mark_event_stream_ready();
                }
                Err(broadcast::error::RecvError::Closed) => {
                    interrupt_and_fail_peer_turns(&state, true, "Peer event stream closed").await;
                    break;
                }
            }
        }
    });
}

fn start_peer_permission_event_fanout(state: PeerHostState) {
    let Ok(mut receiver) = state.agent_runtime.subscribe_permission_requests() else {
        tracing::warn!("CLI Peer permission event fanout is unavailable");
        return;
    };
    tokio::spawn(async move {
        let mut owned_request_ids = HashSet::new();
        loop {
            match receiver.recv().await {
                Ok(event) => match &event {
                    PermissionRequestEvent::Asked { request } => {
                        if !state.turns.owns_permission_request(request) {
                            continue;
                        }
                        owned_request_ids.insert(request.request_id.clone());
                        fanout_permission_event(event).await;
                    }
                    PermissionRequestEvent::Replied { request_id, .. }
                    | PermissionRequestEvent::Cancelled { request_id, .. } => {
                        if owned_request_ids.remove(request_id) {
                            fanout_permission_event(event).await;
                        }
                    }
                },
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("CLI Peer permission event fanout lagged by {skipped} events");
                    let pending = state
                        .agent_runtime
                        .pending_permission_requests()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|request| state.turns.owns_permission_request(request))
                        .collect::<Vec<_>>();
                    let pending_ids = pending
                        .iter()
                        .map(|request| request.request_id.clone())
                        .collect::<HashSet<_>>();
                    let stale_request_ids = owned_request_ids
                        .difference(&pending_ids)
                        .cloned()
                        .collect::<Vec<_>>();
                    for request_id in stale_request_ids {
                        fanout_permission_event(PermissionRequestEvent::Cancelled {
                            request_id,
                            reason: "Permission event stream resynchronized".to_string(),
                        })
                        .await;
                    }
                    owned_request_ids = pending_ids;
                    for request in pending {
                        fanout_permission_event(PermissionRequestEvent::Asked { request }).await;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    if let Err(error) = state
                        .cancel_and_drain_peer_turns("Peer permission event stream closed")
                        .await
                    {
                        tracing::warn!(
                            "Peer work was not fully cancelled after permission event closure: {error}"
                        );
                    }
                    break;
                }
            }
        }
    });
}

async fn fanout_permission_event(event: PermissionRequestEvent) {
    match serde_json::to_value(event) {
        Ok(payload) => fanout_peer_device_event("permission://event".to_string(), payload).await,
        Err(error) => tracing::warn!("CLI Peer permission event serialization failed: {error}"),
    }
}

async fn interrupt_and_fail_peer_turns(state: &PeerHostState, closed: bool, reason: &'static str) {
    let drain = state.turns.interrupt_event_stream(closed);
    let interrupted_turns = drain.turns.clone();
    if let Err(error) = state.cancel_peer_turns(drain, reason).await {
        tracing::warn!("Peer turn cancellation after event interruption was incomplete: {error}");
    }
    for turn in interrupted_turns {
        let (event, payload) = interrupted_turn_failure_projection(&turn, reason);
        fanout_peer_device_event(event, payload).await;
    }
}

/// Returns true when the sender side closed while the stale backlog was drained.
fn drain_broadcast_receiver(
    rx: &mut broadcast::Receiver<bitfun_events::AgenticEventEnvelope>,
) -> bool {
    loop {
        match rx.try_recv() {
            Ok(_) | Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(broadcast::error::TryRecvError::Empty) => return false,
            Err(broadcast::error::TryRecvError::Closed) => return true,
        }
    }
}

async fn handle_agentic_event(state: &PeerHostState, event: AgenticEvent) -> Result<(), String> {
    let event_turn = event_turn_key(&event);
    let terminal_turn = terminal_turn_key(&event);
    if terminal_turn
        .as_ref()
        .is_some_and(|turn| state.turns.is_interrupted_terminal(turn))
    {
        return Ok(());
    }
    if let AgenticEvent::DialogTurnStarted {
        user_message_metadata,
        ..
    } = &event
    {
        let background_lineage = user_message_metadata
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .filter(|metadata| {
                metadata.get("kind").and_then(serde_json::Value::as_str)
                    == Some("background_result")
                    && metadata
                        .get("sourceKind")
                        .and_then(serde_json::Value::as_str)
                        == Some("subagent")
            })
            .and_then(|metadata| {
                Some((
                    PeerTurnKey::new(
                        metadata.get("parentSessionId")?.as_str()?,
                        metadata.get("parentDialogTurnId")?.as_str()?,
                    ),
                    PeerTurnKey::new(
                        metadata.get("subagentSessionId")?.as_str()?,
                        metadata.get("subagentDialogTurnId")?.as_str()?,
                    ),
                ))
            });
        if let Some((parent, source_child)) = background_lineage {
            if let Some(turn) = event_turn.as_ref() {
                state
                    .turns
                    .register_background_follow_up(&parent, &source_child, turn.clone())?;
            }
        }
    }

    if let AgenticEvent::SubagentSessionLinked {
        session_id,
        subagent_dialog_turn_id,
        parent_session_id,
        parent_dialog_turn_id,
        parent_tool_call_id,
        ..
    } = &event
    {
        state.turns.register_linked_child(
            &PeerTurnKey::new(parent_session_id, parent_dialog_turn_id),
            PeerTurnKey::new(session_id, subagent_dialog_turn_id),
            parent_tool_call_id,
        )?;
    }

    if matches!(&event, AgenticEvent::DialogTurnStarted { .. }) {
        let Some(turn) = event_turn.as_ref() else {
            return Ok(());
        };
        if !state.turns.mark_started(turn) {
            return Ok(());
        }
    }

    let Some(session_id) = event.session_id() else {
        return Ok(());
    };
    if !state.turns.owns(
        session_id,
        event_turn.as_ref().map(|turn| turn.turn_id.as_str()),
    ) {
        return Ok(());
    }
    if let Some(turn) = terminal_turn.as_ref() {
        if !state.turns.claim_terminal_delivery(turn)? {
            return Ok(());
        }
    }

    if let AgenticEvent::UserSteeringInjected {
        session_id,
        turn_id,
        steering_id,
        ..
    } = &event
    {
        state
            .turns
            .finish_background_injection(&PeerTurnKey::new(session_id, turn_id), steering_id);
    }

    if let AgenticEvent::ToolEvent {
        session_id,
        turn_id,
        tool_event: ToolEventData::Started {
            identity, params, ..
        },
        ..
    } = &event
    {
        let (tool_name, params) = effective_tool_invocation(&identity.tool_name, params);
        debug_assert_eq!(identity.effective_name(), tool_name);
        if tool_name == "Task"
            && params
                .get("run_in_background")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            state.turns.record_background_task_call(
                &PeerTurnKey::new(session_id, turn_id),
                identity.tool_id.clone(),
            )?;
        } else if tool_name == "Task"
            && params.get("action").and_then(serde_json::Value::as_str) == Some("cancel")
        {
            if let Some(target_session_id) =
                params.get("session_id").and_then(serde_json::Value::as_str)
            {
                state.turns.record_background_task_cancellation(
                    &PeerTurnKey::new(session_id, turn_id),
                    identity.tool_id.clone(),
                    target_session_id.to_string(),
                )?;
            }
        }
    }

    if let AgenticEvent::ToolEvent {
        session_id,
        turn_id,
        tool_event,
        ..
    } = &event
    {
        let terminal_task_call = match tool_event {
            ToolEventData::Completed {
                identity, result, ..
            } if identity.effective_name() == "Task" => Some((
                identity.tool_id.as_str(),
                result
                    .get("background_task_id")
                    .and_then(serde_json::Value::as_str),
                result
                    .get("cancelled_background_tasks")
                    .and_then(serde_json::Value::as_u64),
            )),
            ToolEventData::Failed { identity, .. } | ToolEventData::Cancelled { identity, .. }
                if identity.effective_name() == "Task" =>
            {
                Some((identity.tool_id.as_str(), None, None))
            }
            _ => None,
        };
        if let Some((tool_id, background_task_id, cancelled_background_tasks)) = terminal_task_call
        {
            state.turns.finish_task_call(
                &PeerTurnKey::new(session_id, turn_id),
                tool_id,
                background_task_id,
                cancelled_background_tasks,
            );
        }
    }

    let Some(projected) = project_agentic_frontend_event(event) else {
        if let Some(turn) = terminal_turn {
            state.turns.finish_turn(&turn);
        }
        return Ok(());
    };
    let targets = attached_controllers();
    if targets.is_empty() {
        return Err("no attached Peer controller can receive Agent events".to_string());
    }
    let generation = state.turns.current_event_stream_generation()?;
    let owner = crate::account::capture_peer_fanout_owner()
        .await
        .map_err(|error| format!("Peer event routing owner unavailable: {error}"))?;
    enqueue_peer_device_event(
        peer_event_sender(),
        QueuedPeerDeviceEvent::for_agent_event(
            owner,
            targets,
            projected.event_name,
            projected.payload,
            state.turns.clone(),
            generation,
            terminal_turn,
        ),
    )
    .await
    .map_err(|_| "Peer event delivery queue is closed".to_string())?;
    Ok(())
}

fn event_turn_key(event: &AgenticEvent) -> Option<PeerTurnKey> {
    let (session_id, turn_id) = match event {
        AgenticEvent::DialogTurnStarted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::DialogTurnCompleted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::DialogTurnCancelled {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::DialogTurnFailed {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::TokenUsageUpdated {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ContextCompressionStarted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ContextCompressionCompleted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ContextCompressionFailed {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ModelRoundStarted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ModelRoundCompleted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::TextChunk {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ThinkingChunk {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::ToolEvent {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::DeepReviewQueueStateChanged {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::UserSteeringInjected {
            session_id,
            turn_id,
            ..
        } => (session_id, turn_id),
        AgenticEvent::SubagentSessionLinked {
            session_id,
            subagent_dialog_turn_id,
            ..
        } => (session_id, subagent_dialog_turn_id),
        _ => return None,
    };
    Some(PeerTurnKey::new(session_id, turn_id))
}

fn terminal_turn_key(event: &AgenticEvent) -> Option<PeerTurnKey> {
    match event {
        AgenticEvent::DialogTurnCompleted {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::DialogTurnCancelled {
            session_id,
            turn_id,
            ..
        }
        | AgenticEvent::DialogTurnFailed {
            session_id,
            turn_id,
            ..
        } => Some(PeerTurnKey::new(session_id, turn_id)),
        _ => None,
    }
}

/// Queue an explicit Peer command event with its current delivery targets.
pub(crate) async fn fanout_peer_device_event(event: String, payload: serde_json::Value) {
    let targets = attached_controllers();
    if targets.is_empty() {
        return;
    }
    let inherited_owner = crate::account::inherited_peer_fanout_owner();
    let inherits_routing_lease = inherited_owner.is_some();
    let owner = match inherited_owner {
        Some(owner) => owner,
        None => match crate::account::capture_peer_fanout_owner().await {
            Ok(owner) => owner,
            Err(error) => {
                tracing::debug!("Peer event fanout skipped before enqueue: {error}");
                return;
            }
        },
    };
    let queued = QueuedPeerDeviceEvent::new(owner, targets, event, payload);
    if inherits_routing_lease {
        // HostInvoke already holds the lifecycle read lease. Never await queue
        // capacity or acquire a nested read here: a queued transition writer
        // would otherwise create a writer-priority self-deadlock. A detached
        // task preserves backpressure and validates the captured owner later.
        enqueue_inherited_peer_device_event(peer_event_sender().clone(), queued);
        return;
    }
    if let Err(queued) = enqueue_peer_device_event(peer_event_sender(), queued).await {
        tracing::warn!(
            "Peer event delivery queue closed before accepting command event; using direct delivery"
        );
        fanout_peer_device_event_once(queued).await;
    }
}

fn enqueue_inherited_peer_device_event(
    sender: mpsc::Sender<QueuedPeerDeviceEvent>,
    queued: QueuedPeerDeviceEvent,
) {
    match sender.try_send(queued) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(queued)) => {
            tokio::spawn(async move {
                if let Err(queued) = enqueue_peer_device_event(&sender, queued).await {
                    tracing::warn!(
                        "Peer event delivery queue closed while draining inherited routing event"
                    );
                    fanout_peer_device_event_once(queued).await;
                }
            });
        }
        Err(mpsc::error::TrySendError::Closed(queued)) => {
            tokio::spawn(async move {
                tracing::warn!(
                    "Peer event delivery queue closed for inherited routing event; using direct delivery"
                );
                fanout_peer_device_event_once(queued).await;
            });
        }
    }
}

async fn enqueue_peer_device_event(
    sender: &mpsc::Sender<QueuedPeerDeviceEvent>,
    queued: QueuedPeerDeviceEvent,
) -> Result<(), QueuedPeerDeviceEvent> {
    sender.send(queued).await.map_err(|error| error.0)
}

async fn fanout_peer_device_event_once(queued: QueuedPeerDeviceEvent) {
    let QueuedPeerDeviceEvent {
        owner,
        targets,
        event,
        payload,
        continuity,
        terminal,
    } = queued;
    if !continuity_is_current(&continuity) {
        return;
    }
    let _terminal_delivery = TerminalDeliveryGuard::new(terminal);
    if targets.is_empty() {
        return;
    }

    let routing_lease = match crate::account::acquire_peer_fanout_lease(&owner).await {
        Ok(lease) => lease,
        Err(error) => {
            tracing::debug!("Queued Peer event dropped after owner change: {error}");
            return;
        }
    };
    let session = &routing_lease.session;
    let relay_client = &routing_lease.relay_client;

    let envelope = match serde_json::to_string(&RemoteCommand::DeviceEvent { event, payload }) {
        Ok(envelope) => envelope,
        Err(error) => {
            tracing::warn!("Peer event fanout serialization failed: {error}");
            return;
        }
    };
    let (encrypted_data, nonce) = match encrypt_to_base64(&session.master_key, &envelope) {
        Ok(encrypted) => encrypted,
        Err(error) => {
            tracing::warn!("Peer event fanout encryption failed: {error}");
            return;
        }
    };
    let targets = retained_delivery_targets(&targets, &attached_controllers());
    if targets.is_empty() {
        return;
    }

    for target in &targets {
        if !continuity_is_current(&continuity) {
            break;
        }
        let Some(_delivery_lease) = controller_delivery_lease(target).await else {
            continue;
        };
        let correlation_id = uuid::Uuid::new_v4().to_string();
        if let Err(error) = relay_client
            .send_device_message(&target, &correlation_id, &encrypted_data, &nonce)
            .await
        {
            tracing::debug!("Peer event fanout to {target} failed: {error}");
        }
    }
}

fn retained_delivery_targets(snapshot: &[String], currently_attached: &[String]) -> Vec<String> {
    let currently_attached = currently_attached.iter().collect::<HashSet<_>>();
    snapshot
        .iter()
        .filter(|target| currently_attached.contains(target))
        .cloned()
        .collect()
}

struct TerminalDeliveryGuard {
    terminal: Option<(super::state::PeerTurnTracker, u64, PeerTurnKey)>,
}

impl TerminalDeliveryGuard {
    fn new(terminal: Option<(super::state::PeerTurnTracker, u64, PeerTurnKey)>) -> Self {
        Self { terminal }
    }
}

impl Drop for TerminalDeliveryGuard {
    fn drop(&mut self) {
        complete_terminal_delivery(self.terminal.take());
    }
}

fn complete_terminal_delivery(terminal: Option<(super::state::PeerTurnTracker, u64, PeerTurnKey)>) {
    if let Some((turns, generation, turn)) = terminal {
        turns.complete_terminal_delivery(generation, &turn);
    }
}

fn interrupted_turn_failure_projection(
    turn: &PeerTurnKey,
    reason: &str,
) -> (String, serde_json::Value) {
    let projected = project_agentic_frontend_event(AgenticEvent::DialogTurnFailed {
        session_id: turn.session_id.clone(),
        turn_id: turn.turn_id.clone(),
        error: reason.to_string(),
        error_category: None,
        error_detail: None,
    })
    .expect("DialogTurnFailed must have a frontend projection");
    (projected.event_name, projected.payload)
}

#[cfg(test)]
mod tests {
    use bitfun_events::{AgenticEvent, AgenticEventEnvelope, AgenticEventPriority};

    use super::{
        continuity_is_current, drain_broadcast_receiver, enqueue_inherited_peer_device_event,
        enqueue_peer_device_event, event_turn_key, interrupted_turn_failure_projection,
        retained_delivery_targets, QueuedPeerDeviceEvent, TerminalDeliveryGuard,
    };
    use crate::peer_host::state::{PeerTurnKey, PeerTurnTracker};

    fn test_owner(generation: u64) -> crate::account::PeerFanoutOwner {
        crate::account::PeerFanoutOwner::for_test(generation, "test-token")
    }

    #[test]
    fn queued_events_keep_the_target_snapshot_from_enqueue_time() {
        let mut current_targets = vec!["controller-1".to_string()];
        let queued = QueuedPeerDeviceEvent::new(
            test_owner(7),
            current_targets.clone(),
            "dialog_turn_started".to_string(),
            serde_json::json!({}),
        );
        current_targets.push("controller-2".to_string());

        assert_eq!(queued.targets, vec!["controller-1"]);
        assert_eq!(current_targets, vec!["controller-1", "controller-2"]);
    }

    #[test]
    fn queued_events_exclude_controllers_that_detached_after_enqueue() {
        let queued_targets = vec!["controller-1".to_string(), "controller-2".to_string()];
        let currently_attached = vec!["controller-2".to_string(), "controller-3".to_string()];

        assert_eq!(
            retained_delivery_targets(&queued_targets, &currently_attached),
            vec!["controller-2"]
        );
        assert!(retained_delivery_targets(&queued_targets, &[]).is_empty());
    }

    #[test]
    fn queued_peer_turn_does_not_authorize_session_scoped_events() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let turn = PeerTurnKey::new("session-1", "peer-turn");
        tracker.register_root(turn.clone()).expect("register root");
        let event = AgenticEvent::ImageAnalysisStarted {
            session_id: turn.session_id.clone(),
            image_count: 1,
            user_input: "local input".to_string(),
            image_metadata: None,
        };

        assert!(event_turn_key(&event).is_none());
        assert!(!tracker.owns(&turn.session_id, None));

        assert!(tracker.mark_started(&turn));
        assert!(tracker.owns(&turn.session_id, None));
        tracker.finish_turn(&turn);
        assert!(!tracker.owns(&turn.session_id, None));
    }

    #[tokio::test]
    async fn closed_delivery_queue_returns_the_event_for_direct_fallback() {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        drop(rx);
        let queued = QueuedPeerDeviceEvent::new(
            test_owner(7),
            vec!["controller-1".to_string()],
            "agentic://dialog-turn-failed".to_string(),
            serde_json::json!({ "turnId": "turn-1" }),
        );

        let recovered = enqueue_peer_device_event(&tx, queued)
            .await
            .expect_err("closed queue must return the undelivered event");
        assert_eq!(recovered.event, "agentic://dialog-turn-failed");
    }

    #[tokio::test]
    async fn inherited_enqueue_does_not_wait_for_full_delivery_queue() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        tx.send(QueuedPeerDeviceEvent::new(
            test_owner(7),
            vec!["controller-1".to_string()],
            "first".to_string(),
            serde_json::json!({}),
        ))
        .await
        .expect("seed queue");

        enqueue_inherited_peer_device_event(
            tx,
            QueuedPeerDeviceEvent::new(
                test_owner(7),
                vec!["controller-1".to_string()],
                "second".to_string(),
                serde_json::json!({}),
            ),
        );

        assert_eq!(rx.recv().await.expect("first queued event").event, "first");
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("detached enqueue should complete after capacity is available")
            .expect("second queued event");
        assert_eq!(second.event, "second");
    }

    #[test]
    fn terminal_turn_stays_owned_until_delivery_completion() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let turn = PeerTurnKey::new("session-1", "turn-1");
        let generation = tracker.register_root(turn.clone()).expect("register root");

        assert!(tracker.owns("session-1", Some("turn-1")));
        {
            let _delivery = TerminalDeliveryGuard::new(Some((tracker.clone(), generation, turn)));
            assert!(tracker.owns("session-1", Some("turn-1")));
        }
        assert!(!tracker.owns("session-1", Some("turn-1")));
    }

    #[test]
    fn stale_terminal_delivery_cannot_release_an_interrupted_turn_key_after_reset() {
        let tracker = PeerTurnTracker::new();
        tracker.mark_event_stream_ready();
        let turn = PeerTurnKey::new("session-1", "turn-1");
        let stale_generation = tracker
            .register_root(turn.clone())
            .expect("register old root");
        let stale_delivery =
            TerminalDeliveryGuard::new(Some((tracker.clone(), stale_generation, turn.clone())));

        tracker.interrupt_event_stream(false);
        tracker.mark_event_stream_ready();
        assert!(tracker.register_root(turn.clone()).is_err());
        drop(stale_delivery);
        assert!(tracker.register_root(turn.clone()).is_err());
        assert!(!tracker.owns("session-1", Some("turn-1")));
    }

    #[test]
    fn interrupted_turn_projects_an_existing_failed_terminal_event() {
        let (event, payload) = interrupted_turn_failure_projection(
            &PeerTurnKey::new("session-1", "turn-1"),
            "Peer event stream lagged",
        );

        assert_eq!(event, "agentic://dialog-turn-failed");
        assert_eq!(payload["sessionId"], "session-1");
        assert_eq!(payload["turnId"], "turn-1");
        assert_eq!(payload["error"], "Peer event stream lagged");
    }

    #[test]
    fn queued_agent_events_are_invalidated_by_stream_interruption() {
        let turns = PeerTurnTracker::new();
        turns.mark_event_stream_ready();
        let generation = turns
            .current_event_stream_generation()
            .expect("ready generation");
        let queued = QueuedPeerDeviceEvent::for_agent_event(
            test_owner(7),
            vec!["controller-1".to_string()],
            "dialog_turn_started".to_string(),
            serde_json::json!({}),
            turns.clone(),
            generation,
            None,
        );

        assert!(continuity_is_current(&queued.continuity));
        assert_eq!(queued.owner.generation_for_test(), 7);
        turns.interrupt_event_stream(false);
        turns.mark_event_stream_ready();
        assert!(!continuity_is_current(&queued.continuity));
    }

    #[test]
    fn draining_distinguishes_an_empty_live_stream_from_a_closed_stream() {
        let (tx, mut live_rx) = tokio::sync::broadcast::channel(1);
        assert!(!drain_broadcast_receiver(&mut live_rx));

        let mut closed_rx = tx.subscribe();
        drop(tx);
        assert!(drain_broadcast_receiver(&mut closed_rx));
    }

    #[test]
    fn draining_stale_backlog_does_not_release_interrupted_turn_quarantine() {
        let turns = PeerTurnTracker::new();
        turns.mark_event_stream_ready();
        let turn = PeerTurnKey::new("session-1", "turn-1");
        turns.register_root(turn.clone()).expect("register root");
        turns.interrupt_event_stream(false);
        turns.mark_event_stream_ready();

        let (tx, mut rx) = tokio::sync::broadcast::channel(2);
        tx.send(AgenticEventEnvelope::new(
            AgenticEvent::DialogTurnCancelled {
                session_id: turn.session_id.clone(),
                turn_id: turn.turn_id.clone(),
            },
            AgenticEventPriority::Normal,
        ))
        .expect("queue stale terminal");

        assert!(!drain_broadcast_receiver(&mut rx));
        assert!(turns.register_root(turn).is_err());
    }
}
