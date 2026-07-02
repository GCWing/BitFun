use crate::agentic::core::strip_prompt_markup;
use crate::service::session::{DialogTurnData, SessionTranscriptExportOptions, ToolItemData};

#[derive(Debug, Clone)]
pub(crate) struct TranscriptTextBlock {
    pub(crate) round_index: usize,
    pub(crate) content: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TranscriptToolBlock {
    pub(crate) tool_name: String,
    pub(crate) tool_input: Option<String>,
    pub(crate) result: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum TranscriptRoundBlock {
    Thinking(String),
    Assistant(String),
    Tool(TranscriptToolBlock),
}

#[derive(Debug, Clone)]
pub(crate) struct TranscriptRoundData {
    pub(crate) round_index: usize,
    pub(crate) blocks: Vec<TranscriptRoundBlock>,
}

pub(crate) fn transcript_text_lines(content: &str) -> Vec<String> {
    if content.is_empty() {
        return vec!["(empty)".to_string()];
    }

    let lines = content
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec!["(empty)".to_string()]
    } else {
        lines
    }
}

pub(crate) fn transcript_value_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

pub(crate) fn transcript_tool_input(item: &ToolItemData, tool_inputs: bool) -> Option<String> {
    if !tool_inputs || item.tool_call.input.is_null() {
        return None;
    }

    Some(transcript_value_string(&item.tool_call.input))
}

pub(crate) fn transcript_tool_result(item: &ToolItemData) -> Option<String> {
    item.tool_result.as_ref().and_then(|result| {
        result
            .result_for_assistant
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                if result.result.is_null() {
                    None
                } else {
                    Some(transcript_value_string(&result.result))
                }
            })
    })
}

pub(crate) fn transcript_display_user_content(turn: &DialogTurnData) -> String {
    turn.user_message
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("original_text"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| strip_prompt_markup(&turn.user_message.content))
}

pub(crate) fn transcript_assistant_blocks(turn: &DialogTurnData) -> Vec<TranscriptTextBlock> {
    turn.model_rounds
        .iter()
        .filter_map(|round| {
            let content = round
                .text_items
                .iter()
                .filter(|item| !item.is_subagent_item.unwrap_or(false))
                .map(|item| item.content.trim())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            if content.is_empty() {
                None
            } else {
                Some(TranscriptTextBlock {
                    round_index: round.round_index,
                    content,
                })
            }
        })
        .collect()
}

pub(crate) fn transcript_thinking_blocks(turn: &DialogTurnData) -> Vec<TranscriptTextBlock> {
    turn.model_rounds
        .iter()
        .filter_map(|round| {
            let content = round
                .thinking_items
                .iter()
                .filter(|item| !item.is_subagent_item.unwrap_or(false))
                .map(|item| item.content.trim())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            if content.is_empty() {
                None
            } else {
                Some(TranscriptTextBlock {
                    round_index: round.round_index,
                    content,
                })
            }
        })
        .collect()
}

pub(crate) fn transcript_tool_blocks(
    turn: &DialogTurnData,
    tool_inputs: bool,
) -> Vec<TranscriptToolBlock> {
    turn.model_rounds
        .iter()
        .flat_map(|round| round.tool_items.iter())
        .filter(|item| !item.is_subagent_item.unwrap_or(false))
        .map(|item| TranscriptToolBlock {
            tool_name: item.tool_name.clone(),
            tool_input: transcript_tool_input(item, tool_inputs),
            result: transcript_tool_result(item),
        })
        .collect()
}

pub(crate) fn transcript_round_blocks(
    turn: &DialogTurnData,
    options: &SessionTranscriptExportOptions,
) -> Vec<TranscriptRoundData> {
    turn.model_rounds
        .iter()
        .filter_map(|round| {
            let thinking_content = if options.thinking {
                round
                    .thinking_items
                    .iter()
                    .filter(|item| !item.is_subagent_item.unwrap_or(false))
                    .map(|item| item.content.trim())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n")
            } else {
                String::new()
            };

            let assistant_content = round
                .text_items
                .iter()
                .filter(|item| !item.is_subagent_item.unwrap_or(false))
                .map(|item| item.content.trim())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");

            let tool_blocks = if options.tools {
                round
                    .tool_items
                    .iter()
                    .filter(|item| !item.is_subagent_item.unwrap_or(false))
                    .map(|item| TranscriptToolBlock {
                        tool_name: item.tool_name.clone(),
                        tool_input: transcript_tool_input(item, options.tool_inputs),
                        result: transcript_tool_result(item),
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            if thinking_content.is_empty() && assistant_content.is_empty() && tool_blocks.is_empty()
            {
                return None;
            }

            let mut blocks = Vec::new();
            if !thinking_content.is_empty() {
                blocks.push(TranscriptRoundBlock::Thinking(thinking_content));
            }
            if !assistant_content.is_empty() {
                blocks.push(TranscriptRoundBlock::Assistant(assistant_content));
            }
            for tool in tool_blocks {
                blocks.push(TranscriptRoundBlock::Tool(tool));
            }

            Some(TranscriptRoundData {
                round_index: round.round_index,
                blocks,
            })
        })
        .collect()
}
