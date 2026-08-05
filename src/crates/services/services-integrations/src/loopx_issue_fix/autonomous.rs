//! LoopX-owned control state for continuous issue fixing.
//!
//! BitFun persists no issue queue here. Selected issues are written directly as
//! LoopX agent todos, and every UI refresh is rebuilt from LoopX todo/quota
//! packets. The host scheduler only wakes the generated heartbeat prompt.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::{LoopxIssueFix, LoopxIssueFixError};

const ISSUE_INTAKE_ACTION: &str = "issue_fix_intake";
const MAX_USER_REASON_CHARS: usize = 240;

/// Host preamble prepended to LoopX's generated heartbeat contract.
///
/// LoopX owns every lifecycle rule (the `--compact` task body below it); this
/// header only translates host-surface concerns — where the agent runs, how
/// BitFun projects user gates, and what a tick means here. It must never add
/// lifecycle branching of its own.
const HEARTBEAT_HOST_PREAMBLE: &str = "\
You are BitFun's continuous Issue-Fix host agent. Each scheduled message in this \
conversation is one LoopX heartbeat tick. LoopX Kernel state is the ONLY source of \
truth: on every tick re-read it fresh through the `loopx` CLI, and disregard any \
conclusion from earlier messages in this conversation that conflicts with the \
current packets. Never invent issue or PR state, and never record progress \
anywhere except through LoopX writebacks.

The goal is continuous multi-issue repair. Each selected issue is an independent \
one-off advancement todo that must travel the full lifecycle — reproduce, patch in \
an isolated worktree, validate (failure-before / pass-after), publish, then monitor \
its pull request through the grouped lifecycle monitors — to validated terminal \
closeout, with every transition written back to LoopX. Do one bounded, verifiable \
segment per tick, then stop and wait for the next tick.

