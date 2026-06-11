//! Session metadata counters and visible-index mutation rules.

use super::types::{DialogTurnData, SessionMetadata, StoredSessionIndexFile};

fn estimate_turn_message_count(turn: &DialogTurnData) -> usize {
    let assistant_text_count: usize = turn
        .model_rounds
        .iter()
        .map(|round| round.text_items.len())
        .sum();
    1 + assistant_text_count
}

pub fn refresh_session_metadata_from_turns(
    metadata: &mut SessionMetadata,
    workspace_path: &str,
    turns: &[DialogTurnData],
    last_active_at: u64,
) {
    metadata.turn_count = turns.len();
    metadata.message_count = turns.iter().map(estimate_turn_message_count).sum();
    metadata.tool_call_count = turns.iter().map(DialogTurnData::count_tool_calls).sum();
    metadata.last_active_at = last_active_at;
    fill_workspace_path_if_missing(metadata, workspace_path);
}

pub fn try_refresh_session_metadata_for_saved_turn(
    metadata: &mut SessionMetadata,
    workspace_path: &str,
    previous_turn: Option<&DialogTurnData>,
    turn: &DialogTurnData,
    last_active_at: u64,
) -> bool {
    let new_message_count = estimate_turn_message_count(turn);
    let new_tool_call_count = turn.count_tool_calls();

    match previous_turn {
        Some(previous)
            if previous.session_id == turn.session_id
                && previous.turn_index == turn.turn_index
                && turn.turn_index < metadata.turn_count =>
        {
            metadata.message_count = metadata
                .message_count
                .saturating_sub(estimate_turn_message_count(previous))
                .saturating_add(new_message_count);
            metadata.tool_call_count = metadata
                .tool_call_count
                .saturating_sub(previous.count_tool_calls())
                .saturating_add(new_tool_call_count);
        }
        None if turn.turn_index == metadata.turn_count => {
            metadata.turn_count += 1;
            metadata.message_count = metadata.message_count.saturating_add(new_message_count);
            metadata.tool_call_count = metadata.tool_call_count.saturating_add(new_tool_call_count);
        }
        _ => return false,
    }

    metadata.last_active_at = last_active_at;
    fill_workspace_path_if_missing(metadata, workspace_path);
    true
}

pub fn build_session_index_snapshot(
    metadata_list: Vec<SessionMetadata>,
    updated_at: u64,
) -> (StoredSessionIndexFile, Vec<SessionMetadata>) {
    let metadata_file_count = metadata_list.len();
    let mut visible_sessions = metadata_list
        .into_iter()
        .filter(|metadata| !metadata.should_hide_from_user_lists())
        .collect::<Vec<_>>();
    visible_sessions.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));

    let index = StoredSessionIndexFile::with_metadata_file_count(
        updated_at,
        visible_sessions.clone(),
        metadata_file_count,
    );
    (index, visible_sessions)
}

pub fn upsert_session_index_entry(
    existing_index: Option<StoredSessionIndexFile>,
    metadata: &SessionMetadata,
    metadata_file_created: bool,
    disk_metadata_file_count: usize,
    updated_at: u64,
) -> StoredSessionIndexFile {
    let had_index = existing_index.is_some();
    let mut index = existing_index.unwrap_or_else(|| StoredSessionIndexFile {
        schema_version: super::types::SESSION_STORAGE_SCHEMA_VERSION,
        updated_at: 0,
        metadata_file_count: disk_metadata_file_count,
        sessions: Vec::new(),
    });

    if let Some(existing) = index
        .sessions
        .iter_mut()
        .find(|value| value.session_id == metadata.session_id)
    {
        *existing = metadata.clone();
    } else {
        index.sessions.push(metadata.clone());
    }

    index
        .sessions
        .sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));
    if had_index && metadata_file_created {
        index.metadata_file_count = index.metadata_file_count.saturating_add(1);
    }
    index.updated_at = updated_at;
    index.schema_version = super::types::SESSION_STORAGE_SCHEMA_VERSION;
    index
}

pub fn remove_session_index_entry(
    existing_index: Option<StoredSessionIndexFile>,
    session_id: &str,
    metadata_file_count_delta: isize,
    updated_at: u64,
) -> Option<StoredSessionIndexFile> {
    let mut index = existing_index?;

    index
        .sessions
        .retain(|value| value.session_id != session_id);
    if metadata_file_count_delta > 0 {
        index.metadata_file_count = index
            .metadata_file_count
            .saturating_add(metadata_file_count_delta as usize);
    } else if metadata_file_count_delta < 0 {
        index.metadata_file_count = index
            .metadata_file_count
            .saturating_sub(metadata_file_count_delta.unsigned_abs());
    }
    index.updated_at = updated_at;
    index.schema_version = super::types::SESSION_STORAGE_SCHEMA_VERSION;
    Some(index)
}

fn fill_workspace_path_if_missing(metadata: &mut SessionMetadata, workspace_path: &str) {
    if metadata.workspace_path.is_none() {
        metadata.workspace_path = Some(workspace_path.to_string());
    }
}
