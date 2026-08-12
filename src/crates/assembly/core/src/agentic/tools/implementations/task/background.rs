use super::*;

impl TaskTool {
    pub(super) fn background_subagent_started_assistant_message(
        agent_id: &str,
        bg_task_id: &str,
    ) -> String {
        format!(
            "Background subagent started successfully.\nagent_id: \"{}\"\nbg_task_id: \"{}\"\nA completion notice will be delivered back to this session automatically when the subagent finishes; use SessionHistory on the subagent session to view the full reply. Use AgentWait with this bg_task_id if you need to block for the result in-band.",
            agent_id, bg_task_id
        )
    }
}
