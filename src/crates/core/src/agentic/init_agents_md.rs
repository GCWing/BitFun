use crate::agentic::agents::get_embedded_prompt;
use crate::agentic::core::PromptEnvelope;
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

pub(crate) async fn build_init_agents_md_user_input() -> BitFunResult<(String, String)> {
    let prompt = get_embedded_prompt(INIT_AGENTS_MD_PROMPT_NAME).ok_or_else(|| {
        BitFunError::Agent(format!(
            "{} not found in embedded files",
            INIT_AGENTS_MD_PROMPT_NAME
        ))
    })?;
    let is_chinese = get_app_language_code().await.starts_with("zh");
    let user_query = init_agents_md_user_query(is_chinese).to_string();
    let mut envelope = PromptEnvelope::new();
    envelope.push_system_reminder(prompt.to_string());
    envelope.push_user_query(user_query.clone());
    Ok((envelope.render(), user_query))
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
    async fn init_agents_md_user_input_wraps_reminder_before_query() {
        let (user_input, original_user_input) = build_init_agents_md_user_input()
            .await
            .expect("init agents md prompt should build");

        assert!(user_input.contains("<system_reminder>"));
        assert!(user_input.contains("<user_query>"));
        assert!(user_input.find("<system_reminder>") < user_input.find("<user_query>"));
        assert!(!original_user_input.trim().is_empty());
    }
}
