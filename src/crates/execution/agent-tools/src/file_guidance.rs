//! Provider-neutral guidance markers for file tool guardrails.

pub const FILE_TOOL_GUIDANCE_PREFIX: &str = "[guidance] ";

pub fn file_tool_guidance_message(message: impl Into<String>) -> String {
    format!("{FILE_TOOL_GUIDANCE_PREFIX}{}", message.into())
}

pub fn is_file_tool_guidance_message(message: &str) -> bool {
    message.starts_with(FILE_TOOL_GUIDANCE_PREFIX)
}