Host surface notes:
- You run inside a BitFun desktop chat session; BitFun's host loop owns scheduling. \
LoopX skill packs are not installed here — follow the contract below directly and \
consult `loopx <command> --help` when unsure.
- You are a process INSIDE the host application. NEVER force-kill processes you \
did not start in this very turn: what looks \"stale\" may be your host, its dev \
tooling, or another agent's work, and killing it terminates you mid-turn. When a \
build or file lock is contended, wait and retry, keep your build outputs inside \
your own worktree, or record a blocker todo — never clear processes.
- Work each repository todo in an isolated worktree, created under one sibling \
folder of the repository (<repository>-worktrees/). Worktrees and whatever build \
caches you create inside them are yours to reclaim: at terminal closeout remove \
the worktree (committed work lives on its branch), and never leave large build \
outputs behind on completed or abandoned work.
- Repository-specific policy — toolchains, validation commands, path boundaries — \
belongs in the goal's active state and registry, not in this prompt. Read it from \
there, and write durable local rules back to the active state as you learn them.
- Raise human decisions ONLY as typed LoopX user todos: `loopx todo add --role user \
--task-class user_gate --unblocks-todo-id <the todo this gate blocks> ...`, always \
linking the gate to the blocked todo. BitFun's Issue-Fix panel projects open gates \
to the user and records each decision through the LoopX todo lifecycle; a plain \
chat reply never grants authority.
- User-lane todo text is projected verbatim into BitFun's panel, so keep it to ONE \
compact line (<=160 chars) in the shape \"<action> <PR/issue ref> — <which issue it \
serves> · <state that justifies it now>\", e.g. \"Merge PR #2038 — fixes #1980 \
stream truncation · CI green, validated\". Include the full URL of the primary \
PR/issue. Drafted comments, long evidence, and reasoning go in --note or \
--evidence, never in the todo text.
- NOTIFY / DONT_NOTIFY in the contract below control only the final chat summary: \
for NOTIFY end with a concise user-facing summary (in the contract's notification \
language), for DONT_NOTIFY end with a single quiet status line.

--- LoopX heartbeat contract follows ---";
const REQUIRED_CAPABILITIES: [&str; 4] = ["shell", "filesystem_write", "git", "network"];
const HEARTBEAT_CAPABILITIES: [&str; 5] = [
    "shell",
    "filesystem_write",
    "git",
    "network",
    "external_evidence_poll",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueSelection {
    pub issue_ref: String,
    pub issue_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutonomousIssueTodo {
    pub issue_ref: String,
    pub issue_url: String,
    pub todo_id: String,
    pub status: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutonomousUserQuestion {
    pub todo_id: String,
    pub prompt: String,
}

/// One open user-lane todo for the panel's read-only "pending your action"
/// block: gates answer through the question card, actions (e.g. "review PR
/// #N") resolve on the provider side and close via the Kernel's monitors, so
/// no mutation surface is offered here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutonomousUserTodo {
    pub todo_id: String,
    pub task_class: String,
    pub text: String,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserDecision {
    Approve,
    Reject,
    Cancel,
}

impl UserDecision {
    fn as_loopx_value(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutonomousControlState {
    pub goal_id: String,
    pub agent_id: String,
    pub kernel_state: String,
    pub should_run: bool,
    pub action_required: bool,
    pub recommended_action: Option<String>,
    pub gate_prompt: Option<String>,
    pub selected_todo_id: Option<String>,
    pub issues: Vec<AutonomousIssueTodo>,
    pub user_question: Option<AutonomousUserQuestion>,
    pub user_todos: Vec<AutonomousUserTodo>,
}

/// A cheap projection for background polling: todo list only, no `quota
/// should-run`. LoopX appends a rollout event on every `should-run` call, so a
/// UI poll loop must not run it; gates and issue todos are fully derivable
/// from `todo list`, which is read-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutonomousLightState {
    pub goal_id: String,
    pub agent_id: String,
    pub action_required: bool,
    pub issues: Vec<AutonomousIssueTodo>,
    pub user_question: Option<AutonomousUserQuestion>,
    pub user_todos: Vec<AutonomousUserTodo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutonomousStartPlan {
    pub control: AutonomousControlState,
    pub heartbeat_prompt: String,
    pub added_issue_refs: Vec<String>,
}

#[derive(Debug, Error)]
pub enum AutonomousIssueFixError {
    #[error(transparent)]
    Loopx(#[from] LoopxIssueFixError),
    #[error("failed to read LoopX registry {path}: {source}")]
    RegistryRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("LoopX registry {path} is not valid JSON: {source}")]
    RegistryJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("expected exactly one active LoopX goal, found {0}")]
    ActiveGoalCount(usize),
    #[error("expected exactly one registered LoopX agent for goal {goal_id}, found {count}")]
    RegisteredAgentCount { goal_id: String, count: usize },
    #[error("LoopX {command} packet is missing required field {field}")]
    MissingField {
        command: &'static str,
        field: &'static str,
    },
    #[error("invalid issue selection: {0}")]
    InvalidSelection(String),
    #[error("invalid user response: {0}")]
    InvalidUserResponse(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlIdentity {
    goal_id: String,
    agent_id: String,
}

#[derive(Debug, Clone)]
pub struct AutonomousIssueFix {
    loopx: LoopxIssueFix,
}

impl AutonomousIssueFix {
    pub fn new(loopx: LoopxIssueFix) -> Self {
        Self { loopx }
    }

    /// Project the current Issue-Fix surface from LoopX without writing state.
    pub async fn inspect(
        &self,
        repository_path: &Path,
    ) -> Result<AutonomousControlState, AutonomousIssueFixError> {
        let identity = read_identity(repository_path)?;
        self.inspect_with_identity(repository_path, &identity).await
    }

    /// Cheap read-only projection for background polling: `todo list` only.
    ///
    /// Unlike [`Self::inspect`], this never invokes `quota should-run`, which
    /// appends a rollout event per call and would grow LoopX's event log
    /// unboundedly under a poll loop.
    pub async fn poll(
        &self,
        repository_path: &Path,
    ) -> Result<AutonomousLightState, AutonomousIssueFixError> {
        let identity = read_identity(repository_path)?;
        let todos = self.list_todos(repository_path, &identity).await?;
        let issues = todos
            .iter()
            .filter_map(project_issue_todo)
            .collect::<Vec<_>>();
        let user_question = project_user_question(&todos, &issues);
        Ok(AutonomousLightState {
            goal_id: identity.goal_id,
            agent_id: identity.agent_id,
            action_required: user_question.is_some(),
            issues,
            user_question,
            user_todos: project_user_todos(&todos),
        })
    }

    /// Resolve the control identity (active goal, registered agent) from the
    /// project registry without touching LoopX state. Host-loop management
    /// needs the goal id even when no LoopX command should run.
    pub fn identity(
        repository_path: &Path,
    ) -> Result<(String, String), AutonomousIssueFixError> {
        read_identity(repository_path).map(|identity| (identity.goal_id, identity.agent_id))
    }

    /// Regenerate the full host heartbeat prompt for the registered goal.
    ///
    /// The stored cron payload is a snapshot; callers should refresh it at
    /// every natural write point (start, gate answers) so LoopX upgrades that
    /// change the generated contract propagate without a manual restart.
    pub async fn heartbeat_prompt(
        &self,
        repository_path: &Path,
    ) -> Result<String, AutonomousIssueFixError> {
        let identity = read_identity(repository_path)?;
        self.heartbeat_prompt_with_identity(repository_path, &identity)
            .await
    }

    /// Persist every selected issue as a LoopX todo, then generate the host
    /// heartbeat from the resulting Kernel state.
    pub async fn start(
        &self,
        repository_path: &Path,
        repo: &str,
        issues: &[IssueSelection],
    ) -> Result<AutonomousStartPlan, AutonomousIssueFixError> {
        if issues.is_empty() {
            return Err(AutonomousIssueFixError::InvalidSelection(
                "at least one issue is required".to_string(),
            ));
        }
        let identity = read_identity(repository_path)?;
        let mut existing = self
            .list_issue_todos(repository_path, &identity)
            .await?
            .into_iter()
            .map(|todo| todo.issue_ref)
            .collect::<HashSet<_>>();
        let mut added_issue_refs = Vec::new();

        for issue in issues {
            validate_selection(issue)?;
            if !existing.insert(issue.issue_ref.clone()) {
                continue;
            }
            let task_repository = task_repository(repo, &issue.issue_url)?;
            let text = issue_todo_text(repo, issue);
            let mut args = vec![
                "todo".to_string(),
                "add".to_string(),
                "--goal-id".to_string(),
                identity.goal_id.clone(),
                "--role".to_string(),
                "agent".to_string(),
                "--text".to_string(),
                text,
                "--task-class".to_string(),
                "advancement_task".to_string(),
                "--action-kind".to_string(),
                ISSUE_INTAKE_ACTION.to_string(),
                "--task-repository".to_string(),
                task_repository,
                "--claimed-by".to_string(),
                identity.agent_id.clone(),
            ];
            for capability in REQUIRED_CAPABILITIES {
                args.push("--required-capability".to_string());
                args.push(capability.to_string());
            }
            self.loopx.json_in(repository_path, args).await?;
            added_issue_refs.push(issue.issue_ref.clone());
        }

        let (control, heartbeat_prompt) = tokio::try_join!(
            self.inspect_with_identity(repository_path, &identity),
            self.heartbeat_prompt_with_identity(repository_path, &identity),
        )?;

        Ok(AutonomousStartPlan {
            control,
            heartbeat_prompt,
            added_issue_refs,
        })
    }

    /// Resolve the currently projected Issue-Fix user gate through LoopX's
    /// typed todo lifecycle. Only the projected `user_gate` todo is accepted;
    /// other user todos (reading queues, `user_action`) are rejected.
    pub async fn answer_user_question(
        &self,
        repository_path: &Path,
        todo_id: &str,
        decision: UserDecision,
        reason: Option<&str>,
    ) -> Result<AutonomousControlState, AutonomousIssueFixError> {
        let identity = read_identity(repository_path)?;
        let current = self
            .inspect_with_identity(repository_path, &identity)
            .await?;
        let question = current.user_question.as_ref().ok_or_else(|| {
            AutonomousIssueFixError::InvalidUserResponse(
                "there is no open Issue-Fix user question".to_string(),
            )
        })?;
        if question.todo_id != todo_id.trim() {
            return Err(AutonomousIssueFixError::InvalidUserResponse(format!(
                "todo {} is not the current Issue-Fix user question",
                todo_id.trim()
            )));
        }

        let args = user_decision_args(&identity, &question.todo_id, decision, reason)?;
        self.loopx.json_in(repository_path, args).await?;
        self.inspect_with_identity(repository_path, &identity).await
    }

    async fn inspect_with_identity(
        &self,
        repository_path: &Path,
        identity: &ControlIdentity,
    ) -> Result<AutonomousControlState, AutonomousIssueFixError> {
        let quota_args = scheduler_args("quota", "should-run", identity, None);
        let (quota, todos) = tokio::try_join!(
            async {
                self.loopx
                    .json_in(repository_path, quota_args)
                    .await
                    .map_err(AutonomousIssueFixError::from)
            },
            self.list_todos(repository_path, identity),
        )?;

        let mut issues = todos
            .iter()
            .filter_map(project_issue_todo)
            .collect::<Vec<_>>();
        let selected_todo_id = optional_string(&quota, "selected_todo", "todo_id")
            .filter(|todo_id| issues.iter().any(|issue| issue.todo_id == *todo_id));
        for issue in &mut issues {
            issue.selected = selected_todo_id.as_deref() == Some(issue.todo_id.as_str());
        }
        let user_question = project_user_question(&todos, &issues);
        let action_required = user_question.is_some();

        Ok(AutonomousControlState {
            goal_id: identity.goal_id.clone(),
            agent_id: identity.agent_id.clone(),
            kernel_state: required_string(&quota, "quota should-run", "state")?,
            should_run: quota
                .get("should_run")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            action_required,
            recommended_action: optional_top_string(&quota, "recommended_action"),
            gate_prompt: user_question
                .as_ref()
                .map(|question| question.prompt.clone()),
            selected_todo_id,
            issues,
            user_question,
            user_todos: project_user_todos(&todos),
        })
    }

    async fn list_issue_todos(
        &self,
        repository_path: &Path,
        identity: &ControlIdentity,
    ) -> Result<Vec<AutonomousIssueTodo>, AutonomousIssueFixError> {
        let todos = self.list_todos(repository_path, identity).await?;
        Ok(todos.iter().filter_map(project_issue_todo).collect())
    }

    async fn list_todos(
        &self,
        repository_path: &Path,
        identity: &ControlIdentity,
    ) -> Result<Vec<Value>, AutonomousIssueFixError> {
        let packet = self
            .loopx
            .json_in(
                repository_path,
                [
                    "todo",
                    "list",
                    "--goal-id",
                    identity.goal_id.as_str(),
                    "--agent-id",
                    identity.agent_id.as_str(),
                ],
            )
            .await?;
        packet
            .get("todos")
            .and_then(Value::as_array)
            .cloned()
            .ok_or(AutonomousIssueFixError::MissingField {
                command: "todo list",
                field: "todos",
            })
    }

    async fn heartbeat_prompt_with_identity(
        &self,
        repository_path: &Path,
        identity: &ControlIdentity,
    ) -> Result<String, AutonomousIssueFixError> {
        // `--compact` rather than `--thin`: the thin dispatcher delegates to
        // LoopX skill packs (`loopx-project`) that are not installed in a
        // BitFun agent session, while the compact body carries the full
        // should_run lifecycle inline.
        let packet = self
            .loopx
            .json_in(
                repository_path,
                scheduler_args("heartbeat-prompt", "", identity, Some("--compact")),
            )
            .await?;
        let task_body = required_string(&packet, "heartbeat-prompt", "task_body")?;
        Ok(compose_heartbeat_prompt(&task_body))
    }
}

/// Wrap LoopX's generated contract with the BitFun host preamble.
fn compose_heartbeat_prompt(task_body: &str) -> String {
    format!("{HEARTBEAT_HOST_PREAMBLE}\n\n{task_body}")
}

fn scheduler_args(
    command: &str,
    subcommand: &str,
    identity: &ControlIdentity,
    prompt_mode: Option<&str>,
) -> Vec<String> {
    let mut args = vec![command.to_string()];
    if !subcommand.is_empty() {
        args.push(subcommand.to_string());
    }
    args.extend([
        "--goal-id".to_string(),
        identity.goal_id.clone(),
        "--agent-id".to_string(),
        identity.agent_id.clone(),
        "--host-surface".to_string(),
        "local_scheduler".to_string(),
        "--scheduler-owner".to_string(),
        "host_automation".to_string(),
        "--execution-mode".to_string(),
        "hosted_automation".to_string(),
    ]);
    for capability in HEARTBEAT_CAPABILITIES {
        args.push("--available-capability".to_string());
        args.push(capability.to_string());
    }
    if let Some(mode) = prompt_mode {
        args.push(mode.to_string());
    }
    args
}

fn read_identity(repository_path: &Path) -> Result<ControlIdentity, AutonomousIssueFixError> {
    let path = repository_path.join(".loopx").join("registry.json");
    let bytes = std::fs::read(&path).map_err(|source| AutonomousIssueFixError::RegistryRead {
        path: path.clone(),
        source,
    })?;
    let registry: Value =
        serde_json::from_slice(&bytes).map_err(|source| AutonomousIssueFixError::RegistryJson {
            path: path.clone(),
            source,
        })?;
    let active_goals = registry
        .get("goals")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|goal| goal.get("status").and_then(Value::as_str) == Some("active"))
        .collect::<Vec<_>>();
    if active_goals.len() != 1 {
        return Err(AutonomousIssueFixError::ActiveGoalCount(active_goals.len()));
    }
    let goal = active_goals[0];
    let goal_id = goal
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(AutonomousIssueFixError::MissingField {
            command: "registry",
            field: "goals[].id",
        })?
        .to_string();
    let agents = goal
        .pointer("/coordination/registered_agents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if agents.len() != 1 {
        return Err(AutonomousIssueFixError::RegisteredAgentCount {
            goal_id,
            count: agents.len(),
        });
    }
    Ok(ControlIdentity {
        goal_id,
        agent_id: agents[0].to_string(),
    })
}

fn project_issue_todo(todo: &Value) -> Option<AutonomousIssueTodo> {
    if todo.get("role").and_then(Value::as_str) != Some("agent") {
        return None;
    }
    let action_kind = todo.get("action_kind").and_then(Value::as_str)?;
    if action_kind != ISSUE_INTAKE_ACTION {
        return None;
    }
    let text = todo.get("text").and_then(Value::as_str)?;
    let (issue_url, issue_ref) = explicit_issue_url(text)?;
    Some(AutonomousIssueTodo {
        issue_ref,
        issue_url,
        todo_id: todo.get("todo_id")?.as_str()?.to_string(),
        status: todo
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("open")
            .to_string(),
        selected: false,
    })
}

/// Project the first open Issue-Fix user gate straight from the todo list.
///
/// `quota should-run` also previews gates, but its `gate_open_items` lane is
/// compacted to two entries; enumerating the todo list is the only complete
/// source, and it keeps gate projection available to the quota-free poll path.
///
/// Issue-linked gates (unblocking a managed intake todo) win, but an open gate
/// must never be invisible — the Kernel blocks on it either way — so unlinked
/// or goal-wide gates are surfaced as a fallback. Only `user_gate` todos ever
/// project; other user todos (reading queues, `user_action`) stay out.
fn project_user_question(
    todos: &[Value],
    issues: &[AutonomousIssueTodo],
) -> Option<AutonomousUserQuestion> {
    let open_gates = todos
        .iter()
        .filter(|todo| {
            todo.get("role").and_then(Value::as_str) == Some("user")
                && todo.get("status").and_then(Value::as_str) == Some("open")
                && todo.get("task_class").and_then(Value::as_str) == Some("user_gate")
        })
        .collect::<Vec<_>>();
    let linked = open_gates.iter().find(|todo| {
        todo.get("unblocks_todo_id")
            .and_then(Value::as_str)
            .is_some_and(|unblocks| issues.iter().any(|issue| issue.todo_id == unblocks))
    });
    let todo = linked.or(open_gates.first())?;
    Some(AutonomousUserQuestion {
        todo_id: todo.get("todo_id")?.as_str()?.to_string(),
        prompt: todo.get("text")?.as_str()?.to_string(),
    })
}

/// Project every open user-lane todo (gates and actions) for the panel's
/// read-only "pending your action" block. Other user todo classes (reading
/// queues, blockers) stay out — they are not actionable from this surface.
fn project_user_todos(todos: &[Value]) -> Vec<AutonomousUserTodo> {
    todos
        .iter()
        .filter_map(|todo| {
            if todo.get("role").and_then(Value::as_str) != Some("user")
                || todo.get("status").and_then(Value::as_str) != Some("open")
            {
                return None;
            }
            let task_class = todo.get("task_class").and_then(Value::as_str)?;
            if task_class != "user_gate" && task_class != "user_action" {
                return None;
            }
            let text = todo.get("text")?.as_str()?.to_string();
            Some(AutonomousUserTodo {
                todo_id: todo.get("todo_id")?.as_str()?.to_string(),
                task_class: task_class.to_string(),
                link: first_http_link(&text),
                text,
            })
        })
        .collect()
}

/// First http(s) URL in a todo text, so the panel can offer a jump link
/// (typically the PR awaiting review or the issue awaiting closure).
fn first_http_link(text: &str) -> Option<String> {
    for token in text.split(|character: char| character.is_whitespace() || character == '(') {
        let url = token.trim_matches(|character: char| {
            matches!(character, ')' | ']' | ',' | '.' | ';' | '`' | '"' | '\'')
        });
        if url.starts_with("https://") || url.starts_with("http://") {
            return Some(url.to_string());
        }
    }
    None
}

fn user_decision_args(
    identity: &ControlIdentity,
    todo_id: &str,
    decision: UserDecision,
    reason: Option<&str>,
) -> Result<Vec<String>, AutonomousIssueFixError> {
    let reason = reason.map(str::trim).filter(|value| !value.is_empty());
    if reason.is_some_and(|value| value.chars().count() > MAX_USER_REASON_CHARS) {
        return Err(AutonomousIssueFixError::InvalidUserResponse(format!(
            "reason must be at most {MAX_USER_REASON_CHARS} characters"
        )));
    }
    let note = reason
        .map(str::to_string)
        .unwrap_or_else(|| "Submitted from the BitFun continuous Issue-Fix panel.".to_string());
    Ok(vec![
        "todo".to_string(),
        "complete".to_string(),
        "--goal-id".to_string(),
        identity.goal_id.clone(),
        "--role".to_string(),
        "user".to_string(),
        "--todo-id".to_string(),
        todo_id.to_string(),
        "--agent-id".to_string(),
        identity.agent_id.clone(),
        "--decision-outcome".to_string(),
        decision.as_loopx_value().to_string(),
        "--note".to_string(),
        note,
    ])
}

fn explicit_issue_url(text: &str) -> Option<(String, String)> {
    for token in text.split(|character: char| character.is_whitespace() || character == '(') {
        let url = token.trim_matches(|character: char| {
            matches!(character, ')' | ']' | ',' | '.' | ';' | ':' | '`')
        });
        let marker = "/issues/";
        let Some(marker_index) = url.find(marker) else {
            continue;
        };
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            continue;
        }
        let suffix = &url[marker_index + marker.len()..];
        let issue_ref = suffix
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
            .next()
            .filter(|value| !value.is_empty())?;
        return Some((url.to_string(), issue_ref.to_string()));
    }
    None
}

fn validate_selection(issue: &IssueSelection) -> Result<(), AutonomousIssueFixError> {
    let issue_ref = issue.issue_ref.trim();
    if issue_ref.is_empty()
        || issue_ref
            .chars()
            .any(|character| !character.is_ascii_alphanumeric() && character != '-')
    {
        return Err(AutonomousIssueFixError::InvalidSelection(format!(
            "unsupported issue ref {:?}",
            issue.issue_ref
        )));
    }
    let Some((_, url_ref)) = explicit_issue_url(&issue.issue_url) else {
        return Err(AutonomousIssueFixError::InvalidSelection(format!(
            "issue URL must be an explicit http(s) /issues/<ref> URL: {}",
            issue.issue_url
        )));
    };
    if url_ref != issue_ref {
        return Err(AutonomousIssueFixError::InvalidSelection(format!(
            "issue ref {} does not match URL ref {}",
            issue_ref, url_ref
        )));
    }
    Ok(())
}

fn task_repository(repo: &str, issue_url: &str) -> Result<String, AutonomousIssueFixError> {
    let host = issue_url
        .split_once("://")
        .and_then(|(_, rest)| rest.split('/').next())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AutonomousIssueFixError::InvalidSelection(format!(
                "cannot determine provider host from {issue_url}"
            ))
        })?;
    let repo = repo.trim().trim_matches('/');
    if repo.split('/').count() < 2 {
        return Err(AutonomousIssueFixError::InvalidSelection(format!(
            "repository identity must be owner/repo: {repo}"
        )));
    }
    Ok(format!("git:{host}/{repo}"))
}

fn issue_todo_text(repo: &str, issue: &IssueSelection) -> String {
    format!(
        "[P0] Advance issue-fix for {repo}#{} ({}): run the canonical LoopX Issue-Fix lifecycle through validated terminal closeout and write every transition back to LoopX.",
        issue.issue_ref, issue.issue_url
    )
}

fn required_string(
    value: &Value,
    command: &'static str,
    field: &'static str,
) -> Result<String, AutonomousIssueFixError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or(AutonomousIssueFixError::MissingField { command, field })
}

fn optional_top_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn optional_string(value: &Value, object: &str, field: &str) -> Option<String> {
    value
        .get(object)
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_only_explicit_issue_fix_todos() {
        let todo = serde_json::json!({
            "role": "agent",
            "action_kind": "issue_fix_intake",
            "text": "[P0] Advance issue-fix for owner/repo#1849 (https://github.com/owner/repo/issues/1849): continue.",
            "todo_id": "todo_1849",
            "status": "open"
        });

        let projected = project_issue_todo(&todo).expect("issue todo projects");
        assert_eq!(projected.issue_ref, "1849");
        assert_eq!(
            projected.issue_url,
            "https://github.com/owner/repo/issues/1849"
        );
        assert_eq!(projected.status, "open");
    }

    #[test]
    fn ignores_issue_todos_owned_by_another_loopx_workflow() {
        let todo = serde_json::json!({
            "role": "agent",
            "action_kind": "issue_fix_portfolio_advancement",
            "text": "Advance https://github.com/owner/repo/issues/1920",
            "todo_id": "todo_1920",
            "status": "open"
        });

        assert!(project_issue_todo(&todo).is_none());
    }

    #[test]
    fn projects_only_a_gate_linked_to_a_managed_issue() {
        let issues = vec![AutonomousIssueTodo {
            issue_ref: "1849".to_string(),
            issue_url: "https://github.com/owner/repo/issues/1849".to_string(),
            todo_id: "todo_1849".to_string(),
            status: "open".to_string(),
            selected: true,
        }];
        let todos = vec![serde_json::json!({
            "todo_id": "gate_1849",
            "role": "user",
            "status": "open",
            "task_class": "user_gate",
            "unblocks_todo_id": "todo_1849",
            "text": "Open the validated pull request for #1849?"
        })];

        let question = project_user_question(&todos, &issues).expect("gate projects");
        assert_eq!(question.todo_id, "gate_1849");
        assert_eq!(
            question.prompt,
            "Open the validated pull request for #1849?"
        );
    }

    #[test]
    fn issue_linked_gates_outrank_goal_wide_gates() {
        let issues = vec![AutonomousIssueTodo {
            issue_ref: "1849".to_string(),
            issue_url: "https://github.com/owner/repo/issues/1849".to_string(),
            todo_id: "todo_1849".to_string(),
            status: "open".to_string(),
            selected: true,
        }];
        let todos = vec![
            serde_json::json!({
                "todo_id": "gate_goal",
                "role": "user",
                "status": "open",
                "task_class": "user_gate",
                "text": "Goal-wide gate without a link"
            }),
            serde_json::json!({
                "todo_id": "gate_1849",
                "role": "user",
                "status": "open",
                "task_class": "user_gate",
                "unblocks_todo_id": "todo_1849",
                "text": "Open the validated pull request for #1849?"
            }),
        ];

        let question = project_user_question(&todos, &issues).expect("gate projects");
        assert_eq!(question.todo_id, "gate_1849");
    }

    #[test]
    fn an_open_gate_is_never_invisible_even_without_an_issue_link() {
        // The Kernel blocks on any open user_gate; hiding it would stall the
        // loop with nothing to answer, so unlinked gates surface as fallback.
        let issues = vec![AutonomousIssueTodo {
            issue_ref: "1849".to_string(),
            issue_url: "https://github.com/owner/repo/issues/1849".to_string(),
            todo_id: "todo_1849".to_string(),
            status: "open".to_string(),
            selected: true,
        }];
        let unlinked = vec![serde_json::json!({
            "todo_id": "gate_force_push",
            "role": "user",
            "status": "open",
            "task_class": "user_gate",
            "unblocks_todo_id": "todo_other",
            "text": "Allow force-push to the feature branch?"
        })];
        let question = project_user_question(&unlinked, &issues).expect("fallback projects");
        assert_eq!(question.todo_id, "gate_force_push");

        // Non-gate user todos (reading queues, user_action) never project.
        let user_action = vec![serde_json::json!({
            "todo_id": "todo_review",
            "role": "user",
            "status": "open",
            "task_class": "user_action",
            "text": "Review the weekly report"
        })];
        assert!(project_user_question(&user_action, &issues).is_none());
    }

    #[test]
    fn gate_projection_does_not_depend_on_quota_preview_truncation() {
        // LoopX compacts `gate_open_items` to two entries; the projection must
        // find a gate that never appears in that preview.
        let issues = vec![AutonomousIssueTodo {
            issue_ref: "3".to_string(),
            issue_url: "https://github.com/owner/repo/issues/3".to_string(),
            todo_id: "todo_3".to_string(),
            status: "open".to_string(),
            selected: false,
        }];
        let todos = vec![
            serde_json::json!({
                "todo_id": "gate_unmanaged",
                "role": "user",
                "status": "open",
                "task_class": "user_gate",
                "unblocks_todo_id": "todo_unmanaged",
                "text": "Gate for a todo this panel does not manage"
            }),
            serde_json::json!({
                "todo_id": "gate_3",
                "role": "user",
                "status": "open",
                "task_class": "user_gate",
                "unblocks_todo_id": "todo_3",
                "text": "Publish the validated patch for #3?"
            }),
        ];

        let question = project_user_question(&todos, &issues).expect("third gate projects");
        assert_eq!(question.todo_id, "gate_3");
    }

    #[test]
    fn user_lane_todos_project_for_the_pending_block() {
        let todos = vec![
            serde_json::json!({
                "todo_id": "todo_review",
                "role": "user",
                "status": "open",
                "task_class": "user_action",
                "text": "[P0] Review and merge PR #2054 (https://github.com/owner/repo/pull/2054)."
            }),
            serde_json::json!({
                "todo_id": "gate_close",
                "role": "user",
                "status": "open",
                "task_class": "user_gate",
                "unblocks_todo_id": "todo_x",
                "text": "Authorize closing issue #2016?"
            }),
            // Excluded: done, agent-lane, and non-actionable classes.
            serde_json::json!({
                "todo_id": "todo_done",
                "role": "user",
                "status": "done",
                "task_class": "user_action",
                "text": "Old action"
            }),
            serde_json::json!({
                "todo_id": "todo_agent",
                "role": "agent",
                "status": "open",
                "task_class": "advancement_task",
                "text": "Agent work"
            }),
            serde_json::json!({
                "todo_id": "todo_reading",
                "role": "user",
                "status": "open",
                "task_class": "blocker",
                "text": "Reading queue entry"
            }),
        ];

        let projected = project_user_todos(&todos);
        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].todo_id, "todo_review");
        assert_eq!(projected[0].task_class, "user_action");
        assert_eq!(
            projected[0].link.as_deref(),
            Some("https://github.com/owner/repo/pull/2054")
        );
        assert_eq!(projected[1].todo_id, "gate_close");
        assert_eq!(projected[1].link, None);
    }

    #[test]
    fn heartbeat_prompt_carries_host_preamble_before_loopx_contract() {
        let prompt = compose_heartbeat_prompt("Advance `goal` using `state`.");
        let preamble_end = prompt
            .find("--- LoopX heartbeat contract follows ---")
            .expect("delimiter present");
        assert!(prompt[..preamble_end].contains("LoopX Kernel state is the ONLY source of truth"));
        assert!(prompt.ends_with("Advance `goal` using `state`."));
    }

    #[test]
    fn scheduler_args_select_prompt_mode_only_for_heartbeat() {
        let identity = ControlIdentity {
            goal_id: "goal".to_string(),
            agent_id: "agent".to_string(),
        };
        let quota = scheduler_args("quota", "should-run", &identity, None);
        assert!(!quota.iter().any(|arg| arg == "--compact" || arg == "--thin"));
        let heartbeat = scheduler_args("heartbeat-prompt", "", &identity, Some("--compact"));
        assert_eq!(heartbeat.last().map(String::as_str), Some("--compact"));
    }

    #[test]
    fn user_decision_is_a_typed_loopx_todo_transition() {
        let identity = ControlIdentity {
            goal_id: "goal".to_string(),
            agent_id: "agent".to_string(),
        };
        let args = user_decision_args(
            &identity,
            "gate_1849",
            UserDecision::Reject,
            Some("Keep the validated patch local."),
        )
        .expect("decision args");

        assert_eq!(
            args,
            vec![
                "todo",
                "complete",
                "--goal-id",
                "goal",
                "--role",
                "user",
                "--todo-id",
                "gate_1849",
                "--agent-id",
                "agent",
                "--decision-outcome",
                "reject",
                "--note",
                "Keep the validated patch local.",
            ]
        );
    }

    #[test]
    fn prose_issue_number_is_not_treated_as_identity() {
        let todo = serde_json::json!({
            "role": "agent",
            "action_kind": "issue_fix_intake",
            "text": "Investigate issue #1849 without an explicit URL.",
            "todo_id": "todo_1849"
        });
        assert!(project_issue_todo(&todo).is_none());
    }

    #[test]
    fn selection_ref_must_match_url() {
        let error = validate_selection(&IssueSelection {
            issue_ref: "1849".to_string(),
            issue_url: "https://github.com/owner/repo/issues/1580".to_string(),
        })
        .expect_err("mismatch must fail");
        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn todo_text_is_stable_for_loopx_deduplication() {
        let issue = IssueSelection {
            issue_ref: "1849".to_string(),
            issue_url: "https://github.com/owner/repo/issues/1849".to_string(),
        };
        assert_eq!(
            issue_todo_text("owner/repo", &issue),
            issue_todo_text("owner/repo", &issue)
        );
    }

    #[test]
    fn identity_requires_one_goal_and_one_agent() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join(".loopx")).expect("registry dir");
        std::fs::write(
            temp.path().join(".loopx").join("registry.json"),
            br#"{"goals":[{"id":"goal","status":"active","coordination":{"registered_agents":["agent"]}}]}"#,
        )
        .expect("registry write");

        let identity = read_identity(temp.path()).expect("identity resolves");
        assert_eq!(identity.goal_id, "goal");
        assert_eq!(identity.agent_id, "agent");
    }
}
