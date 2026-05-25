//! Shared guidance markers for file Write/Edit tool guardrails shown to users as hints.

pub const FILE_TOOL_GUIDANCE_PREFIX: &str = "[guidance] ";

pub fn file_tool_guidance_message(message: impl Into<String>) -> String {
    format!("{FILE_TOOL_GUIDANCE_PREFIX}{}", message.into())
}

pub fn is_file_tool_guidance_message(message: &str) -> bool {
    message.starts_with(FILE_TOOL_GUIDANCE_PREFIX)
}
