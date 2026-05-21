use super::types::*;
use crate::function_agents::common::AgentResult;
use crate::function_agents::port_adapters::{
    CoreFunctionAgentAiAdapter, CoreFunctionAgentGitAdapter,
};
use crate::infrastructure::ai::AIClientFactory;
use bitfun_product_domains::function_agents::ports::FunctionAgentRuntimeFacade;
/**
 * Git Function Agent - commit message generator
 *
 * Uses AI to deeply analyze code changes and generate compliant commit messages
 */
use log::info;
use std::path::Path;
use std::sync::Arc;

pub struct CommitGenerator;

impl CommitGenerator {
    pub async fn generate_commit_message(
        repo_path: &Path,
        options: CommitMessageOptions,
        factory: Arc<AIClientFactory>,
    ) -> AgentResult<CommitMessage> {
        info!(
            "Generating commit message (AI-driven): repo_path={:?}",
            repo_path
        );

        let git_adapter = CoreFunctionAgentGitAdapter::default();
        let ai_adapter = CoreFunctionAgentAiAdapter::new(factory);
        let facade = FunctionAgentRuntimeFacade::new(&git_adapter, &ai_adapter);
        facade
            .generate_commit_message(repo_path.to_path_buf(), options)
            .await
    }
}
