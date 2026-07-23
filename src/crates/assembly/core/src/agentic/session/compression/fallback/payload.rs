use super::render::render_payload_for_model;
use super::types::{CompressionFallbackOptions, CompressionUnit};
use crate::agentic::core::{
    render_system_reminder, CompressedMessage, CompressedTodoSnapshot, CompressionEntry,
    CompressionPayload, Message,
};

pub(super) fn trim_payload_to_budget(
    entries: Vec<CompressionEntry>,
    options: &CompressionFallbackOptions,
) -> CompressionPayload {
    if entries.is_empty() {
        return CompressionPayload::default();
    }

    let units = flatten_entries_to_units(entries);
    let mut selected_units: Vec<CompressionUnit> = units
        .iter()
        .filter_map(|unit| match unit {
            CompressionUnit::Contract { .. } => Some(unit.clone()),
            _ => None,
        })
        .collect();
    let history_units: Vec<CompressionUnit> = units
        .into_iter()
        .filter(|unit| !matches!(unit, CompressionUnit::Contract { .. }))
        .collect();

    for unit in history_units.into_iter().rev() {
        let mut candidate_units = vec![unit.clone()];
        candidate_units.extend(selected_units.clone());

        let candidate_payload = rebuild_payload_from_units(candidate_units);
        if estimate_payload_tokens(&candidate_payload) <= options.max_tokens {
            let history_insert_index = selected_units
                .iter()
                .take_while(|selected| matches!(selected, CompressionUnit::Contract { .. }))
                .count();
            selected_units.insert(history_insert_index, unit);
        }
    }

    rebuild_payload_from_units(selected_units)
}

fn flatten_entries_to_units(entries: Vec<CompressionEntry>) -> Vec<CompressionUnit> {
    let mut units = Vec::new();

    for (entry_id, entry) in entries.into_iter().enumerate() {
        match entry {
            CompressionEntry::Contract { contract } => {
                units.push(CompressionUnit::Contract { contract });
            }
            CompressionEntry::ModelSummary { text } => {
                units.push(CompressionUnit::ModelSummary { text });
            }
            CompressionEntry::Turn {
                turn_id,
                messages,
                todo,
            } => {
                for message in messages {
                    units.push(CompressionUnit::TurnMessage {
                        entry_id,
                        turn_id: turn_id.clone(),
                        message,
                    });
                }
                if let Some(todo) = todo {
                    units.push(CompressionUnit::TurnTodo {
                        entry_id,
                        turn_id,
                        todo,
                    });
                }
            }
        }
    }

    units
}

fn rebuild_payload_from_units(units: Vec<CompressionUnit>) -> CompressionPayload {
    let mut entries = Vec::new();
    let mut current_turn_entry_id: Option<usize> = None;
    let mut current_turn_id: Option<String> = None;
    let mut current_messages = Vec::new();
    let mut current_todo = None;

    for unit in units {
        match unit {
            CompressionUnit::Contract { contract } => {
                flush_rebuilt_turn(
                    &mut entries,
                    &mut current_turn_entry_id,
                    &mut current_turn_id,
                    &mut current_messages,
                    &mut current_todo,
                );
                entries.push(CompressionEntry::Contract { contract });
            }
            CompressionUnit::ModelSummary { text } => {
                flush_rebuilt_turn(
                    &mut entries,
                    &mut current_turn_entry_id,
                    &mut current_turn_id,
                    &mut current_messages,
                    &mut current_todo,
                );
                entries.push(CompressionEntry::ModelSummary { text });
            }
            CompressionUnit::TurnMessage {
                entry_id,
                turn_id,
                message,
            } => {
                if current_turn_entry_id != Some(entry_id) {
                    flush_rebuilt_turn(
                        &mut entries,
                        &mut current_turn_entry_id,
                        &mut current_turn_id,
                        &mut current_messages,
                        &mut current_todo,
                    );
                    current_turn_entry_id = Some(entry_id);
                    current_turn_id = turn_id;
                }
                current_messages.push(message);
            }
            CompressionUnit::TurnTodo {
                entry_id,
                turn_id,
                todo,
            } => {
                if current_turn_entry_id != Some(entry_id) {
                    flush_rebuilt_turn(
                        &mut entries,
                        &mut current_turn_entry_id,
                        &mut current_turn_id,
                        &mut current_messages,
                        &mut current_todo,
                    );
                    current_turn_entry_id = Some(entry_id);
                    current_turn_id = turn_id;
                }
                current_todo = Some(todo);
            }
        }
    }

    flush_rebuilt_turn(
        &mut entries,
        &mut current_turn_entry_id,
        &mut current_turn_id,
        &mut current_messages,
        &mut current_todo,
    );

    CompressionPayload { entries }
}

fn flush_rebuilt_turn(
    entries: &mut Vec<CompressionEntry>,
    current_turn_entry_id: &mut Option<usize>,
    current_turn_id: &mut Option<String>,
    current_messages: &mut Vec<CompressedMessage>,
    current_todo: &mut Option<CompressedTodoSnapshot>,
) {
    if current_turn_entry_id.is_none() {
        return;
    }

    if current_messages.is_empty() && current_todo.is_none() {
        *current_turn_entry_id = None;
        *current_turn_id = None;
        return;
    }

    entries.push(CompressionEntry::Turn {
        turn_id: current_turn_id.clone(),
        messages: std::mem::take(current_messages),
        todo: current_todo.take(),
    });
    *current_turn_entry_id = None;
    *current_turn_id = None;
}

fn estimate_payload_tokens(payload: &CompressionPayload) -> usize {
    let rendered = render_payload_for_model(payload);
    let mut synthetic_message = Message::user(render_system_reminder(&rendered));
    synthetic_message.get_tokens()
}
