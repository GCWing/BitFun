//! Scheduler owner decisions.

use crate::events::turn_outcome_kind;
use crate::thread_goal::{build_objective_updated_plan, build_thread_goal_continuation_plan};
use bitfun_runtime_ports::{
    should_skip_agent_session_reply, should_suppress_agent_session_cancelled_reply,
    AgentDialogPrependedReminder, AgentSessionReplyRoute, DialogQueuePriority,
    DialogRoundInjectionSource, DialogSessionStateFact, DialogSteerOutcome,
    DialogSubmissionPolicy, DialogTriggerSource, RoundInjection, RoundInjectionKind,
    RoundInjectionTarget, RoundInjectionToolPreemption, ThreadGoal,
    MAX_THREAD_GOAL_AUTO_CONTINUATIONS,
};
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_MAX_DIALOG_QUEUE_DEPTH: usize = 20;

#[derive(Debug, Clone)]
pub struct ActiveDialogTurn {
    turn_id: String,
    workspace_path: Option<String>,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
    agent_type: String,
    user_input: String,
    user_message_metadata: Option<serde_json::Value>,
    policy: DialogSubmissionPolicy,
    reply_route: Option<AgentSessionReplyRoute>,
}

impl ActiveDialogTurn {
    #[allow(clippy::too_many_arguments)] // state constructor; mirrors the struct fields
    pub fn new(
        turn_id: String,
        workspace_path: Option<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
        agent_type: String,
        user_input: String,
        user_message_metadata: Option<serde_json::Value>,
        policy: DialogSubmissionPolicy,
        reply_route: Option<AgentSessionReplyRoute>,
    ) -> Self {
        Self {
            turn_id,
            workspace_path,
            remote_connection_id,
            remote_ssh_host,
            agent_type,
            user_input,
            user_message_metadata,
            policy,
            reply_route,
        }
    }

    pub fn turn_id(&self) -> &str {
        &self.turn_id
    }

    pub fn workspace_path(&self) -> Option<&str> {
        self.workspace_path.as_deref()
    }

    pub fn workspace_path_owned(&self) -> Option<String> {
        self.workspace_path.clone()
    }

    pub fn remote_connection_id(&self) -> Option<&str> {
        self.remote_connection_id.as_deref()
    }

    pub fn remote_connection_id_owned(&self) -> Option<String> {
        self.remote_connection_id.clone()
    }

    pub fn remote_ssh_host(&self) -> Option<&str> {
        self.remote_ssh_host.as_deref()
    }

    pub fn remote_ssh_host_owned(&self) -> Option<String> {
        self.remote_ssh_host.clone()
    }

    pub fn agent_type(&self) -> &str {
        &self.agent_type
    }

    pub fn agent_type_owned(&self) -> String {
        self.agent_type.clone()
    }

    pub fn user_input(&self) -> &str {
        &self.user_input
    }

    pub fn user_message_metadata(&self) -> Option<&serde_json::Value> {
        self.user_message_metadata.as_ref()
    }

    pub fn reply_route(&self) -> Option<&AgentSessionReplyRoute> {
        self.reply_route.as_ref()
    }

    pub fn is_agent_session_request(&self) -> bool {
        self.policy.trigger_source == DialogTriggerSource::AgentSession
            && self.reply_route.is_some()
    }

    pub fn should_suppress_cancelled_reply_for_requester(
        &self,
        requester_session_id: &str,
    ) -> bool {
        should_suppress_agent_session_cancelled_reply(
            &self.policy,
            self.reply_route
                .as_ref()
                .map(|reply_route| reply_route.source_session_id.as_str()),
            requester_session_id,
        )
    }
}

#[derive(Debug, Default)]
pub struct ActiveDialogTurnStore {
    inner: dashmap::DashMap<String, ActiveDialogTurn>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // matched turn is inherently larger than control outcomes
pub enum ActiveDialogTurnTakeResult {
    Matched(ActiveDialogTurn),
    Absent,
    DifferentTurn,
}

impl ActiveDialogTurnStore {
    pub fn insert(&self, session_id: &str, turn: ActiveDialogTurn) {
        self.inner.insert(session_id.to_string(), turn);
    }

    pub fn remove(&self, session_id: &str) -> Option<ActiveDialogTurn> {
        self.inner.remove(session_id).map(|(_, turn)| turn)
    }

    /// Atomically take the active metadata only when it belongs to the
    /// outcome's turn generation.
    pub fn take_for_outcome(&self, session_id: &str, turn_id: &str) -> ActiveDialogTurnTakeResult {
        match self.inner.entry(session_id.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(entry) if entry.get().turn_id() == turn_id => {
                ActiveDialogTurnTakeResult::Matched(entry.remove())
            }
            dashmap::mapref::entry::Entry::Occupied(_) => ActiveDialogTurnTakeResult::DifferentTurn,
            dashmap::mapref::entry::Entry::Vacant(_) => ActiveDialogTurnTakeResult::Absent,
        }
    }

    pub fn contains(&self, session_id: &str) -> bool {
        self.inner.contains_key(session_id)
    }

    pub fn matches_turn(&self, session_id: &str, turn_id: &str) -> bool {
        self.inner
            .get(session_id)
            .is_some_and(|turn| turn.turn_id() == turn_id)
    }

    /// User input of the currently active turn for `session_id`, if any.
    pub fn active_turn_user_input(&self, session_id: &str) -> Option<String> {
        self.inner
            .get(session_id)
            .map(|turn| turn.user_input().to_string())
    }

    pub fn suppression_key_for_requester(
        &self,
        target_session_id: &str,
        requester_session_id: &str,
    ) -> Option<(String, String)> {
        self.inner.get(target_session_id).and_then(|active_turn| {
            active_turn
                .should_suppress_cancelled_reply_for_requester(requester_session_id)
                .then(|| {
                    (
                        target_session_id.to_string(),
                        active_turn.turn_id().to_string(),
                    )
                })
        })
    }
}

#[derive(Debug, Default)]
pub struct DialogReplySuppressionSet {
    inner: dashmap::DashMap<(String, String), ()>,
}

impl DialogReplySuppressionSet {
    pub fn mark(&self, session_id: &str, turn_id: &str) {
        self.inner
            .insert((session_id.to_string(), turn_id.to_string()), ());
    }

    pub fn clear(&self, session_id: &str, turn_id: &str) {
        self.inner
            .remove(&(session_id.to_string(), turn_id.to_string()));
    }

