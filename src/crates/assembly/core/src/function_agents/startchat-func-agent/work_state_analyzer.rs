use super::types::*;
use crate::function_agents::common::AgentResult;
use crate::infrastructure::ai::AIClientFactory;
use crate::product_domain_runtime::CoreProductDomainRuntime;
use std::path::Path;
use std::sync::Arc;

/// Compatibility facade for the legacy work-state analyzer import path.
pub struct WorkStateAnalyzer;

impl WorkStateAnalyzer {
    pub async fn analyze_work_state(
        factory: Arc<AIClientFactory>,
        repo_path: &Path,
        options: WorkStateOptions,
    ) -> AgentResult<WorkStateAnalysis> {
        CoreProductDomainRuntime::analyze_function_agent_work_state(factory, repo_path, options)
            .await
    }
}
