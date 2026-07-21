use super::*;

impl TaskTool {
    pub(super) fn background_subagent_started_assistant_message(
        session_id: &str,
        background_task_id: &str,
    ) -> String {
        format!(
            "Background subagent started successfully.\nsession_id: \"{}\"\nbackground_task_id: \"{}\"\nUse AgentWait with this background_task_id when you need its result. The result will not be delivered automatically.",
            session_id, background_task_id
        )
    }
}
