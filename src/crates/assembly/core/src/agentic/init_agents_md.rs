use crate::agentic::agents::get_embedded_prompt;
use crate::agentic::core::{InternalReminderKind, Message};
use crate::service::config::get_app_language_code;
use crate::util::errors::{BitFunError, BitFunResult};

const INIT_AGENTS_MD_PROMPT_NAME: &str = "init_agents_md";

fn init_agents_md_user_query(is_chinese: bool) -> &'static str {
    if is_chinese {
        "请根据当前项目内容生成或更新 AGENTS.md"
    } else {
        "Please generate or update AGENTS.md so it matches the current project"
    }
}

pub(crate) async fn build_init_agents_md_user_input() -> BitFunResult<(String, Vec<Message>)> {
    let prompt = get_embedded_prompt(INIT_AGENTS_MD_PROMPT_NAME).ok_or_else(|| {
        BitFunError::Agent(format!(
            "{} not found in embedded files",
            INIT_AGENTS_MD_PROMPT_NAME
        ))
    })?;
    let is_chinese = get_app_language_code().await.starts_with("zh");
    let user_query = init_agents_md_user_query(is_chinese).to_string();
    Ok((
        user_query,
        vec![Message::internal_reminder(
            InternalReminderKind::InitAgentsMd,
            prompt.to_string(),
        )],
    ))
}

#[cfg(test)]
mod tests {
    use super::{build_init_agents_md_user_input, init_agents_md_user_query};

    #[test]
    fn init_agents_md_user_query_matches_language() {
        assert!(init_agents_md_user_query(true).starts_with("请根据当前项目内容"));
        assert!(init_agents_md_user_query(false).starts_with("Please generate or update AGENTS.md"));
    }

    #[tokio::test]
    async fn init_agents_md_user_input_returns_query_and_reminder_message() {
        let (user_input, prepended_messages) = build_init_agents_md_user_input()
            .await
            .expect("init agents md prompt should build");

        assert!(!user_input.trim().is_empty());
        assert_eq!(prepended_messages.len(), 1);
        assert_eq!(
            prepended_messages[0].internal_reminder_kind(),
            Some(crate::agentic::core::InternalReminderKind::InitAgentsMd)
        );
    }
}
