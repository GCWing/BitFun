use super::{CompressedTodoItem, CompressedTodoSnapshot, Message, MessageContent, MessageRole};
use crate::util::token_counter::TokenCounter;
use crate::util::types::Message as AIMessage;
use crate::util::types::ToolDefinition;
pub struct MessageHelper;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestReasoningTokenPolicy {
    FullHistory,
    LatestTurnOnly,
    SkipAll,
}

impl MessageHelper {
    pub fn convert_messages(messages: &[Message]) -> Vec<AIMessage> {
        messages.iter().map(AIMessage::from).collect()
    }

    pub fn estimate_request_tokens(
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        reasoning_policy: RequestReasoningTokenPolicy,
    ) -> usize {
        let reasoning_frontier_start = match reasoning_policy {
            RequestReasoningTokenPolicy::FullHistory => Some(0),
            RequestReasoningTokenPolicy::LatestTurnOnly => {
                Some(Self::find_reasoning_frontier_start(messages))
            }
            RequestReasoningTokenPolicy::SkipAll => None,
        };

        let mut total = messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let include_reasoning =
                    reasoning_frontier_start.is_some_and(|frontier_start| index >= frontier_start);
                message.estimate_tokens_with_reasoning(include_reasoning)
            })
            .sum::<usize>();

        total += 3;

        if let Some(tool_defs) = tools {
            total += TokenCounter::estimate_tool_definitions_tokens(tool_defs);
        }

        total
    }

    fn find_reasoning_frontier_start(messages: &[Message]) -> usize {
        if messages.is_empty() {
            return 0;
        }

        if let Some(last_turn_id) = messages.last().and_then(|m| m.metadata.turn_id.as_deref()) {
            if let Some(frontier_start) = messages
                .iter()
                .position(|m| m.metadata.turn_id.as_deref() == Some(last_turn_id))
            {
                return frontier_start;
            }
        }

        messages
            .iter()
            .rposition(Message::is_actual_user_message)
            .unwrap_or(messages.len().saturating_sub(1))
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

            if message.role == MessageRole::Assistant && delta < min_delta0 {
                min_delta0 = delta;
                mid_assistant_msg_idx = Some(idx);
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

    pub fn get_last_todo_snapshot(messages: &[Message]) -> Option<CompressedTodoSnapshot> {
        for message in messages.iter().rev() {
            if message.role == MessageRole::Assistant {
                let MessageContent::Mixed { tool_calls, .. } = &message.content else {
                    continue;
                };
                if tool_calls.is_empty() {
                    continue;
                }
                for tool_call in tool_calls.iter().rev() {
                    if tool_call.tool_name != "TodoWrite" {
                        continue;
                    }

                    let todos = tool_call.arguments.get("todos")?.as_array()?;
                    let mut compressed_todos = Vec::new();

                    for todo in todos {
                        let Some(todo_object) = todo.as_object() else {
                            continue;
                        };
                        let Some(content) = todo_object
                            .get("content")
                            .and_then(serde_json::Value::as_str)
                            .map(str::trim)
                            .filter(|content| !content.is_empty())
                        else {
                            continue;
                        };

                        let status = todo_object
                            .get("status")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("pending");
                        let id = todo_object
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string);

                        compressed_todos.push(CompressedTodoItem {
                            id,
                            content: content.to_string(),
                            status: status.to_string(),
                        });
                    }

                    if compressed_todos.is_empty() {
                        continue;
                    }

                    return Some(CompressedTodoSnapshot {
                        todos: compressed_todos,
                        summary: None,
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageHelper, RequestReasoningTokenPolicy};
    use crate::agentic::core::Message;
    use crate::util::token_counter::TokenCounter;

    #[test]
    fn latest_turn_reasoning_policy_uses_turn_id_boundary() {
        let messages = vec![
            Message::user("old user".to_string()).with_turn_id("turn-1".to_string()),
            Message::assistant_with_reasoning(
                Some("old reasoning".to_string()),
                "old answer".to_string(),
                Vec::new(),
            )
            .with_turn_id("turn-1".to_string()),
            Message::user("new user".to_string()).with_turn_id("turn-2".to_string()),
            Message::assistant_with_reasoning(
                Some("new reasoning".to_string()),
                "new answer".to_string(),
                Vec::new(),
            )
            .with_turn_id("turn-2".to_string()),
        ];

        let full = MessageHelper::estimate_request_tokens(
            &messages,
            None,
            RequestReasoningTokenPolicy::FullHistory,
        );
        let latest = MessageHelper::estimate_request_tokens(
            &messages,
            None,
            RequestReasoningTokenPolicy::LatestTurnOnly,
        );
        let skip_all = MessageHelper::estimate_request_tokens(
            &messages,
            None,
            RequestReasoningTokenPolicy::SkipAll,
        );

        assert_eq!(
            full - latest,
            TokenCounter::estimate_tokens("old reasoning")
        );
        assert_eq!(
            latest - skip_all,
            TokenCounter::estimate_tokens("new reasoning")
        );
    }

    #[test]
    fn latest_turn_reasoning_policy_falls_back_to_last_actual_user_message() {
        let messages = vec![
            Message::user("old user".to_string()),
            Message::assistant_with_reasoning(
                Some("old reasoning".to_string()),
                "old answer".to_string(),
                Vec::new(),
            ),
            Message::user("new user".to_string()),
            Message::assistant_with_reasoning(
                Some("new reasoning".to_string()),
                "new answer".to_string(),
                Vec::new(),
            ),
        ];

        let latest = MessageHelper::estimate_request_tokens(
            &messages,
            None,
            RequestReasoningTokenPolicy::LatestTurnOnly,
        );
        let skip_all = MessageHelper::estimate_request_tokens(
            &messages,
            None,
            RequestReasoningTokenPolicy::SkipAll,
        );

        assert_eq!(
            latest - skip_all,
            TokenCounter::estimate_tokens("new reasoning")
        );
    }
}
