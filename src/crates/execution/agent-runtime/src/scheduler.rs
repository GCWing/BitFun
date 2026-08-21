//! Scheduler owner decisions.

use crate::events::turn_outcome_kind;
use crate::thread_goal::{build_objective_updated_plan, build_thread_goal_continuation_plan};
use bitfun_runtime_ports::{
    should_skip_agent_session_reply, should_suppress_agent_session_cancelled_reply,
    AgentDialogPrependedReminder, AgentInputAttachment, AgentSessionReplyRoute,
    DialogQueuePriority, DialogRoundInjectionSource, DialogSessionStateFact, DialogSteerOutcome,
    DialogSubmissionPolicy, DialogTriggerSource, RoundInjection, RoundInjectionKind,
    RoundInjectionTarget, RoundInjectionToolPreemption, ThreadGoal,
    MAX_THREAD_GOAL_AUTO_CONTINUATIONS,
};
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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

    pub fn matches_agent_session_request(&self, session_id: &str, turn_id: &str) -> bool {
        self.inner
            .get(session_id)
            .is_some_and(|turn| turn.turn_id() == turn_id && turn.is_agent_session_request())
    }

    /// User input of the currently active turn for session_id, if any.
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

/// Suppression marks for urgent-steered turns (R-ASYNC-01 项2, fixed by
/// urgent-reply-01 方案 B). Unlike the generic [`DialogReplySuppressionSet`],
/// each entry records the *injector* session id that steered the message into
/// the running turn. At turn completion the consumer compares this injector
/// against the turn's `reply_route`: when the route points back at the
/// injector, the auto-reply is the *only* delivery channel the injector waits
/// on (the UserSteering channel has no reply capability), so the mark must NOT
/// suppress it. The mark only suppresses when the turn's reply route belongs
/// to someone else (a duplicate auto-reply scenario).
#[derive(Debug, Default)]
pub struct InjectedTurnReplySuppressionSet {
    inner: dashmap::DashMap<(String, String), String>,
}

impl InjectedTurnReplySuppressionSet {
    pub fn mark(&self, session_id: &str, turn_id: &str, injector_session_id: &str) {
        self.inner.insert(
            (session_id.to_string(), turn_id.to_string()),
            injector_session_id.to_string(),
        );
    }

    /// Remove and return the recorded injector session id, if any.
    pub fn take(&self, session_id: &str, turn_id: &str) -> Option<String> {
        self.inner
            .remove(&(session_id.to_string(), turn_id.to_string()))
            .map(|(_, injector)| injector)
    }

