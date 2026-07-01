use super::*;

impl TaskTool {
    pub(super) fn background_subagent_started_assistant_message(session_id: &str) -> String {
        format!(
            "Background subagent started successfully.\nsession_id: \"{}\"\nNote: Its final result will be delivered back automatically to you when it is finished. Avoid polling for status updates. If your current path is blocked on this result and there is no other useful work to do, it is fine to end the current turn.",
            session_id
        )
    }
}