    pub fn take(&self, session_id: &str, turn_id: &str) -> bool {
        self.inner
            .remove(&(session_id.to_string(), turn_id.to_string()))
            .is_some()
    }

    /// Remove every entry belonging to `session_id`, regardless of turn id.
    ///
    /// Session-end cleanup: a recycled session id must not inherit suppression
    /// marks or retired-outcome tombstones from the previous session.
    pub fn clear_session(&self, session_id: &str) {
        self.inner
            .retain(|(entry_session_id, _), _| entry_session_id != session_id);
    }
}

#[derive(Debug, Default)]
pub struct SessionAbortFlags {
    inner: dashmap::DashMap<String, ()>,
}

impl SessionAbortFlags {
    pub fn mark(&self, session_id: &str) {
        self.inner.insert(session_id.to_string(), ());
    }

    pub fn clear(&self, session_id: &str) {
        self.inner.remove(session_id);
    }

    pub fn contains(&self, session_id: &str) -> bool {
        self.inner.contains_key(session_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogTurnQueueError {
    Full {
        session_id: String,
        max_depth: usize,
    },
}

impl fmt::Display for DialogTurnQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full {
                session_id,
                max_depth,
            } => write!(
                f,
                "Message queue full for session {session_id} (max {max_depth} messages)"
            ),
        }
    }
}

impl std::error::Error for DialogTurnQueueError {}

#[derive(Debug, Clone)]
struct QueuedDialogTurn<T> {
    priority: DialogQueuePriority,
    turn: T,
}

/// Per-session dialog-turn queue with product scheduler priority semantics.
#[derive(Debug)]
pub struct DialogTurnQueue<T> {
    max_depth: usize,
    inner: dashmap::DashMap<String, VecDeque<QueuedDialogTurn<T>>>,
}

impl<T> Default for DialogTurnQueue<T> {
    fn default() -> Self {
        Self::with_max_depth(DEFAULT_MAX_DIALOG_QUEUE_DEPTH)
    }
}

impl<T> DialogTurnQueue<T> {
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
            inner: dashmap::DashMap::new(),
        }
    }

    pub const fn max_depth(&self) -> usize {
        self.max_depth
    }

    pub fn depth(&self, session_id: &str) -> usize {
        self.inner.get(session_id).map(|q| q.len()).unwrap_or(0)
    }

    pub fn has_items(&self, session_id: &str) -> bool {
        self.depth(session_id) > 0
    }

    pub fn enqueue(
        &self,
        session_id: &str,
        turn: T,
        priority: DialogQueuePriority,
    ) -> Result<usize, DialogTurnQueueError> {
        let mut queue = self.inner.entry(session_id.to_string()).or_default();
        if queue.len() >= self.max_depth {
            return Err(DialogTurnQueueError::Full {
                session_id: session_id.to_string(),
                max_depth: self.max_depth,
            });
        }

        let queued = QueuedDialogTurn { priority, turn };
        let insert_at = queue
            .iter()
            .position(|existing| existing.priority < queued.priority);
        if let Some(index) = insert_at {
            queue.insert(index, queued);
        } else {
            queue.push_back(queued);
        }

        Ok(queue.len())
    }

    pub fn clear(&self, session_id: &str) -> Vec<T> {
        self.inner
            .remove(session_id)
            .map(|(_, queue)| queue.into_iter().map(|item| item.turn).collect())
            .unwrap_or_default()
    }

    pub fn dequeue_next(&self, session_id: &str) -> Option<T> {
        let turn = self
            .inner
            .get_mut(session_id)
            .and_then(|mut queue| queue.pop_front().map(|item| item.turn));
        self.inner
            .remove_if(session_id, |_, queue| queue.is_empty());
        turn
    }

    pub fn remove_first_matching<F>(&self, session_id: &str, mut predicate: F) -> Option<T>
    where
        F: FnMut(&T) -> bool,
    {
        let turn = self.inner.get_mut(session_id).and_then(|mut q| {
            q.iter()
                .position(|item| predicate(&item.turn))
                .and_then(|index| q.remove(index).map(|item| item.turn))
        });
        self.inner
            .remove_if(session_id, |_, queue| queue.is_empty());
        turn
    }

    /// Whether any queued turn for `session_id` satisfies `predicate`.
    ///
    /// Used to coalesce identical agent-driven follow-up turns: when the same
    /// background-result notification is already queued, a duplicate submit is
    /// skipped instead of spawning a second model request.
    pub fn any_matching<F>(&self, session_id: &str, mut predicate: F) -> bool
    where
        F: FnMut(&T) -> bool,
    {
        self.inner
            .get(session_id)
            .is_some_and(|queue| queue.iter().any(|item| predicate(&item.turn)))
    }

    pub fn requeue_front(&self, session_id: &str, turn: T, priority: DialogQueuePriority) {
        self.inner
            .entry(session_id.to_string())
            .or_default()
            .push_front(QueuedDialogTurn { priority, turn });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionReplyPlan {
    pub target_session_id: String,
    pub target_workspace_path: String,
    pub target_remote_connection_id: Option<String>,
    pub target_remote_ssh_host: Option<String>,
    pub user_input: String,
    pub reminder_text: String,
    pub user_message_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSessionReplyAction {
    NoReply,
    SkipSuppressedCancelledReply,
    Forward(AgentSessionReplyPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogSteeringAction {
    Reject {
        error: String,
    },
    Buffer {
        injection: RoundInjection,
        outcome: DialogSteerOutcome,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundDeliveryFacts {
    pub session_state: DialogSessionStateFact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundDeliveryAction {
    InjectIntoRunningTurn,
    SubmitAgentSessionFollowUp { queue_priority: DialogQueuePriority },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundInjectionKind {
    ThreadGoalObjectiveUpdated,
    BackgroundResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadGoalDeliveryReminderKind {
    GoalContinuation,
    GoalObjectiveUpdated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadGoalDeliveryReminder {
    pub kind: ThreadGoalDeliveryReminderKind,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadGoalDeliveryPlan {
    pub injection_prompt: String,
    pub injection_display: String,
    pub display_message: String,
    pub follow_up_user_input: String,
    pub follow_up_original_user_input: Option<String>,
    pub user_message_metadata: serde_json::Value,
    pub prepended_reminders: Vec<ThreadGoalDeliveryReminder>,
}

impl BackgroundDeliveryAction {
    pub const fn follow_up_submission_policy(self) -> Option<DialogSubmissionPolicy> {
        match self {
            Self::InjectIntoRunningTurn => None,
            Self::SubmitAgentSessionFollowUp { queue_priority } => Some(
                DialogSubmissionPolicy::new(DialogTriggerSource::AgentSession, queue_priority),
            ),
        }
    }
}

pub fn build_thread_goal_resumed_delivery_plan(goal: &ThreadGoal) -> ThreadGoalDeliveryPlan {
    let plan = build_thread_goal_continuation_plan(goal, MAX_THREAD_GOAL_AUTO_CONTINUATIONS);
    let injection_prompt = plan
        .prepended_reminders
        .first()
        .cloned()
        .unwrap_or_default();
    let display_message = plan.display_message;
    ThreadGoalDeliveryPlan {
        injection_prompt,
        injection_display: display_message.clone(),
        display_message: display_message.clone(),
        follow_up_user_input: "Resume working toward the active thread goal.".to_string(),
        follow_up_original_user_input: Some(display_message),
        user_message_metadata: plan.user_message_metadata,
        prepended_reminders: plan
            .prepended_reminders
            .into_iter()
            .map(|content| ThreadGoalDeliveryReminder {
                kind: ThreadGoalDeliveryReminderKind::GoalContinuation,
                content,
            })
            .collect(),
    }
}

pub fn build_thread_goal_objective_updated_delivery_plan(
    goal: &ThreadGoal,
) -> ThreadGoalDeliveryPlan {
    let plan = build_objective_updated_plan(goal);
    let injection_prompt = plan
        .prepended_reminders
        .first()
        .cloned()
        .unwrap_or_default();
    let display_message = plan.display_message;
    ThreadGoalDeliveryPlan {
        injection_prompt,
        injection_display: display_message.clone(),
        display_message: display_message.clone(),
        follow_up_user_input: "Adjust work to match the updated thread goal.".to_string(),
        follow_up_original_user_input: Some(display_message),
        user_message_metadata: plan.user_message_metadata,
        prepended_reminders: plan
            .prepended_reminders
            .into_iter()
            .map(|content| ThreadGoalDeliveryReminder {
                kind: ThreadGoalDeliveryReminderKind::GoalObjectiveUpdated,
                content,
            })
            .collect(),
    }
}

/// Used when no scheduler is wired (e.g. tests, isolated execution).
pub struct NoopDialogRoundInjectionSource;

impl DialogRoundInjectionSource for NoopDialogRoundInjectionSource {
    fn has_pending(&self, _session_id: &str, _turn_id: &str) -> bool {
        false
    }

    fn pending_tool_preemption(
        &self,
        _session_id: &str,
        _turn_id: &str,
    ) -> RoundInjectionToolPreemption {
        RoundInjectionToolPreemption::None
    }

    fn take_pending(&self, _session_id: &str, _turn_id: &str) -> Vec<RoundInjection> {
        Vec::new()
    }
}

#[derive(Clone)]
pub struct DialogRoundInjectionInterrupt {
    session_id: String,
    turn_id: String,
    source: Arc<dyn DialogRoundInjectionSource>,
}

impl std::fmt::Debug for DialogRoundInjectionInterrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialogRoundInjectionInterrupt")
            .field("session_id", &self.session_id)
            .field("turn_id", &self.turn_id)
            .finish_non_exhaustive()
    }
}

impl DialogRoundInjectionInterrupt {
    pub fn new(
        session_id: String,
        turn_id: String,
        source: Arc<dyn DialogRoundInjectionSource>,
    ) -> Self {
        Self {
            session_id,
            turn_id,
            source,
        }
    }

    pub fn pending_tool_preemption(&self) -> RoundInjectionToolPreemption {
        self.source
            .pending_tool_preemption(&self.session_id, &self.turn_id)
    }

    pub fn should_interrupt_after_current_atomic_unit(&self) -> bool {
        self.pending_tool_preemption()
            .should_interrupt_after_current_atomic_unit()
    }

    pub fn should_cancel_running_tools(&self) -> bool {
        self.pending_tool_preemption().should_cancel_running_tools()
    }

    pub fn should_interrupt(&self) -> bool {
        self.should_interrupt_after_current_atomic_unit()
    }
}

/// Per-session FIFO buffer of round injections keyed by `session_id`.
#[derive(Debug, Default)]
pub struct SessionRoundInjectionBuffer {
    inner: dashmap::DashMap<String, Vec<RoundInjection>>,
    /// Consumed UserSteering content keys `(session_id, content)` so a user
    /// message that was already injected into this session is never injected
    /// again — the observable driver of the 2-7x UserSteering duplicates.
    /// Cleared when the session is cleared/recycled (`clear`).
    consumed_steering: dashmap::DashSet<(String, String)>,
    /// Injection-id → content map for steering entries drained but not yet
    /// acknowledged; `acknowledge_injection` looks the content up here and
    /// records it into `consumed_steering`.
    pending_steering_content: dashmap::DashMap<(String, String), String>,
}

/// Time window within which same-kind background-result notifications for the
/// same session are coalesced into a single model request (5s). A notification
/// that arrives after the window is a genuinely new event and is delivered.
pub const NOTIFICATION_DEDUP_WINDOW: Duration = Duration::from_secs(5);

impl SessionRoundInjectionBuffer {
    /// Push a round injection, deduplicating against pending entries for the
    /// same session so a notification storm cannot turn N identical events into
    /// N model requests.
    ///
    /// Dedup keys (窗口语义：同会话 5 秒窗口内）:
    /// - `BackgroundResult` / `ThreadGoalObjectiveUpdated`: the notification
    ///   text is a fixed template (the display text never enters the prompt),
    ///   so all pending entries of the same kind created within the 5s window
    ///   are semantically identical — keep only the first and drop the rest.
    ///   Entries older than the window are kept: a genuinely later notification
    ///   must still reach the model (后台通知 = 必要功能，只去风暴不去通知).
    /// - `UserSteering`: the user message text is the prompt payload; two
    ///   pending entries with the same content within the window are the same
    ///   message re-steered, so keep only the first. Distinct messages always
    ///   both survive, regardless of timing.
    ///
    /// The dedup happens at push time, before the engine drains the buffer at a
    /// round boundary. It never mutates the injected text, the injection
    /// position, or the per-kind template, so the provider-side prompt prefix
    /// for the *kept* injection is byte-identical to the pre-fix behavior.
    pub fn push(&self, session_id: &str, message: RoundInjection) {
        // UserSteering 消费确认：同内容已被本会话注入过（acked）→ 不重复注入。
        // 注入结构（模板/位置/顺序）零改动，仅抑制已消费内容的重复投递。
        if message.kind == RoundInjectionKind::UserSteering
            && self.steering_already_consumed(session_id, &message.content)
        {
            log::debug!(
                "UserSteering already consumed; suppressing re-injection: session_id={}, content_len={}",
                session_id,
                message.content.len()
            );
            return;
        }
        let mut entry = self.inner.entry(session_id.to_string()).or_default();
        let duplicate = entry.iter().any(|existing| match (&existing.kind, &message.kind) {
            (RoundInjectionKind::BackgroundResult, RoundInjectionKind::BackgroundResult)
            | (
                RoundInjectionKind::ThreadGoalObjectiveUpdated,
                RoundInjectionKind::ThreadGoalObjectiveUpdated,
            ) => Self::within_dedup_window(existing, &message),
            (RoundInjectionKind::UserSteering, RoundInjectionKind::UserSteering) => {
                existing.content == message.content
                    && existing.prepended_reminders == message.prepended_reminders
            }
            _ => false,
        });
        if duplicate {
            log::debug!(
                "Round injection deduplicated: session_id={}, kind={:?}, pending={}",
                session_id,
                message.kind,
                entry.len()
            );
            return;
        }
        entry.push(message);
    }

    /// Record that a UserSteering content was actually injected for the
    /// session, so later duplicate pushes are suppressed. Keys are cleared
    /// when the session is cleared/recycled (`clear`).
    pub fn mark_steering_consumed(&self, session_id: &str, content: &str) {
        self.consumed_steering
            .insert((session_id.to_string(), content.to_string()));
    }

    /// Whether the (session, content) key is currently marked consumed.
    fn steering_already_consumed(&self, session_id: &str, content: &str) -> bool {
        self.consumed_steering
            .contains(&(session_id.to_string(), content.to_string()))
    }

    /// Whether `existing` and `candidate` fall inside the same notification
    /// dedup window (5s). Time is monotonic-ish for this purpose: created_at
    /// values are SystemTime; the window test is `|a - b| <= 5s`. A backwards
    /// clock (Err) is treated as within the window — both entries are still
    /// pending, so coalescing them is safe.
    fn within_dedup_window(existing: &RoundInjection, candidate: &RoundInjection) -> bool {
        // 对称窗口：|existing.created_at - candidate.created_at| <= 5s。
        // 方向无关——无论哪条更早，只要落在同一 5s 窗口内即视为同一风暴。
        match existing.created_at.duration_since(candidate.created_at) {
            Ok(diff) => diff <= NOTIFICATION_DEDUP_WINDOW,
            Err(system_time_error) => system_time_error.duration() <= NOTIFICATION_DEDUP_WINDOW,
        }
    }

    /// Drain all messages eligible for the currently running turn. Exact-turn
    /// injections that target a different turn are retained until the targeted
    /// turn consumes them or the session is cleared.
    pub fn drain_for_turn(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection> {
        let Some(mut entry) = self.inner.get_mut(session_id) else {
            return Vec::new();
        };
        let mut taken = Vec::new();
        let mut keep = Vec::new();
        for msg in entry.drain(..) {
            match &msg.target {
                RoundInjectionTarget::ExactTurn(target_turn_id) if target_turn_id == turn_id => {
                    if msg.kind == RoundInjectionKind::UserSteering {
                        self.pending_steering_content.insert(
                            (session_id.to_string(), msg.id.clone()),
                            msg.content.clone(),
                        );
                    }
                    taken.push(msg);
                }
                RoundInjectionTarget::CurrentRunningTurn => {
                    if msg.kind == RoundInjectionKind::UserSteering {
                        self.pending_steering_content.insert(
                            (session_id.to_string(), msg.id.clone()),
                            msg.content.clone(),
                        );
                    }
                    taken.push(msg);
                }
                RoundInjectionTarget::ExactTurn(_) => keep.push(msg),
            }
        }
        *entry = keep;
        taken
    }

    /// Look up the drained steering content for `injection_id` and record it as
    /// consumed for the session, so a duplicate push is suppressed.
    pub fn acknowledge_injection(&self, session_id: &str, injection_id: &str) {
        if let Some((_, content)) = self
            .pending_steering_content
            .remove(&(session_id.to_string(), injection_id.to_string()))
        {
            self.mark_steering_consumed(session_id, &content);
        }
    }

    /// Drain UserSteering entries still pending for `turn_id` that were never
    /// consumed (the turn ended before a round boundary drained them). These
    /// are returned so the scheduler can re-deliver them as a normal follow-up
    /// turn instead of silently dropping a real user message.
    pub fn drain_undelivered_steering(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection> {
        let Some(mut entry) = self.inner.get_mut(session_id) else {
            return Vec::new();
        };
        let mut taken = Vec::new();
        let mut keep = Vec::new();
        for msg in entry.drain(..) {
            let matches = match &msg.target {
                RoundInjectionTarget::ExactTurn(target_turn_id) => target_turn_id == turn_id,
                RoundInjectionTarget::CurrentRunningTurn => true,
            };
            if matches && msg.kind == RoundInjectionKind::UserSteering {
                taken.push(msg);
            } else {
                keep.push(msg);
            }
        }
        *entry = keep;
        taken
    }

    pub fn remove_by_id(&self, session_id: &str, injection_id: &str) -> Option<RoundInjection> {
        let mut entry = self.inner.get_mut(session_id)?;
        let index = entry
            .iter()
            .position(|message| message.id == injection_id)?;
        Some(entry.remove(index))
    }

    pub fn has_pending_for_turn(&self, session_id: &str, turn_id: &str) -> bool {
        self.inner
            .get(session_id)
            .map(|entry| {
                entry.iter().any(|msg| match &msg.target {
                    RoundInjectionTarget::ExactTurn(target_turn_id) => target_turn_id == turn_id,
                    RoundInjectionTarget::CurrentRunningTurn => true,
                })
            })
            .unwrap_or(false)
    }

    pub fn pending_tool_preemption_for_turn(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> RoundInjectionToolPreemption {
        self.inner
            .get(session_id)
            .map(|entry| {
                entry
                    .iter()
                    .filter(|msg| match &msg.target {
                        RoundInjectionTarget::ExactTurn(target_turn_id) => {
                            target_turn_id == turn_id
                        }
                        RoundInjectionTarget::CurrentRunningTurn => true,
                    })
                    .map(|msg| msg.execution_policy.tool_preemption)
                    .max()
                    .unwrap_or(RoundInjectionToolPreemption::None)
            })
            .unwrap_or(RoundInjectionToolPreemption::None)
    }

    /// Drop all messages for a session (e.g. session deleted or unrecoverable error).
    pub fn clear(&self, session_id: &str) {
        self.inner.remove(session_id);
        self.consumed_steering
            .retain(|(entry_session_id, _)| entry_session_id != session_id);
        self.pending_steering_content
            .retain(|(entry_session_id, _), _| entry_session_id != session_id);
    }

    pub fn pending_count(&self, session_id: &str) -> usize {
        self.inner.get(session_id).map(|v| v.len()).unwrap_or(0)
    }
}

impl DialogRoundInjectionSource for SessionRoundInjectionBuffer {
    fn has_pending(&self, session_id: &str, turn_id: &str) -> bool {
        self.has_pending_for_turn(session_id, turn_id)
    }

    fn pending_tool_preemption(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> RoundInjectionToolPreemption {
        self.pending_tool_preemption_for_turn(session_id, turn_id)
    }

    fn take_pending(&self, session_id: &str, turn_id: &str) -> Vec<RoundInjection> {
        self.drain_for_turn(session_id, turn_id)
    }

    fn acknowledge_consumed(
        &self,
        session_id: &str,
        _turn_id: &str,
        injection_id: &str,
        kind: RoundInjectionKind,
    ) {
        // UserSteering 消费确认：引擎注入完成（持久化进历史）后，把内容标记为
        // 已消费——同一用户消息再次经 steering 通道推入时被 push 去重抑制，
        // 杜绝 2-7 次重复注入。消费确认的记录与注入点分离：模板/结构/位置
        // 零改动，仅记录"这个内容已注入过"。标记在 buffer 内部（push 侧查）。
        if kind == RoundInjectionKind::UserSteering {
            // The engine only acknowledges with an injection id; the content
            // key is derived from the pending entries drained for this turn.
            // We keep the steering content keyed by id -> content mapping on
            // the buffer so the same message cannot re-enter through a new
            // buffer entry (see `acknowledge_injection`).
            self.acknowledge_injection(session_id, injection_id);
        }
    }
}

pub const fn resolve_background_delivery_action(
    facts: BackgroundDeliveryFacts,
) -> BackgroundDeliveryAction {
    match facts.session_state {
        DialogSessionStateFact::Processing => BackgroundDeliveryAction::InjectIntoRunningTurn,
        DialogSessionStateFact::Missing
        | DialogSessionStateFact::Idle
        | DialogSessionStateFact::Error => {
            let policy = DialogSubmissionPolicy::for_source(DialogTriggerSource::AgentSession);
            BackgroundDeliveryAction::SubmitAgentSessionFollowUp {
                queue_priority: policy.queue_priority,
            }
        }
    }
}

pub fn resolve_background_delivery_injection(
    kind: BackgroundInjectionKind,
    injection_id: String,
    content: String,
    display_content: Option<String>,
    created_at: SystemTime,
) -> RoundInjection {
    let display_content = display_content.unwrap_or_else(|| content.clone());
    let kind = match kind {
        BackgroundInjectionKind::ThreadGoalObjectiveUpdated => {
            RoundInjectionKind::ThreadGoalObjectiveUpdated
        }
        BackgroundInjectionKind::BackgroundResult => RoundInjectionKind::BackgroundResult,
    };
    RoundInjection {
        id: injection_id,
        kind,
        execution_policy: kind.default_execution_policy(),
        target: RoundInjectionTarget::CurrentRunningTurn,
        content,
        display_content,
        created_at,
        prepended_reminders: Vec::new(),
    }
}

pub fn resolve_background_delivery_injection_for_turn(
    kind: BackgroundInjectionKind,
    injection_id: String,
    content: String,
    display_content: Option<String>,
    created_at: SystemTime,
    turn_id: String,
) -> RoundInjection {
    let mut injection = resolve_background_delivery_injection(
        kind,
        injection_id,
        content,
        display_content,
        created_at,
    );
    injection.target = RoundInjectionTarget::ExactTurn(turn_id);
    injection
}

pub fn is_background_result_injection(kind: RoundInjectionKind) -> bool {
    kind == RoundInjectionKind::BackgroundResult
}

/// Outcome of a completed dialog turn, used to notify the concrete scheduler.
#[derive(Debug, Clone)]
pub enum TurnOutcome {
    /// Turn completed normally.
    Completed {
        turn_id: String,
        final_response: String,
    },
    /// Turn was cancelled by user.
    Cancelled { turn_id: String },
    /// Turn failed with an error.
    Failed { turn_id: String, error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcomeQueueAction {
    DispatchNext,
    ClearQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcomeStatus {
    Completed,
    Cancelled,
    Failed,
}

impl TurnOutcomeStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for TurnOutcomeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TurnOutcome {
    pub fn turn_id(&self) -> &str {
        match self {
            Self::Completed { turn_id, .. }
            | Self::Cancelled { turn_id }
            | Self::Failed { turn_id, .. } => turn_id,
        }
    }

    pub fn status(&self) -> TurnOutcomeStatus {
        match self {
            Self::Completed { .. } => TurnOutcomeStatus::Completed,
            Self::Cancelled { .. } => TurnOutcomeStatus::Cancelled,
            Self::Failed { .. } => TurnOutcomeStatus::Failed,
        }
    }

    pub fn status_str(&self) -> &'static str {
        self.status().as_str()
    }

    pub fn reply_text(&self) -> String {
        match self {
            Self::Completed { final_response, .. } => {
                if final_response.trim().is_empty() {
                    "(no final text response)".to_string()
                } else {
                    final_response.clone()
                }
            }
            Self::Cancelled { .. } => {
                "The target session cancelled this request before producing a final answer."
                    .to_string()
            }
            Self::Failed { error, .. } => {
                format!("The target session failed to complete this request.\nError: {error}")
            }
        }
    }

    pub fn queue_action(&self) -> TurnOutcomeQueueAction {
        match self {
            Self::Completed { .. } | Self::Cancelled { .. } => TurnOutcomeQueueAction::DispatchNext,
            Self::Failed { .. } => TurnOutcomeQueueAction::ClearQueue,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalContinuationAfterTurnAction {
    SkipNoActiveTurn,
    AbortForCancelled,
    Evaluate { turn_completed: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnOutcomeLifecyclePlan {
    pub status: TurnOutcomeStatus,
    pub queue_action: TurnOutcomeQueueAction,
    pub drain_finished_turn_injections: bool,
    pub goal_continuation: GoalContinuationAfterTurnAction,
}

impl TurnOutcomeLifecyclePlan {
    pub const fn dispatch_next(self) -> bool {
        matches!(self.queue_action, TurnOutcomeQueueAction::DispatchNext)
    }

    pub const fn clear_queue(self) -> bool {
        matches!(self.queue_action, TurnOutcomeQueueAction::ClearQueue)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogStartRouteFacts {
    pub has_image_contexts: bool,
    pub has_prepended_messages: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogStartRoute {
    Plain,
    WithPrependedMessages,
    WithImageContexts,
    WithImageContextsAndPrependedMessages,
}

pub const fn resolve_dialog_start_route(facts: DialogStartRouteFacts) -> DialogStartRoute {
    match (facts.has_image_contexts, facts.has_prepended_messages) {
        (false, false) => DialogStartRoute::Plain,
        (false, true) => DialogStartRoute::WithPrependedMessages,
        (true, false) => DialogStartRoute::WithImageContexts,
        (true, true) => DialogStartRoute::WithImageContextsAndPrependedMessages,
    }
}

pub fn resolve_turn_outcome_lifecycle_plan(
    outcome: &TurnOutcome,
    has_active_turn: bool,
) -> TurnOutcomeLifecyclePlan {
    let status = outcome.status();
    let goal_continuation = if !has_active_turn {
        GoalContinuationAfterTurnAction::SkipNoActiveTurn
    } else {
        match status {
            TurnOutcomeStatus::Cancelled => GoalContinuationAfterTurnAction::AbortForCancelled,
            TurnOutcomeStatus::Completed => GoalContinuationAfterTurnAction::Evaluate {
                turn_completed: true,
            },
            TurnOutcomeStatus::Failed => GoalContinuationAfterTurnAction::Evaluate {
                turn_completed: false,
            },
        }
    };

    TurnOutcomeLifecyclePlan {
        status,
        queue_action: outcome.queue_action(),
        drain_finished_turn_injections: true,
        goal_continuation,
    }
}

/// Current UTC time formatted as ISO-8601 with second precision and a `Z`
/// suffix (e.g. `2026-08-05T03:14:15Z`), matching the GetTime tool's `utc_time`
/// shape (see `get_time_tool.rs` `to_rfc3339_opts(SecondsFormat::Secs, true)`).
///
/// std-only implementation: `bitfun-agent-runtime` deliberately has no
/// `chrono` dependency, so the civil-date conversion uses Howard Hinnant's
/// public-domain `civil_from_days` algorithm (from the C++ `<chrono>`
/// compatibility paper), translated to Rust (not a Cargo dependency).
pub fn utc_iso8601_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_seconds = now.as_secs() as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Days since 1970-01-01 to a civil (year, month, day) date.
///
/// Howard Hinnant's `civil_from_days` (public domain, C++ `<chrono>` paper),
/// Rust translation, not a Cargo dependency.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day)
}

pub fn resolve_agent_session_reply_action(
    responder_session_id: &str,
    responder_role: Option<&str>,
    responder_depth: Option<u32>,
    active_turn: &ActiveDialogTurn,
    outcome: &TurnOutcome,
    suppressed_cancelled_reply: bool,
) -> AgentSessionReplyAction {
    if !active_turn.is_agent_session_request() {
        return AgentSessionReplyAction::NoReply;
    }

    if should_skip_agent_session_reply(turn_outcome_kind(outcome), suppressed_cancelled_reply) {
        return AgentSessionReplyAction::SkipSuppressedCancelledReply;
    }

    let Some(reply_route) = active_turn.reply_route() else {
        return AgentSessionReplyAction::NoReply;
    };

    let responder_workspace = active_turn
        .workspace_path()
        .unwrap_or("<unknown workspace>");
    let status = outcome.status();
    let server_time = utc_iso8601_now();
    let mut reminder_lines = vec![
        "This message is an automated reply to a previous SessionMessage call, not a human user message."
            .to_string(),
        format!("From session: {responder_session_id}"),
        format!("From workspace: {responder_workspace}"),
        format!("Status: {status}"),
        format!("Server time: {server_time}"),
    ];
    if let Some(role) = responder_role {
        reminder_lines.push(format!("From role: {role}"));
    }
    if let Some(depth) = responder_depth {
        reminder_lines.push(format!("From depth: {depth}"));
    }
    // Rewrite the forwarded request metadata with the *responder* identity so
    // the reply message never carries the original sender's badge (R-23).
    let mut reply_metadata = match active_turn.user_message_metadata() {
        Some(serde_json::Value::Object(map)) => map.clone(),
        _ => serde_json::Map::new(),
    };
    reply_metadata.retain(|key, _| !key.starts_with("sender"));
    reply_metadata.insert(
        "senderSessionId".to_string(),
        serde_json::json!(responder_session_id),
    );
    // Server-side timestamp for audit/timeline cross-checks. The forwarding
    // side only strips `sender*` keys, so this key passes through untouched.
    reply_metadata.insert("serverTime".to_string(), serde_json::json!(server_time));
    if let Some(role) = responder_role {
        reply_metadata.insert("senderRole".to_string(), serde_json::json!(role));
    }
    if let Some(depth) = responder_depth {
        reply_metadata.insert("senderDepth".to_string(), serde_json::json!(depth));
    }
    AgentSessionReplyAction::Forward(AgentSessionReplyPlan {
        target_session_id: reply_route.source_session_id.clone(),
        target_workspace_path: reply_route.source_workspace_path.clone(),
        target_remote_connection_id: reply_route.source_remote_connection_id.clone(),
        target_remote_ssh_host: reply_route.source_remote_ssh_host.clone(),
        user_input: outcome.reply_text(),
        reminder_text: reminder_lines.join("\n"),
        user_message_metadata: Some(serde_json::Value::Object(reply_metadata)),
    })
}

pub fn resolve_dialog_steering_action(
    active_turn_id: Option<&str>,
    session_id: &str,
    turn_id: &str,
    content: String,
    display_content: Option<String>,
    steering_id: String,
    created_at: SystemTime,
    prepended_reminders: Vec<AgentDialogPrependedReminder>,
) -> DialogSteeringAction {
    if active_turn_id != Some(turn_id) {
        return DialogSteeringAction::Reject {
            error: format!(
                "Dialog turn is no longer running and cannot be steered: session_id={session_id}, turn_id={turn_id}"
            ),
        };
    }

    let display = display_content.unwrap_or_else(|| content.clone());
    DialogSteeringAction::Buffer {
        injection: RoundInjection {
            id: steering_id.clone(),
            kind: RoundInjectionKind::UserSteering,
            execution_policy: RoundInjectionKind::UserSteering.default_execution_policy(),
            target: RoundInjectionTarget::ExactTurn(turn_id.to_string()),
            content,
            display_content: display,
            created_at,
            prepended_reminders,
        },
        outcome: DialogSteerOutcome::Buffered {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            steering_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_turn(turn_id: &str) -> ActiveDialogTurn {
        ActiveDialogTurn::new(
            turn_id.to_string(),
            None,
            None,
            None,
            "agentic".to_string(),
            "input".to_string(),
            None,
            DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
            None,
        )
    }

    fn injection(kind: RoundInjectionKind, content: &str) -> RoundInjection {
        RoundInjection {
            id: uuid_like(),
            kind,
            execution_policy: kind.default_execution_policy(),
            target: RoundInjectionTarget::CurrentRunningTurn,
            content: content.to_string(),
            display_content: content.to_string(),
            created_at: SystemTime::now(),
            prepended_reminders: Vec::new(),
        }
    }

    fn uuid_like() -> String {
        format!("injection-{}", std::process::id())
    }

    #[test]
    fn injection_buffer_deduplicates_background_result_notifications() {
        let buffer = SessionRoundInjectionBuffer::default();
        // A notification storm: N identical background-result entries for the
        // same session within the 5s window must collapse to a single pending
        // entry (one model request) while keeping the fixed template text
        // byte-identical.
        for _ in 0..5 {
            buffer.push("session-1", injection(RoundInjectionKind::BackgroundResult, "bg"));
        }
        let pending = buffer.drain_for_turn("session-1", "turn-1");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, RoundInjectionKind::BackgroundResult);
    }

    #[test]
    fn injection_buffer_keeps_background_result_notification_after_window() {
        let buffer = SessionRoundInjectionBuffer::default();
        // 后台通知 = 必要功能：5s 窗口之外的同类通知是新的真实事件，必须保留。
        let now = SystemTime::now();
        let first = RoundInjection {
            created_at: now - NOTIFICATION_DEDUP_WINDOW - Duration::from_secs(1),
            ..injection(RoundInjectionKind::BackgroundResult, "bg")
        };
        let second = injection(RoundInjectionKind::BackgroundResult, "bg");
        buffer.push("session-1", first);
        buffer.push("session-1", second);
        let pending = buffer.drain_for_turn("session-1", "turn-1");
        assert_eq!(pending.len(), 2, "notification beyond the window is a new event");
    }

    #[test]
    fn consumed_steering_is_not_reinjected_after_acknowledge() {
        let buffer = SessionRoundInjectionBuffer::default();
        // 用户消息注入（drain）后经 acknowledge 标记已消费：同一内容再次
        // 经 steering 通道推入必须被抑制（2-7 次重复注入的根因）。
        let mut steering = injection(RoundInjectionKind::UserSteering, "check tests");
        steering.id = "steer-1".to_string();
        buffer.push("session-1", steering.clone());
        let drained = buffer.drain_for_turn("session-1", "turn-1");
        assert_eq!(drained.len(), 1);
        buffer.acknowledge_injection("session-1", "steer-1");

        // 同内容重复推入：被消费确认抑制。
        buffer.push("session-1", steering);
        let drained_again = buffer.drain_for_turn("session-1", "turn-2");
        assert!(
            drained_again.is_empty(),
            "consumed steering must not be re-injected"
        );
    }

    #[test]
    fn distinct_steering_survives_after_one_is_consumed() {
        let buffer = SessionRoundInjectionBuffer::default();
        let mut first = injection(RoundInjectionKind::UserSteering, "first message");
        first.id = "steer-1".to_string();
        buffer.push("session-1", first);
        buffer.drain_for_turn("session-1", "turn-1");
        buffer.acknowledge_injection("session-1", "steer-1");

        // 不同内容的消息不受已消费标记影响，必须正常注入。
        let second = injection(RoundInjectionKind::UserSteering, "second message");
        buffer.push("session-1", second);
        let drained = buffer.drain_for_turn("session-1", "turn-2");
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].content, "second message");
    }

    #[test]
    fn undelivered_steering_is_retrievable_after_turn_end() {
        let buffer = SessionRoundInjectionBuffer::default();
        // turn 结束时仍未消费的 UserSteering 必须可被取出转交 follow-up，
        // 而不是静默丢弃（真实用户消息零丢失）。
        let mut steering = injection(RoundInjectionKind::UserSteering, "still pending");
        steering.target = RoundInjectionTarget::ExactTurn("turn-1".to_string());
        buffer.push("session-1", steering);

        let undelivered = buffer.drain_undelivered_steering("session-1", "turn-1");
        assert_eq!(undelivered.len(), 1);
        assert_eq!(undelivered[0].content, "still pending");
        // 取出后缓冲为空：不残留。
        assert_eq!(buffer.pending_count("session-1"), 0);
    }

    #[test]
    fn injection_buffer_deduplicates_identical_user_steering() {
        let buffer = SessionRoundInjectionBuffer::default();
        // The same user message re-steered 3 times must be injected once.
        buffer.push(
            "session-1",
            injection(RoundInjectionKind::UserSteering, "check tests"),
        );
        buffer.push(
            "session-1",
            injection(RoundInjectionKind::UserSteering, "check tests"),
        );
        buffer.push(
            "session-1",
            injection(RoundInjectionKind::UserSteering, "check tests"),
        );
        let pending = buffer.drain_for_turn("session-1", "turn-1");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].content, "check tests");
    }

    #[test]
    fn injection_buffer_keeps_distinct_user_steering_messages() {
        let buffer = SessionRoundInjectionBuffer::default();
        // Distinct user messages must never be collapsed.
        buffer.push(
            "session-1",
            injection(RoundInjectionKind::UserSteering, "first message"),
        );
        buffer.push(
            "session-1",
            injection(RoundInjectionKind::UserSteering, "second message"),
        );
        let pending = buffer.drain_for_turn("session-1", "turn-1");
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].content, "first message");
        assert_eq!(pending[1].content, "second message");
    }

    #[test]
    fn dialog_turn_queue_any_matching_sees_queued_turns() {
        let queue = DialogTurnQueue::<&'static str>::default();
        queue
            .enqueue("session-1", "alpha", DialogQueuePriority::Normal)
            .expect("enqueue");
        assert!(queue.any_matching("session-1", |turn| *turn == "alpha"));
        assert!(!queue.any_matching("session-1", |turn| *turn == "beta"));
        assert!(!queue.any_matching("other-session", |turn| *turn == "alpha"));
    }

    #[test]
    fn active_turn_store_ignores_an_outcome_from_an_older_turn_generation() {
        let store = ActiveDialogTurnStore::default();
        store.insert("session-1", active_turn("turn-new"));

        assert!(matches!(
            store.take_for_outcome("session-1", "turn-old"),
            ActiveDialogTurnTakeResult::DifferentTurn
        ));
        let ActiveDialogTurnTakeResult::Matched(turn) =
            store.take_for_outcome("session-1", "turn-new")
        else {
            panic!("current turn should be removed");
        };
        assert_eq!(turn.turn_id(), "turn-new");
        assert!(matches!(
            store.take_for_outcome("session-1", "turn-new"),
            ActiveDialogTurnTakeResult::Absent
        ));
    }

    #[test]
    fn dialog_turn_queue_reclaims_empty_session_entries() {
        let queue = DialogTurnQueue::with_max_depth(4);
        queue
            .enqueue("dequeue", 1, DialogQueuePriority::Normal)
            .expect("enqueue");
        queue
            .enqueue("remove", 2, DialogQueuePriority::Normal)
            .expect("enqueue");

        assert_eq!(queue.dequeue_next("dequeue"), Some(1));
        assert_eq!(
            queue.remove_first_matching("remove", |turn| *turn == 2),
            Some(2)
        );
        assert!(queue.inner.is_empty());
    }

    #[test]
    fn outcome_lifecycle_dispatches_completed_turn_and_verifies_goal() {
        let outcome = TurnOutcome::Completed {
            turn_id: "turn_1".to_string(),
            final_response: "done".to_string(),
        };

        let plan = resolve_turn_outcome_lifecycle_plan(&outcome, true);

        assert_eq!(plan.status, TurnOutcomeStatus::Completed);
        assert_eq!(plan.queue_action, TurnOutcomeQueueAction::DispatchNext);
        assert!(plan.drain_finished_turn_injections);
        assert_eq!(
            plan.goal_continuation,
            GoalContinuationAfterTurnAction::Evaluate {
                turn_completed: true
            }
        );
        assert!(plan.dispatch_next());
        assert!(!plan.clear_queue());
    }

    #[test]
    fn outcome_lifecycle_aborts_goal_continuation_for_cancelled_turn() {
        let outcome = TurnOutcome::Cancelled {
            turn_id: "turn_1".to_string(),
        };

        let plan = resolve_turn_outcome_lifecycle_plan(&outcome, true);

        assert_eq!(plan.status, TurnOutcomeStatus::Cancelled);
        assert_eq!(plan.queue_action, TurnOutcomeQueueAction::DispatchNext);
        assert_eq!(
            plan.goal_continuation,
            GoalContinuationAfterTurnAction::AbortForCancelled
        );
        assert!(plan.dispatch_next());
        assert!(!plan.clear_queue());
    }

    #[test]
    fn outcome_lifecycle_clears_queue_for_failed_turn_and_verifies_goal() {
        let outcome = TurnOutcome::Failed {
            turn_id: "turn_1".to_string(),
            error: "boom".to_string(),
        };

        let plan = resolve_turn_outcome_lifecycle_plan(&outcome, true);

        assert_eq!(plan.status, TurnOutcomeStatus::Failed);
        assert_eq!(plan.queue_action, TurnOutcomeQueueAction::ClearQueue);
        assert_eq!(
            plan.goal_continuation,
            GoalContinuationAfterTurnAction::Evaluate {
                turn_completed: false
            }
        );
        assert!(!plan.dispatch_next());
        assert!(plan.clear_queue());
    }

    #[test]
    fn outcome_lifecycle_skips_goal_when_no_active_turn_exists() {
        let outcome = TurnOutcome::Completed {
            turn_id: "turn_1".to_string(),
            final_response: "done".to_string(),
        };

        let plan = resolve_turn_outcome_lifecycle_plan(&outcome, false);

        assert_eq!(
            plan.goal_continuation,
            GoalContinuationAfterTurnAction::SkipNoActiveTurn
        );
        assert!(plan.dispatch_next());
    }

    #[test]
    fn dialog_steering_rejects_when_target_turn_is_not_running() {
        let action = resolve_dialog_steering_action(
            Some("turn-running"),
            "session-1",
            "turn-finished",
            "urgent correction".to_string(),
            None,
            "steering-1".to_string(),
            SystemTime::now(),
            Vec::new(),
        );

        let DialogSteeringAction::Reject { error } = action else {
            panic!("steering a non-running turn must be rejected");
        };
        assert!(error.contains("no longer running"));
    }

    #[test]
    fn dialog_steering_buffers_user_steering_for_the_active_turn() {
        let action = resolve_dialog_steering_action(
            Some("turn-running"),
            "session-1",
            "turn-running",
            "urgent correction".to_string(),
            Some("display text".to_string()),
            "steering-1".to_string(),
            SystemTime::now(),
            Vec::new(),
        );

        let DialogSteeringAction::Buffer { injection, outcome } = action else {
            panic!("steering the active turn must be buffered");
        };
        assert_eq!(injection.kind, RoundInjectionKind::UserSteering);
        assert_eq!(
            injection.execution_policy,
            RoundInjectionKind::UserSteering.default_execution_policy()
        );
        assert_eq!(
            injection.target,
            RoundInjectionTarget::ExactTurn("turn-running".to_string())
        );
        assert_eq!(injection.content, "urgent correction");
        assert_eq!(injection.display_content.as_str(), "display text");

        let DialogSteerOutcome::Buffered {
            session_id,
            turn_id,
            steering_id,
        } = outcome;
        assert_eq!(session_id, "session-1");
        assert_eq!(turn_id, "turn-running");
        assert_eq!(steering_id, "steering-1");
    }
}