    /// Remove every entry belonging to `session_id`, regardless of turn id.
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
    max_depth: std::sync::atomic::AtomicUsize,
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
            max_depth: std::sync::atomic::AtomicUsize::new(max_depth),
            inner: dashmap::DashMap::new(),
        }
    }

    /// Updates the queue depth cap at runtime (群聊阈值参数配置化, R-GC-26).
    ///
    /// Used to inject `group_chat.queue_limit` after construction; a value of
    /// `0` is ignored so a misconfigured document cannot disable the cap.
    pub fn set_max_depth(&self, max_depth: usize) {
        if max_depth > 0 {
            self.max_depth
                .store(max_depth, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub fn max_depth(&self) -> usize {
        self.max_depth.load(std::sync::atomic::Ordering::Relaxed)
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
        let max_depth = self.max_depth();
        if queue.len() >= max_depth {
            return Err(DialogTurnQueueError::Full {
                session_id: session_id.to_string(),
                max_depth,
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

#[derive(Debug, Clone, PartialEq)]
// Buffer carries a full RoundInjection; kept unboxed for direct field access
// at every match site.
#[allow(clippy::large_enum_variant)]
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
    /// Consumed UserSteering keys so a user message that was already injected
    /// into this session is never injected again — the observable driver of the
    /// 2-7x UserSteering duplicates.
    ///
    /// TOKEN-01: keys are `(session_id, steering_id)` when the injection
    /// carried a dedup marker, and `(session_id, content)` as a content-based
    /// fallback for legacy steering entries without an id. The id-keyed path
    /// avoids content scanning (which risks prompt-cache prefix drift).
    /// Cleared when the session is cleared/recycled (`clear`).
    consumed_steering: dashmap::DashSet<(String, String)>,
    /// Injection-id → content map for steering entries drained but not yet
    /// acknowledged; `acknowledge_injection` looks the content up here and
    /// records it into `consumed_steering`.
    pending_steering_content: dashmap::DashMap<(String, String), String>,
    /// Injection-id → steering-id map for steering entries drained but not yet
    /// acknowledged. When the injection carries a dedup marker (TOKEN-01), the
    /// acknowledgement records `(session, steering_id)` instead of the content
    /// key, so duplicate pushes are suppressed by metadata, not by scanning
    /// the prompt payload.
    pending_steering_ids: dashmap::DashMap<(String, String), String>,
}

/// R-ASYNC-01（项1）：移除 buffer push 5s 窗口去重。
/// 同 (session_id, agent_type) 键的多条后台通知不再合并——全部入队逐条注入。
/// 消费确认（mark_steering_consumed / acknowledge_injection，TOKEN-01）保留。
impl SessionRoundInjectionBuffer {
    /// Push a round injection into the per-session pending buffer.
    ///
    /// R-ASYNC-01（项1）：不再按窗口/键去重——BackgroundResult 同键多条、
    /// ThreadGoal 窗口内重复、UserSteering 同内容多条均逐条保留（移除排队合并，
    /// 通知不再被丢弃）。UserSteering 消费确认（TOKEN-01）保留：已注入过
    /// （acked）的同 steering_id/同内容不重复注入。
    ///
    /// The dedup happens at push time, before the engine drains the buffer at a
    /// round boundary. It never mutates the injected text, the injection
    /// position, or the per-kind template, so the provider-side prompt prefix
    /// for the *kept* injection is byte-identical to the pre-fix behavior.
    pub fn push(&self, session_id: &str, message: RoundInjection) {
        // UserSteering 消费确认：同内容/同 steering_id 已被本会话注入过
        // （acked）→ 不重复注入。注入结构（模板/位置/顺序）零改动，仅抑制
        // 已消费内容的重复投递。TOKEN-01：优先按 steering_id 元数据键判断，
        // 无 id 的遗留条目回退内容键。
        if message.kind == RoundInjectionKind::UserSteering
            && self.steering_already_consumed(session_id, &message)
        {
            log::debug!(
                "UserSteering already consumed; suppressing re-injection: session_id={}, content_len={}, steering_id={:?}",
                session_id,
                message.content.len(),
                message.dedup_key()
            );
            return;
        }
        self.inner
            .entry(session_id.to_string())
            .or_default()
            .push(message);
    }

    /// Record that a UserSteering was actually injected for the session, so
    /// later duplicate pushes are suppressed. Keys are cleared when the
    /// session is cleared/recycled (`clear`). TOKEN-01: prefers the steering
    /// id metadata key when available, falling back to the content key for
    /// legacy steering entries without an id.
    pub fn mark_steering_consumed(
        &self,
        session_id: &str,
        content: &str,
        steering_id: Option<&str>,
    ) {
        let key = steering_id
            .map(|id| format!("id:{id}"))
            .unwrap_or_else(|| format!("content:{content}"));
        self.consumed_steering.insert((session_id.to_string(), key));
    }

    /// Whether the (session, key) is currently marked consumed. TOKEN-01:
    /// the id metadata key is authoritative when present; the content key
    /// remains as a fallback for legacy entries.
    fn steering_already_consumed(&self, session_id: &str, message: &RoundInjection) -> bool {
        match message.dedup_key() {
            Some(steering_id) => self
                .consumed_steering
                .contains(&(session_id.to_string(), format!("id:{steering_id}"))),
            None => self.consumed_steering.contains(&(
                session_id.to_string(),
                format!("content:{}", message.content),
            )),
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
                        if let Some(steering_id) = msg.dedup_key() {
                            self.pending_steering_ids.insert(
                                (session_id.to_string(), msg.id.clone()),
                                steering_id.to_string(),
                            );
                        }
                    }
                    taken.push(msg);
                }
                RoundInjectionTarget::CurrentRunningTurn => {
                    if msg.kind == RoundInjectionKind::UserSteering {
                        self.pending_steering_content.insert(
                            (session_id.to_string(), msg.id.clone()),
                            msg.content.clone(),
                        );
                        if let Some(steering_id) = msg.dedup_key() {
                            self.pending_steering_ids.insert(
                                (session_id.to_string(), msg.id.clone()),
                                steering_id.to_string(),
                            );
                        }
                    }
                    taken.push(msg);
                }
                RoundInjectionTarget::ExactTurn(_) => keep.push(msg),
            }
        }
        *entry = keep;
        taken
    }

    /// Drop injections whose target was only "whatever turn is currently
    /// running" while retaining messages explicitly bound to a Turn. An
    /// interrupted Turn may resume later, but an unscoped injection must not
    /// leak into a different Turn if the user abandons it.
    pub fn discard_current_running(&self, session_id: &str) {
        let Some(mut entry) = self.inner.get_mut(session_id) else {
            return;
        };
        entry.retain(|message| matches!(message.target, RoundInjectionTarget::ExactTurn(_)));
    }

    /// Look up the drained steering content / steering id for `injection_id`
    /// and record it as consumed for the session, so a duplicate push is
    /// suppressed. TOKEN-01: prefers the steering-id metadata key when the
    /// injection carried a dedup marker; falls back to the content key for
    /// legacy steering entries without an id.
    pub fn acknowledge_injection(&self, session_id: &str, injection_id: &str) {
        let steering_id = self
            .pending_steering_ids
            .remove(&(session_id.to_string(), injection_id.to_string()))
            .map(|(_, id)| id);
        if let Some((_, content)) = self
            .pending_steering_content
            .remove(&(session_id.to_string(), injection_id.to_string()))
        {
            self.mark_steering_consumed(session_id, &content, steering_id.as_deref());
        }
    }

    /// Drain UserSteering entries still pending for `turn_id` that were never
    /// consumed (the turn ended before a round boundary drained them). These
    /// are returned so the scheduler can re-deliver them as a normal follow-up
    /// turn instead of silently dropping a real user message.
    pub fn drain_undelivered_steering(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> Vec<RoundInjection> {
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
        self.pending_steering_ids
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
        attachments: Vec::new(),
        metadata: serde_json::Map::new(),
        created_at,
        prepended_reminders: Vec::new(),
    }
}

pub fn target_background_delivery_injection_to_turn(
    mut injection: RoundInjection,
    turn_id: String,
) -> RoundInjection {
    injection.target = RoundInjectionTarget::ExactTurn(turn_id);
    injection
}

pub fn resolve_background_delivery_injection_for_turn(
    kind: BackgroundInjectionKind,
    injection_id: String,
    content: String,
    display_content: Option<String>,
    created_at: SystemTime,
    turn_id: String,
) -> RoundInjection {
    target_background_delivery_injection_to_turn(
        resolve_background_delivery_injection(
            kind,
            injection_id,
            content,
            display_content,
            created_at,
        ),
        turn_id,
    )
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
    /// Turn was intentionally interrupted and may be recovered in place.
    Interrupted {
        turn_id: String,
        execution_generation: u32,
    },
    /// Turn failed with an error.
    Failed { turn_id: String, error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcomeQueueAction {
    DispatchNext,
    HoldQueue,
    ClearQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcomeStatus {
    Completed,
    Cancelled,
    Interrupted,
    Failed,
}

impl TurnOutcomeStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
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
            | Self::Interrupted { turn_id, .. }
            | Self::Failed { turn_id, .. } => turn_id,
        }
    }

    pub fn status(&self) -> TurnOutcomeStatus {
        match self {
            Self::Completed { .. } => TurnOutcomeStatus::Completed,
            Self::Cancelled { .. } => TurnOutcomeStatus::Cancelled,
            Self::Interrupted { .. } => TurnOutcomeStatus::Interrupted,
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
            Self::Interrupted { .. } => {
                "The target session interrupted this request before producing a final answer."
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
            Self::Interrupted { .. } => TurnOutcomeQueueAction::HoldQueue,
            Self::Failed { .. } => TurnOutcomeQueueAction::ClearQueue,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalContinuationAfterTurnAction {
    SkipNoActiveTurn,
    AbortForCancelled,
    AbortForInterrupted,
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
            TurnOutcomeStatus::Interrupted => GoalContinuationAfterTurnAction::AbortForInterrupted,
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
        // Interrupted is a resumable generation of the same Turn. Steering
        // and background results already accepted for that Turn remain valid
        // until the user either resumes it or explicitly abandons it.
        drain_finished_turn_injections: status != TurnOutcomeStatus::Interrupted,
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
    suppress_injected_turn_reply: bool,
) -> AgentSessionReplyAction {
    if !active_turn.is_agent_session_request() {
        return AgentSessionReplyAction::NoReply;
    }

    if should_skip_agent_session_reply(
        turn_outcome_kind(outcome),
        suppressed_cancelled_reply,
        suppress_injected_turn_reply,
    ) {
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

// Public steering-resolution helper; the parameter set mirrors the dialog
// turn wire fields and is intentionally kept flat.
#[allow(clippy::too_many_arguments)]
pub fn resolve_dialog_steering_action(
    active_turn_id: Option<&str>,
    session_id: &str,
    turn_id: &str,
    content: String,
    display_content: Option<String>,
    attachments: Vec<AgentInputAttachment>,
    metadata: serde_json::Map<String, serde_json::Value>,
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
            attachments,
            metadata,
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
            attachments: Vec::new(),
            metadata: serde_json::Map::new(),
            created_at: SystemTime::now(),
            prepended_reminders: Vec::new(),
        }
    }

    fn uuid_like() -> String {
        format!("injection-{}", std::process::id())
    }

    #[test]
    fn subagent_steering_is_drained_only_by_its_own_session_and_turn() {
        // 防回退：子代理 ExecutionContext.round_injection 启用后，引擎会以
        // 子代理自身的 (session_id, turn_id) 调 take_pending → drain_for_turn。
        // ExactTurn 定向 + session_id 键隔离保证：子代理只消费指向自己的
        // steering，父会话条目永不误吞（coordinator.rs 子代理上下文
        // round_injection 原为 None 导致 steering 永不消费的回归防线）。
        let buffer = SessionRoundInjectionBuffer::default();
        let mut steering = injection(RoundInjectionKind::UserSteering, "steer the subagent");
        steering.id = "subagent-steer-1".to_string();
        steering.target = RoundInjectionTarget::ExactTurn("subagent-turn".to_string());
        buffer.push("subagent-session", steering);

        // 父会话有一条指向父会话 turn 的 steering，不得被子代理消费。
        let mut parent_steering = injection(RoundInjectionKind::UserSteering, "steer the parent");
        parent_steering.id = "parent-steer-1".to_string();
        parent_steering.target = RoundInjectionTarget::ExactTurn("parent-turn".to_string());
        buffer.push("parent-session", parent_steering);

        // 子代理消费自己 session 的 ExactTurn 条目。
        let subagent_pending = buffer.drain_for_turn("subagent-session", "subagent-turn");
        assert_eq!(subagent_pending.len(), 1);
        assert_eq!(subagent_pending[0].id, "subagent-steer-1");

        // 父会话条目仍然保留（未被误吞），父会话可正常消费。
        assert_eq!(
            buffer.pending_count("parent-session"),
            1,
            "parent session steering must survive the subagent drain"
        );
        let parent_pending = buffer.drain_for_turn("parent-session", "parent-turn");
        assert_eq!(parent_pending.len(), 1);
        assert_eq!(parent_pending[0].id, "parent-steer-1");
    }

    #[test]
    fn subagent_drain_does_not_consume_parent_turn_steering_for_same_session_key() {
        // 防回退：即便父子共用一个 session_id 键（理论上不存在，子代理会话
        // 拥有独立 session_id），ExactTurn 定向仍保证子代理 turn 不消费指向
        // 父 turn 的条目——drain_for_turn 对不匹配的 ExactTurn 条目保留。
        let buffer = SessionRoundInjectionBuffer::default();
        let mut parent_steering = injection(RoundInjectionKind::UserSteering, "parent turn msg");
        parent_steering.id = "parent-steer-1".to_string();
        parent_steering.target = RoundInjectionTarget::ExactTurn("parent-turn".to_string());
        buffer.push("shared-session", parent_steering);

        let drained = buffer.drain_for_turn("shared-session", "subagent-turn");
        assert!(
            drained.is_empty(),
            "steering targeting a different (parent) turn must be retained"
        );
        assert_eq!(buffer.pending_count("shared-session"), 1);
        let parent_pending = buffer.drain_for_turn("shared-session", "parent-turn");
        assert_eq!(parent_pending.len(), 1);
        assert_eq!(parent_pending[0].id, "parent-steer-1");
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
    fn consumed_steering_id_suppresses_reinjection_without_content_scanning() {
        // TOKEN-01 防回退标记：消费确认记录 steering_id 元数据键。同一
        // steering 事件（同一 steering_id）在后续轮/turn 再次推入时必须被
        // 抑制——即便内容被包装文本包裹（content 键无法匹配，id 键仍命中）。
        let buffer = SessionRoundInjectionBuffer::default();
        let mut steering = injection(RoundInjectionKind::UserSteering, "check tests");
        steering.id = "steer-001".to_string();
        buffer.push("session-1", steering.clone());
        let drained = buffer.drain_for_turn("session-1", "turn-1");
        assert_eq!(drained.len(), 1);
        buffer.acknowledge_injection("session-1", "steer-001");

        // 同一 steering_id 再次 push（例如跨 turn 残留转交后回灌）：被 id 键抑制。
        let mut re_pushed = injection(RoundInjectionKind::UserSteering, "check tests");
        re_pushed.id = "steer-001".to_string();
        buffer.push("session-1", re_pushed);
        let drained_again = buffer.drain_for_turn("session-1", "turn-2");
        assert!(
            drained_again.is_empty(),
            "same steering_id must not be re-injected"
        );
    }

    #[test]
    fn distinct_steering_ids_survive_after_one_is_consumed_by_id() {
        // TOKEN-01 防回退标记：id 键去重不得误伤不同 steering 事件（不同
        // steering_id），即使它们恰好携带相同内容（真实用户两次相同输入）。
        let buffer = SessionRoundInjectionBuffer::default();
        let mut first = injection(RoundInjectionKind::UserSteering, "repeat me");
        first.id = "steer-1".to_string();
        buffer.push("session-1", first);
        buffer.drain_for_turn("session-1", "turn-1");
        buffer.acknowledge_injection("session-1", "steer-1");

        let mut second = injection(RoundInjectionKind::UserSteering, "repeat me");
        second.id = "steer-2".to_string();
        buffer.push("session-1", second);
        let drained = buffer.drain_for_turn("session-1", "turn-2");
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "steer-2");
    }

    #[test]
    fn legacy_content_key_fallback_suppresses_after_acknowledge() {
        // TOKEN-01 防回退标记回退路径：无 steering_id 的遗留条目仍按内容键
        // 抑制，行为与修复前一致（不因引入 id 键而退化）。
        let buffer = SessionRoundInjectionBuffer::default();
        let steering = injection(RoundInjectionKind::UserSteering, "legacy steering");
        buffer.push("session-1", steering.clone());
        let drained = buffer.drain_for_turn("session-1", "turn-1");
        assert_eq!(drained.len(), 1);
        buffer.acknowledge_injection("session-1", &drained[0].id);

        buffer.push("session-1", steering);
        let drained_again = buffer.drain_for_turn("session-1", "turn-2");
        assert!(
            drained_again.is_empty(),
            "legacy content key must still suppress duplicates"
        );
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
    fn dialog_turn_queue_set_max_depth_injects_configured_cap() {
        let queue: DialogTurnQueue<usize> = DialogTurnQueue::default();
        assert_eq!(queue.max_depth(), DEFAULT_MAX_DIALOG_QUEUE_DEPTH);

        // R-GC-26: group_chat.queue_limit injection path.
        queue.set_max_depth(7);
        assert_eq!(queue.max_depth(), 7);

        // A zero value must be ignored so a misconfigured document cannot
        // disable the cap (defense in depth).
        queue.set_max_depth(0);
        assert_eq!(queue.max_depth(), 7);

        // The cap is enforced by enqueue.
        let queue = DialogTurnQueue::with_max_depth(2);
        queue
            .enqueue("s", 1, DialogQueuePriority::Normal)
            .expect("first");
        queue
            .enqueue("s", 2, DialogQueuePriority::Normal)
            .expect("second");
        assert!(matches!(
            queue.enqueue("s", 3, DialogQueuePriority::Normal),
            Err(DialogTurnQueueError::Full { .. })
        ));
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
    fn outcome_lifecycle_holds_queue_for_interrupted_turn() {
        let outcome = TurnOutcome::Interrupted {
            turn_id: "turn_1".to_string(),
            execution_generation: 1,
        };

        let plan = resolve_turn_outcome_lifecycle_plan(&outcome, true);

        assert_eq!(plan.status, TurnOutcomeStatus::Interrupted);
        assert_eq!(plan.queue_action, TurnOutcomeQueueAction::HoldQueue);
        assert!(
            !plan.drain_finished_turn_injections,
            "same-turn injections must survive until recovery or explicit abandonment"
        );
        assert_eq!(
            plan.goal_continuation,
            GoalContinuationAfterTurnAction::AbortForInterrupted
        );
        assert!(!plan.dispatch_next());
        assert!(!plan.clear_queue());
    }

    #[test]
    fn interrupted_turn_discards_only_unscoped_current_turn_injections() {
        let buffer = SessionRoundInjectionBuffer::default();
        buffer.push(
            "session-1",
            resolve_background_delivery_injection(
                BackgroundInjectionKind::ThreadGoalObjectiveUpdated,
                "current".to_string(),
                "current-only".to_string(),
                None,
                SystemTime::now(),
            ),
        );
        buffer.push(
            "session-1",
            target_background_delivery_injection_to_turn(
                resolve_background_delivery_injection(
                    BackgroundInjectionKind::BackgroundResult,
                    "exact".to_string(),
                    "resume-me".to_string(),
                    None,
                    SystemTime::now(),
                ),
                "turn-1".to_string(),
            ),
        );

        buffer.discard_current_running("session-1");

        assert!(buffer.has_pending_for_turn("session-1", "turn-1"));
        assert!(!buffer.has_pending_for_turn("session-1", "turn-2"));
        assert_eq!(buffer.pending_count("session-1"), 1);
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
            Vec::new(),
            serde_json::Map::new(),
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
            Vec::new(),
            serde_json::Map::new(),
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
