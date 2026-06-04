use super::types::*;
use crate::function_agents::common::AgentResult;
use crate::infrastructure::ai::AIClientFactory;
use crate::product_domain_runtime::CoreProductDomainRuntime;
use std::path::Path;
use std::sync::Arc;

/// Compatibility facade for the legacy commit generator import path.
pub struct CommitGenerator;

impl CommitGenerator {
    pub async fn generate_commit_message(
        repo_path: &Path,
        options: CommitMessageOptions,
        factory: Arc<AIClientFactory>,
    ) -> AgentResult<CommitMessage> {
        CoreProductDomainRuntime::generate_function_agent_commit_message(
            factory, repo_path, options,
        )
        .await
    }
}
