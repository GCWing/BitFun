use super::{Message, MessageContent, MessageRole};
use crate::util::types::Message as AIMessage;
use log::warn;
pub struct MessageHelper;

impl MessageHelper {
    pub fn compute_keep_thinking_flags(
        messages: &mut Vec<Message>,
        enable_thinking: bool,
        support_preserved_thinking: bool,
    ) {
        if messages.is_empty() {
            return;
        }
        if !enable_thinking {
            messages.iter_mut().for_each(|m| {
                if m.metadata.keep_thinking {
                    m.metadata.keep_thinking = false;
                    m.metadata.tokens = None;
                }
            });
        } else if support_preserved_thinking {
            messages.iter_mut().for_each(|m| {
                if !m.metadata.keep_thinking {
                    m.metadata.keep_thinking = true;
                    m.metadata.tokens = None;
                }
            });
        } else {
            let last_message_turn_id = messages.last().and_then(|m| m.metadata.turn_id.clone());
            if let Some(last_turn_id) = last_message_turn_id {
                messages.iter_mut().for_each(|m| {
                    let keep_thinking = m
                        .metadata
                        .turn_id
                        .as_ref()
                        .is_some_and(|cur_turn_id| cur_turn_id == &last_turn_id);
                    if m.metadata.keep_thinking != keep_thinking {
                        m.metadata.keep_thinking = keep_thinking;
                        m.metadata.tokens = None;
                    }
                })
            } else {
                // Find the index of the last user message (role is user and not <system-reminder>) from back to front
                let last_user_message_index =
                    messages.iter().rposition(|m| m.is_actual_user_message());
                if let Some(last_user_message_index) = last_user_message_index {
                    // Messages from the last user message onwards are messages for this turn
                    messages.iter_mut().enumerate().for_each(|(index, m)| {
                        let keep_thinking = index >= last_user_message_index;
                        if m.metadata.keep_thinking != keep_thinking {
                            m.metadata.keep_thinking = keep_thinking;
                            m.metadata.tokens = None;
                        }
                    })
                } else {
                    // No user message found, should not reach here in practice
                    warn!("compute_keep_thinking_flags: no user message found");

                    messages.iter_mut().for_each(|m| {
                        if m.metadata.keep_thinking {
                            m.metadata.keep_thinking = false;
                            m.metadata.tokens = None;
                        }
                    });
                }
            }
        }
    }

    pub fn convert_messages(messages: &[Message]) -> Vec<AIMessage> {
        messages.iter().map(|m| AIMessage::from(m)).collect()
    }

    pub fn group_messages_by_turns(mut messages: Vec<Message>) -> Vec<Vec<Message>> {
        let mut turns = Vec::new();
        if messages.is_empty() {
            return turns;
        }
        let mut turn = Vec::new();
        // Regardless of whether the first message is a user message, treat it as the start of a turn
        let remaining_messages = messages.split_off(1);
        turn.push(messages.remove(0));
        // Skip the first message
        for message in remaining_messages {
            if message.is_actual_user_message() {
                turns.push(turn);
                turn = Vec::new();
            }
            turn.push(message);
        }
        turns.push(turn);
        turns
    }

    /// Split messages at a middle assistant, return two message lists
    /// If cannot split at assistant, split at middle message
    pub fn split_messages_in_middle(
        mut messages: Vec<Message>,
    ) -> Option<(Vec<Message>, Vec<Message>)> {
        let messages_tokens: Vec<usize> = messages.iter_mut().map(|m| m.get_tokens()).collect();
        let total_tokens = messages_tokens.iter().sum::<usize>();
        let half_tokens = total_tokens / 2;
        let mut sum = 0usize;
        let mut mid_assistant_msg_idx = None;
        let mut mid_idx = None;
        let (mut min_delta0, mut min_delta1) = (total_tokens, total_tokens);
        for (idx, (message, tokens)) in messages.iter().zip(messages_tokens.iter()).enumerate() {
            let delta = sum.abs_diff(half_tokens);
            if delta < min_delta1 {
                min_delta1 = delta;
                mid_idx = Some(idx);
            }

            if message.role == MessageRole::Assistant {
                if delta < min_delta0 {
                    min_delta0 = delta;
                    mid_assistant_msg_idx = Some(idx);
                }
            }

            // Delta will only get larger going forward, so can exit early
            if sum > half_tokens && mid_assistant_msg_idx.is_some() && mid_idx.is_some() {
                break;
            }

            // Accumulate current message's token count
            sum += tokens;
        }
        let split_at = mid_assistant_msg_idx.or(mid_idx);
        if let Some(split_at) = split_at {
            let remaining_messages = messages.split_off(split_at);
            Some((messages, remaining_messages))
        } else {
            None
        }
    }

    pub fn get_last_todo(messages: &[Message]) -> Option<String> {
        for message in messages.iter().rev() {
            if message.role == MessageRole::Assistant {
                match &message.content {
                    MessageContent::Mixed { tool_calls, .. } => {
                        if tool_calls.is_empty() {
                            continue;
                        }
                        for tool_call in tool_calls.iter().rev() {
                            if tool_call.tool_name == "TodoWrite" {
                                let todos = tool_call.arguments.get("todos").unwrap_or_default();
                                return Some(todos.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }
}
