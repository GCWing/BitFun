pub mod ai_service;
pub mod commit_generator;
pub mod context_analyzer;
/**
 * Git Function Agent - module entry
 *
 * Provides Git-related intelligent functions:
 * - Automatic commit message generation
 */
pub use bitfun_product_domains::function_agents::git_func_agent::types;
pub mod utils;

pub use ai_service::AIAnalysisService;
pub use commit_generator::CommitGenerator;
pub use context_analyzer::ContextAnalyzer;
pub use types::*;

use crate::infrastructure::ai::AIClientFactory;
use crate::product_domain_runtime::CoreProductDomainRuntime;
use std::path::Path;
use std::sync::Arc;

/// Provides commit message generation functionality
pub struct GitFunctionAgent {
    factory: Arc<AIClientFactory>,
}

impl GitFunctionAgent {
    pub fn new(factory: Arc<AIClientFactory>) -> Self {
        Self { factory }
    }

    pub async fn generate_commit_message(
        &self,
        repo_path: &Path,
        options: CommitMessageOptions,
    ) -> AgentResult<CommitMessage> {
        CoreProductDomainRuntime::generate_function_agent_commit_message(
            self.factory.clone(),
            repo_path,
            options,
        )
        .await
    }

    /// Quickly generate commit message (use default options)
    pub async fn quick_commit_message(&self, repo_path: &Path) -> AgentResult<CommitMessage> {
        self.generate_commit_message(repo_path, CommitMessageOptions::default())
            .await
    }
}
