use crate::service::session::{DialogTurnData, DialogTurnKind};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) const SESSION_REVERT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionRevertPhase {
    Applying,
    Staged,
    Clearing,
    Committing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionWorkspaceCheckpoint {
    pub(crate) path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) snapshot_id: Option<String>,
}

impl SessionWorkspaceCheckpoint {
    pub(crate) fn display_path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionRevertState {
    pub(crate) schema_version: u32,
    pub(crate) boundary_turn: usize,
    pub(crate) original_turn_end: usize,
    pub(crate) phase: SessionRevertPhase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) workspace_checkpoint: Vec<SessionWorkspaceCheckpoint>,
}

impl SessionRevertState {
    fn initial(boundary_turn: usize, original_turn_end: usize) -> Self {
        Self {
            schema_version: SESSION_REVERT_SCHEMA_VERSION,
            boundary_turn,
            original_turn_end,
            phase: SessionRevertPhase::Applying,
            workspace_checkpoint: Vec::new(),
        }
    }

    pub(crate) fn diagnostic_mutation_id(&self, session_id: &str) -> String {
        format!(
            "session-revert:{session_id}:{}:{}",
            self.boundary_turn, self.original_turn_end
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionRevertTransition {
    Stage {
        state: SessionRevertState,
        replacement_prompt: Option<String>,
        hidden_turn_count: usize,
    },
    Clear {
        previous_state: SessionRevertState,
    },
}

#[cfg(test)]
impl SessionRevertTransition {
    pub(crate) fn boundary_turn(&self) -> Option<usize> {
        match self {
            Self::Stage { state, .. } => Some(state.boundary_turn),
            Self::Clear { .. } => None,
        }
    }

    pub(crate) fn replacement_prompt(&self) -> Option<&str> {
        match self {
            Self::Stage {
                replacement_prompt, ..
            } => replacement_prompt.as_deref(),
            Self::Clear { .. } => None,
        }
    }

    pub(crate) fn hidden_turn_count(&self) -> usize {
        match self {
            Self::Stage {
                hidden_turn_count, ..
            } => *hidden_turn_count,
            Self::Clear { .. } => 0,
        }
    }

    pub(crate) fn staged_state(&self) -> Option<SessionRevertState> {
        match self {
            Self::Stage { state, .. } => Some(state.clone()),
            Self::Clear { .. } => None,
        }
    }
}

pub(crate) fn resolve_undo(
    turns: &[DialogTurnData],
    current: Option<&SessionRevertState>,
) -> Option<SessionRevertTransition> {
    let current_boundary = current
        .map(|state| state.boundary_turn)
        .unwrap_or_else(|| original_turn_end(turns));
    let target = turns
        .iter()
        .filter(|turn| turn.kind == DialogTurnKind::UserDialog)
        .filter(|turn| turn.turn_index < current_boundary)
        .max_by_key(|turn| turn.turn_index)?;

    let mut state = current.cloned().unwrap_or_else(|| {
        SessionRevertState::initial(target.turn_index, original_turn_end(turns))
    });
    state.boundary_turn = target.turn_index;
    state.phase = SessionRevertPhase::Applying;
    let hidden_turn_count = turns
        .iter()
        .filter(|turn| turn.turn_index >= state.boundary_turn)
        .count();

    Some(SessionRevertTransition::Stage {
        state,
        replacement_prompt: Some(
            target
                .user_message
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("original_text"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&target.user_message.content)
                .to_string(),
        ),
        hidden_turn_count,
    })
}

pub(crate) fn resolve_targeted(
    turns: &[DialogTurnData],
    target_turn_id: &str,
    current: Option<&SessionRevertState>,
) -> Option<SessionRevertTransition> {
    let target = turns
        .iter()
        .find(|turn| turn.kind == DialogTurnKind::UserDialog && turn.turn_id == target_turn_id)?;
    let mut state = current.cloned().unwrap_or_else(|| {
        SessionRevertState::initial(target.turn_index, original_turn_end(turns))
    });
    state.boundary_turn = target.turn_index;
    state.phase = SessionRevertPhase::Applying;
    let hidden_turn_count = turns
        .iter()
        .filter(|turn| turn.turn_index >= state.boundary_turn)
        .count();

    Some(SessionRevertTransition::Stage {
        state,
        replacement_prompt: Some(
            target
                .user_message
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("original_text"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&target.user_message.content)
                .to_string(),
        ),
        hidden_turn_count,
    })
}

pub(crate) fn resolve_redo(
    turns: &[DialogTurnData],
    current: Option<&SessionRevertState>,
) -> Option<SessionRevertTransition> {
    let current = current?;
    let next = turns
        .iter()
        .filter(|turn| turn.kind == DialogTurnKind::UserDialog)
        .filter(|turn| {
            turn.turn_index > current.boundary_turn && turn.turn_index < current.original_turn_end
        })
        .min_by_key(|turn| turn.turn_index);

    if let Some(next) = next {
        let mut state = current.clone();
        state.boundary_turn = next.turn_index;
        state.phase = SessionRevertPhase::Applying;
        let hidden_turn_count = turns
            .iter()
            .filter(|turn| turn.turn_index >= state.boundary_turn)
            .count();
        return Some(SessionRevertTransition::Stage {
            state,
            replacement_prompt: None,
            hidden_turn_count,
        });
    }

    Some(SessionRevertTransition::Clear {
        previous_state: current.clone(),
    })
}

fn original_turn_end(turns: &[DialogTurnData]) -> usize {
    turns
        .iter()
        .map(|turn| turn.turn_index.saturating_add(1))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_redo, resolve_targeted, resolve_undo, SessionRevertPhase, SessionRevertState,
        SessionRevertTransition, SessionWorkspaceCheckpoint, SESSION_REVERT_SCHEMA_VERSION,
    };
    use crate::service::session::{DialogTurnData, DialogTurnKind, TurnStatus, UserMessageData};

    fn user_turn(index: usize, prompt: &str) -> DialogTurnData {
        DialogTurnData {
            turn_id: format!("turn-{index}"),
            turn_index: index,
            session_id: "session-1".to_string(),
            timestamp: index as u64,
            kind: DialogTurnKind::UserDialog,
            agent_type: Some("agentic".to_string()),
            user_message: UserMessageData {
                id: format!("message-{index}"),
                content: prompt.to_string(),
                timestamp: index as u64,
                metadata: None,
            },
            model_rounds: Vec::new(),
            start_time: index as u64,
            end_time: Some(index as u64),
            duration_ms: Some(0),
            token_usage: None,
            finish_reason: None,
            has_final_response: Some(true),
            error: None,
            error_detail: None,
            recovery: None,
            recovery_epoch: None,
            status: TurnStatus::Completed,
            todos: None,
        }
    }

    fn maintenance_turn(index: usize) -> DialogTurnData {
        let mut turn = user_turn(index, "internal compact");
        turn.kind = DialogTurnKind::ManualCompaction;
        turn.agent_type = None;
        turn
    }

    #[test]
    fn undo_walks_visible_user_prompts_and_restores_each_prompt() {
        let turns = vec![user_turn(0, "first"), maintenance_turn(1), {
            let mut turn = user_turn(2, "wrapped second prompt");
            turn.user_message.metadata = Some(serde_json::json!({
                "original_text": "second"
            }));
            turn
        }];

        let first = resolve_undo(&turns, None).expect("latest prompt should be undoable");
        assert_eq!(first.boundary_turn(), Some(2));
        assert_eq!(first.replacement_prompt(), Some("second"));
        assert_eq!(first.hidden_turn_count(), 1);

        let state = first.staged_state().expect("undo should stage a boundary");
        let second = resolve_undo(&turns, Some(&state)).expect("earlier prompt should be undoable");
        assert_eq!(second.boundary_turn(), Some(0));
        assert_eq!(second.replacement_prompt(), Some("first"));
        assert_eq!(second.hidden_turn_count(), 3);
        assert!(resolve_undo(&turns, second.staged_state().as_ref()).is_none());
    }

    #[test]
    fn redo_advances_to_the_next_user_prompt_then_clears_the_stage() {
        let turns = vec![
            user_turn(0, "first"),
            maintenance_turn(1),
            user_turn(2, "second"),
        ];
        let state = resolve_undo(&turns, None)
            .and_then(|transition| transition.staged_state())
            .expect("undo should stage latest prompt");
        let earliest = resolve_undo(&turns, Some(&state))
            .and_then(|transition| transition.staged_state())
            .expect("second undo should stage first prompt");

        let advance = resolve_redo(&turns, Some(&earliest)).expect("redo should advance");
        assert_eq!(advance.boundary_turn(), Some(2));
        assert!(advance.replacement_prompt().is_none());
        assert_eq!(advance.hidden_turn_count(), 1);

        let clear = resolve_redo(&turns, advance.staged_state().as_ref())
            .expect("final redo should clear the stage");
        assert!(matches!(clear, SessionRevertTransition::Clear { .. }));
        assert_eq!(clear.hidden_turn_count(), 0);
        assert!(resolve_redo(&turns, None).is_none());
    }

    #[test]
    fn targeted_resolution_uses_turn_identity_with_sparse_storage_indices() {
        let turns = vec![
            user_turn(0, "first"),
            user_turn(1, "second"),
            maintenance_turn(7),
            user_turn(8, "third"),
        ];
        let transition = resolve_targeted(&turns, "turn-8", None).expect("target should resolve");
        assert_eq!(transition.boundary_turn(), Some(8));
        assert_eq!(transition.hidden_turn_count(), 1);
        assert_eq!(transition.replacement_prompt(), Some("third"));
        assert!(resolve_targeted(&turns, "missing", None).is_none());
        assert!(resolve_targeted(&turns, "turn-7", None).is_none());
    }

    #[test]
    fn targeted_resolution_extends_an_existing_staged_revert() {
        let turns = vec![user_turn(0, "first"), user_turn(1, "second")];
        let staged = SessionRevertState {
            schema_version: SESSION_REVERT_SCHEMA_VERSION,
            boundary_turn: 1,
            original_turn_end: 2,
            phase: SessionRevertPhase::Staged,
            workspace_checkpoint: vec![SessionWorkspaceCheckpoint {
                path: "src/lib.rs".into(),
                snapshot_id: Some("snapshot-1".to_string()),
            }],
        };

        let transition = resolve_targeted(&turns, "turn-0", Some(&staged))
            .expect("visible target should extend the staged revert");
        let state = transition
            .staged_state()
            .expect("target should stage a boundary");
        assert_eq!(state.boundary_turn, 0);
        assert_eq!(state.original_turn_end, 2);
        assert_eq!(state.workspace_checkpoint, staged.workspace_checkpoint);
    }
}
